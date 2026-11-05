//! Detector de atraso (estilo GCC): agrupa pacotes em janelas de envio, acumula a variação de
//! atraso, suaviza e ajusta uma reta; a inclinação indica fila crescendo ou esvaziando.

use crate::window::SlidingMin;
use crate::Us;
use std::collections::VecDeque;
use zap_proto::DelayState;

/// Largura da janela de agrupamento por envio (µs).
const JANELA_US: u64 = 5_000;
/// Quantidade de pontos usados na regressão.
const PONTOS: usize = 20;
const GAMMA_INICIAL: f64 = 12.5;
const GAMMA_MIN: f64 = 6.0;
const GAMMA_MAX: f64 = 600.0;
const K_SOBE: f64 = 0.039;
const K_DESCE: f64 = 0.0087;
/// Tempo (ms) acima do limiar antes de declarar sobrecarga.
const TEMPO_OVERUSE_MS: f64 = 10.0;
/// Grupos seguidos acima do limiar antes de declarar sobrecarga (como no GCC).
const GRUPOS_OVERUSE: u32 = 2;
/// Grupo com mais bytes que este fator × média é uma rajada (keyframe): o atraso que ele cria
/// para si mesmo e para os quadros logo atrás não é congestionamento.
const FATOR_RAJADA: f64 = 4.0;
/// Margem (µs) para considerar que o atraso voltou ao nível de antes da rajada.
const MARGEM_RAJADA_US: f64 = 5_000.0;
/// Tempo máximo (µs) ignorando grupos depois de uma rajada: cobre o tempo de transmissão de um
/// keyframe; congestionamento de verdade não fica escondido mais que isso.
const MAX_RAJADA_US: Us = 200_000;
/// Fila persistente (ideia do CoDel): se nem o **menor** atraso dos últimos 300 ms desce até
/// 30 ms acima da base (menor atraso dos últimos 10 s), a fila nunca esvazia: é sobrecarga mesmo
/// sem tendência. Pega o platô de fila cheia que o gradiente não vê (banda caiu junto com uma
/// rajada, ou gargalo com fila curta que descarta o excesso). Jitter aleatório não segura o mínimo
/// alto, então não dispara à toa.
const FILA_PERSISTENTE_US: f64 = 30_000.0;
const JANELA_FILA_PERSISTENTE_US: Us = 300_000;
const JANELA_ATRASO_MIN_US: Us = 10_000_000;

#[derive(Debug, Clone, Copy)]
struct Grupo {
    primeiro_envio: Us,
    ultimo_envio: Us,
    ultima_chegada: Us,
    bytes: u64,
}

impl Grupo {
    fn novo(send_us: Us, arrival_us: Us, bytes: u32) -> Self {
        Self {
            primeiro_envio: send_us,
            ultimo_envio: send_us,
            ultima_chegada: arrival_us,
            bytes: u64::from(bytes),
        }
    }
    /// Atraso de ida (com a diferença de relógio embutida, que se cancela nas comparações).
    fn atraso(&self) -> f64 {
        self.ultima_chegada as f64 - self.ultimo_envio as f64
    }
}

#[derive(Debug, Clone)]
pub struct DelayDetector {
    state: DelayState,
    atual: Option<Grupo>,
    anterior: Option<Grupo>,
    acc: f64,
    suave: f64,
    n_grupos: u32,
    chegada_base: Option<Us>,
    pontos: VecDeque<(f64, f64)>,
    gamma: f64,
    m_anterior: f64,
    tempo_acima: f64,
    grupos_acima: u32,
    ultimo_fechamento: Option<Us>,
    /// Média móvel de bytes por grupo (só grupos normais).
    media_bytes: f64,
    /// Depois de uma rajada: (atraso de antes dela, atraso da própria rajada, chegada da rajada).
    rajada: Option<(f64, f64, Us)>,
    /// Menor atraso de ida nos últimos 10 s (base sem fila).
    atraso_min: SlidingMin<f64>,
    /// Menor atraso de ida nos últimos 300 ms (fila persistente).
    atraso_min_curto: SlidingMin<f64>,
    /// Chegada do primeiro grupo (a janela curta só vale depois de cheia).
    primeira_chegada: Option<Us>,
    fila_persistente: bool,
    /// Fila estimada (µs) no último grupo fechado; atualizada até durante rajadas.
    fila_us: f64,
}

impl DelayDetector {
    pub fn new() -> Self {
        Self {
            state: DelayState::Normal,
            atual: None,
            anterior: None,
            acc: 0.0,
            suave: 0.0,
            n_grupos: 0,
            chegada_base: None,
            pontos: VecDeque::with_capacity(PONTOS + 1),
            gamma: GAMMA_INICIAL,
            m_anterior: 0.0,
            tempo_acima: 0.0,
            grupos_acima: 0,
            ultimo_fechamento: None,
            media_bytes: 0.0,
            rajada: None,
            atraso_min: SlidingMin::new(JANELA_ATRASO_MIN_US),
            atraso_min_curto: SlidingMin::new(JANELA_FILA_PERSISTENTE_US),
            primeira_chegada: None,
            fila_persistente: false,
            fila_us: 0.0,
        }
    }

    pub fn on_packet(&mut self, send_us: Us, arrival_us: Us, bytes: u32) -> DelayState {
        match self.atual {
            None => {
                self.atual = Some(Grupo::novo(send_us, arrival_us, bytes));
            }
            Some(mut g) => {
                if send_us < g.ultimo_envio {
                    // Fora de ordem de envio: ignora.
                    return self.state;
                }
                if send_us - g.primeiro_envio >= JANELA_US {
                    self.fechar(g);
                    self.atual = Some(Grupo::novo(send_us, arrival_us, bytes));
                } else {
                    g.ultimo_envio = send_us;
                    g.ultima_chegada = g.ultima_chegada.max(arrival_us);
                    g.bytes += u64::from(bytes);
                    self.atual = Some(g);
                }
            }
        }
        self.state
    }

    pub fn state(&self) -> DelayState {
        self.state
    }

    /// Fila atual estimada (µs): atraso de ida do último grupo fechado menos o menor atraso dos
    /// últimos 10 s (`atraso_min`). 0.0 antes de dois grupos. Nunca negativo.
    pub fn fila_us(&self) -> f64 {
        self.fila_us
    }

    #[cfg(test)]
    pub(crate) fn gamma(&self) -> f64 {
        self.gamma
    }

    /// Fecha um grupo: calcula a variação de atraso e atualiza o estado.
    fn fechar(&mut self, g: Grupo) {
        let Some(ant) = self.anterior.replace(g) else {
            self.media_bytes = g.bytes as f64;
            return;
        };
        let base = self.atraso_min.push(g.ultima_chegada, g.atraso());
        // Fila = atraso deste grupo acima da base; antes do filtro de rajada para acompanhá-la.
        self.fila_us = (g.atraso() - base).max(0.0);
        let curto = self.atraso_min_curto.push(g.ultima_chegada, g.atraso());
        let inicio = *self.primeira_chegada.get_or_insert(g.ultima_chegada);
        let janela_cheia = g.ultima_chegada.saturating_sub(inicio) >= JANELA_FILA_PERSISTENTE_US;
        self.fila_persistente = janela_cheia && curto - base > FILA_PERSISTENTE_US;
        // Filtro de rajada: um grupo muito maior que a média (keyframe) atrasa a si mesmo e aos
        // quadros logo atrás dele, e depois a fila esvazia sozinha. Esses grupos ficam fora da
        // tendência até o atraso voltar ao nível de antes, piorar além do da rajada, ou passar
        // MAX_RAJADA_US.
        if let Some((atraso_antes, atraso_rajada, inicio)) = self.rajada {
            let voltou = g.atraso() <= atraso_antes + MARGEM_RAJADA_US;
            let piorou = g.atraso() > atraso_rajada + MARGEM_RAJADA_US;
            let expirou = g.ultima_chegada.saturating_sub(inicio) >= MAX_RAJADA_US;
            if !(voltou || piorou || expirou) {
                return;
            }
            self.rajada = None;
            if voltou {
                return; // este grupo ainda fecha a rajada; a tendência recomeça no próximo
            }
        } else if self.media_bytes > 0.0 && g.bytes as f64 > FATOR_RAJADA * self.media_bytes {
            self.rajada = Some((ant.atraso(), g.atraso(), g.ultima_chegada));
            return;
        }
        self.media_bytes = 0.9 * self.media_bytes + 0.1 * g.bytes as f64;
        let d_chegada = g.ultima_chegada as f64 - ant.ultima_chegada as f64;
        let d_envio = g.ultimo_envio as f64 - ant.ultimo_envio as f64;
        let d = (d_chegada - d_envio) / 1000.0;

        self.acc += d;
        self.suave = 0.9 * self.suave + 0.1 * self.acc;
        self.n_grupos = self.n_grupos.saturating_add(1);

        let base = *self.chegada_base.get_or_insert(ant.ultima_chegada);
        let chegada_ms = (g.ultima_chegada as f64 - base as f64) / 1000.0;
        if self.pontos.len() == PONTOS {
            self.pontos.pop_front();
        }
        self.pontos.push_back((chegada_ms, self.suave));

        let k = self.inclinacao();
        let m = k * f64::from(self.n_grupos.min(60)) * 4.0;

        let dt_ms = match self.ultimo_fechamento {
            Some(u) => ((g.ultima_chegada as f64 - u as f64) / 1000.0).clamp(0.0, 100.0),
            None => 0.0,
        };
        self.ultimo_fechamento = Some(g.ultima_chegada);

        // Limiar adaptativo; picos grandes não o arrastam.
        let excesso = m.abs() - self.gamma;
        if excesso <= 15.0 {
            let k_adapt = if m.abs() > self.gamma { K_SOBE } else { K_DESCE };
            self.gamma = (self.gamma + dt_ms * k_adapt * excesso).clamp(GAMMA_MIN, GAMMA_MAX);
        }

        if m > self.gamma {
            // Como no GCC: o primeiro grupo acima conta meio intervalo, e são precisos
            // GRUPOS_OVERUSE grupos seguidos com a tendência ainda subindo.
            self.tempo_acima += if self.grupos_acima == 0 { dt_ms / 2.0 } else { dt_ms };
            self.grupos_acima += 1;
            self.state = if self.tempo_acima >= TEMPO_OVERUSE_MS
                && self.grupos_acima >= GRUPOS_OVERUSE
                && m >= self.m_anterior
            {
                DelayState::Overuse
            } else {
                DelayState::Normal
            };
        } else {
            self.tempo_acima = 0.0;
            self.grupos_acima = 0;
            self.state = if m < -self.gamma {
                DelayState::Underuse
            } else {
                DelayState::Normal
            };
        }
        if self.fila_persistente {
            self.state = DelayState::Overuse;
        }
        self.m_anterior = m;
    }

    /// Regressão linear por mínimos quadrados (atraso suavizado por ms).
    fn inclinacao(&self) -> f64 {
        let n = self.pontos.len();
        if n < 2 {
            return 0.0;
        }
        let nf = n as f64;
        let mx = self.pontos.iter().map(|p| p.0).sum::<f64>() / nf;
        let my = self.pontos.iter().map(|p| p.1).sum::<f64>() / nf;
        let (num, den) = self.pontos.iter().fold((0.0, 0.0), |(a, b), p| {
            (a + (p.0 - mx) * (p.1 - my), b + (p.0 - mx) * (p.0 - mx))
        });
        if den == 0.0 {
            0.0
        } else {
            num / den
        }
    }
}

impl Default for DelayDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Gera pacotes a cada 2 ms; `atraso_ms(i)` define o atraso de rede. Chegada monotônica.
    fn rodar(det: &mut DelayDetector, inicio: u64, n: u64, mut atraso_ms: impl FnMut(u64) -> i64) -> Vec<DelayState> {
        let mut saida = Vec::new();
        let mut ult = 0u64;
        for i in 0..n {
            let envio = (inicio + i) * 2_000;
            let a = (atraso_ms(inicio + i) * 1000).max(0) as u64;
            let chegada = (envio + a).max(ult);
            ult = chegada;
            saida.push(det.on_packet(envio, chegada + 1_000_000, 1200));
        }
        saida
    }

    #[test]
    fn enlace_estavel() {
        let mut d = DelayDetector::new();
        let s = rodar(&mut d, 0, 500, |_| 30);
        assert!(s.iter().all(|e| *e != DelayState::Overuse));
    }

    #[test]
    fn fila_crescente() {
        let mut d = DelayDetector::new();
        let s = rodar(&mut d, 0, 300, |i| 30 + i as i64);
        let pos = s.iter().position(|e| *e == DelayState::Overuse);
        assert!(matches!(pos, Some(p) if p < 200), "pos = {pos:?}");
    }

    #[test]
    fn fila_esvaziando() {
        let mut d = DelayDetector::new();
        rodar(&mut d, 0, 300, |i| 30 + i as i64);
        let s = rodar(&mut d, 300, 300, |i| 330 - (i as i64 - 300));
        assert!(s.contains(&DelayState::Underuse));
        let s2 = rodar(&mut d, 600, 300, |_| 30);
        assert_eq!(*s2.last().unwrap(), DelayState::Normal);
    }

    #[test]
    fn jitter_pequeno() {
        let mut d = DelayDetector::new();
        let mut x: u64 = 0x2545_F491_4F6C_DD1D;
        let s = rodar(&mut d, 0, 2000, |_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            30 + (x % 7) as i64 - 3
        });
        let over = s.iter().filter(|e| **e == DelayState::Overuse).count();
        assert!(over * 100 <= s.len(), "overuse = {over}");
    }

    #[test]
    fn limiar_adapta() {
        let mut d = DelayDetector::new();
        rodar(&mut d, 0, 5000, |_| 30);
        let g = d.gamma();
        assert!((6.0..=600.0).contains(&g), "gamma = {g}");
    }

    /// Quadros de 60 fps (um grupo por quadro) num enlace de `cap_kbps` sem outra fila: devolve
    /// os estados. `tamanho(i)` dá os bytes do quadro i.
    fn quadros(det: &mut DelayDetector, n: u64, cap_kbps: f64, mut tamanho: impl FnMut(u64) -> u32) -> Vec<DelayState> {
        let mut livre_em = 0f64; // µs em que o enlace termina o que já tem
        let mut estados = Vec::new();
        for i in 0..n {
            let envio = i as f64 * 16_667.0;
            let bytes = tamanho(i);
            let inicio = envio.max(livre_em);
            livre_em = inicio + f64::from(bytes) * 8.0 / cap_kbps * 1000.0;
            estados.push(det.on_packet(envio as u64, (livre_em + 30_000.0) as u64, bytes));
        }
        estados
    }

    #[test]
    fn keyframe_com_folga_nao_e_sobrecarga() {
        // 6 Mbps num enlace de 8 Mbps, keyframe 8x a cada 2 s: a fila do keyframe esvazia sozinha.
        let mut det = DelayDetector::new();
        let normal = 6_000_000 / 8 / 60;
        let estados = quadros(
            &mut det,
            600,
            8_000.0,
            |i| if i % 120 == 0 { normal * 8 } else { normal },
        );
        assert!(estados.iter().all(|e| *e != DelayState::Overuse), "{estados:?}");
    }

    #[test]
    fn fila_cheia_sem_tendencia_e_sobrecarga() {
        // O atraso pula 250 ms de uma vez e fica parado (fila cheia): o gradiente some, mas a
        // fila que não esvazia por 300 ms é sobrecarga.
        let mut det = DelayDetector::new();
        let mut estados = Vec::new();
        for i in 0..400u64 {
            let envio = i * 16_667;
            let atraso = if i < 200 { 30_000 } else { 280_000 };
            estados.push(det.on_packet(envio, envio + atraso, 15_000));
        }
        assert!(
            estados[200..230].contains(&DelayState::Overuse),
            "{:?}",
            &estados[200..230]
        );
    }

    #[test]
    fn jitter_aleatorio_nao_e_fila_persistente() {
        // Atraso base + |ruído| de até 60 ms, sem fila: o mínimo de 300 ms fica perto da base.
        let mut det = DelayDetector::new();
        // (O detector de tendência do GCC ainda acusa sobrecarga de vez em quando com um jitter
        // tão grande; aqui só importa que a regra nova não dispare.)
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        for i in 0..1200u64 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let ruido = x % 60_000;
            let envio = i * 16_667;
            det.on_packet(envio, envio + 30_000 + ruido, 12_000);
            assert!(
                !det.fila_persistente,
                "fila persistente acusada no quadro {i} só com jitter"
            );
        }
    }

    /// Pacotes de 1250 B a cada 5 ms (um grupo por pacote) com atraso fixo; a chegada nunca
    /// volta atrás (`max` com a anterior), como num enlace real.
    fn pacotes(det: &mut DelayDetector, inicio_envio: u64, n: u64, atraso_us: u64, ult: &mut u64) {
        for i in 0..n {
            let envio = inicio_envio + i * 5_000;
            let chegada = (envio + atraso_us).max(*ult);
            *ult = chegada;
            det.on_packet(envio, chegada, 1250);
        }
    }

    #[test]
    fn fila_us_sobe_e_desce() {
        let mut det = DelayDetector::new();
        let mut ult = 0u64;
        // Base: 1 s com atraso de 20 ms.
        pacotes(&mut det, 0, 200, 20_000, &mut ult);
        assert!(det.fila_us() <= 5_000.0, "fila = {}", det.fila_us());
        // Fila cresce: 40 pacotes (200 ms de envio) com atraso de 100 ms.
        let envio = 200 * 5_000;
        pacotes(&mut det, envio, 40, 100_000, &mut ult);
        assert!(det.fila_us() >= 70_000.0, "fila = {}", det.fila_us());
        // Fila esvazia: 1 s de volta aos 20 ms.
        let envio = envio + 40 * 5_000;
        pacotes(&mut det, envio, 200, 20_000, &mut ult);
        assert!(det.fila_us() <= 5_000.0, "fila = {}", det.fila_us());
    }

    #[test]
    fn fila_us_zero_no_inicio() {
        let mut det = DelayDetector::new();
        assert_eq!(det.fila_us(), 0.0);
        // Um pacote: grupo ainda aberto.
        det.on_packet(0, 20_000, 1250);
        assert_eq!(det.fila_us(), 0.0);
        // Segundo grupo: fecha só o primeiro, que ainda não basta para a fila.
        det.on_packet(10_000, 30_000, 1250);
        assert_eq!(det.fila_us(), 0.0);
    }

    #[test]
    fn fila_us_nunca_negativa() {
        let mut det = DelayDetector::new();
        let mut ult = 0u64;
        // Atraso caindo de 100 ms para 20 ms: o último grupo está sempre no mínimo.
        for i in 0..200u64 {
            let atraso = 100_000 - i * 400;
            pacotes(&mut det, i * 5_000, 1, atraso, &mut ult);
            assert!(det.fila_us() >= 0.0, "fila = {} no pacote {i}", det.fila_us());
        }
    }
}
