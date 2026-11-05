//! Estimador de banda (detector de atraso + AIMD + perda).
#![allow(dead_code)]

use crate::trendline::DelayDetector;
use crate::window::{LossMeter, RateMeter};
use crate::Us;
use zap_proto::DelayState;

/// Mínimo da carência depois da última sonda (keyframe não tem mínimo): sobrecarga e descarte
/// do emissor não reduzem nem aparecem.
const CARENCIA_SONDA_MIN_US: Us = 300_000;
/// Máximo da carência depois do último evento de rajada própria.
const CARENCIA_MAX_US: Us = 1_000_000;
/// Fila (atraso acima da base) abaixo da qual a rajada já escoou.
const FILA_ESCOADA_US: f64 = 30_000.0;
/// Fração da taxa medida no grupo que vira estimativa.
const FATOR_SONDA: f64 = 0.95;
/// Janela da vazão medida (µs).
const JANELA_VAZAO_US: Us = 500_000;
/// Janela de perda (µs).
const JANELA_PERDA_US: Us = 2_000_000;
/// Intervalo mínimo entre atualizações (µs).
const INTERVALO_MIN_US: Us = 25_000;
/// Limite do dt usado no crescimento (µs).
const DT_MAX_US: Us = 1_000_000;
/// Espaço mínimo entre reduções por sobrecarga (µs).
const ESPACO_REDUCAO_US: Us = 200_000;
/// Espaço mínimo entre reduções por perda (µs).
const ESPACO_REDUCAO_PERDA_US: Us = 300_000;
/// Nos primeiros ms a vazão ainda subestima: não reduz por sobrecarga (µs).
const CARENCIA_US: Us = 500_000;
/// Fator da redução por sobrecarga sobre a vazão medida.
const FATOR_OVERUSE: f64 = 0.85;
/// Abaixo desta fração do alvo a vazão indica fonte limitada (app-limited).
const FRACAO_APP_LIMITED: f64 = 0.5;
/// Crescimento multiplicativo por segundo.
const CRESCIMENTO_MULT: f64 = 1.08;
/// Crescimento na fase inicial (por segundo), até a primeira sobrecarga ou perda: acha a
/// capacidade em poucos segundos em vez de levar minutos subindo 8%/s (como o "slow start").
const CRESCIMENTO_INICIAL: f64 = 1.5;
/// Crescimento aditivo perto do ponto de sobrecarga anterior: o maior entre um valor fixo
/// (kbps por segundo) e uma fração do alvo por segundo.
const CRESCIMENTO_ADIT_KBPS: f64 = 96.0;
const CRESCIMENTO_ADIT_FRAC: f64 = 0.03;
/// Depois desse tempo sem sobrecarga, a vazão da última sobrecarga é esquecida e volta o
/// crescimento multiplicativo (a rede pode ter melhorado; ex.: volta de 3 para 8 Mbps).
const ESQUECER_SOBRECARGA_US: Us = 5_000_000;
/// Alvo acima de (este fator * vazão na última sobrecarga) volta ao modo multiplicativo.
const FATOR_RETOMADA: f64 = 1.5;
/// Perda acima disto reduz; abaixo de PERDA_BAIXA não faz nada.
const PERDA_ALTA: f32 = 0.10;
/// Quadros descartados pelo emissor (buracos de `seq` no vídeo) nesta janela...
const JANELA_DESCARTE_US: Us = 1_000_000;
/// ...a partir desta quantidade indicam que a rota não aguenta a taxa atual, mesmo sem fila
/// visível: o emissor segura o excesso e descarta (admissão/prazo). A estimativa vai para
/// FATOR_DESCARTE × a vazão que de fato chega (para baixo ou para cima) e não sobe além disso.
const DESCARTES_LIMITE: usize = 3;
const FATOR_DESCARTE: f64 = 0.95;
const PERDA_BAIXA: f32 = 0.02;

#[derive(Debug, Clone, Copy)]
pub struct BweConfig {
    pub min_kbps: u32,
    pub max_kbps: u32,
    pub start_kbps: u32,
}
impl Default for BweConfig {
    fn default() -> Self {
        Self {
            min_kbps: 150,
            max_kbps: 50_000,
            start_kbps: 1_000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Bwe {
    cfg: BweConfig,
    estimate_kbps: u32,
    state: DelayState,
    alvo: f64,
    detector: DelayDetector,
    rate: RateMeter,
    loss: LossMeter,
    loss_atual: f32,
    now_chegada: Us,
    primeiro_pacote: Option<Us>,
    ultimo_update: Option<Us>,
    ultima_reducao: Option<Us>,
    ultima_reducao_perda: Option<Us>,
    vazao_na_sobrecarga: Option<f64>,
    ultima_sobrecarga: Option<Us>,
    /// Até a primeira sobrecarga ou perda relevante: crescimento rápido.
    fase_inicial: bool,
    /// Instantes dos quadros descartados pelo emissor (janela de 1 s).
    descartes: std::collections::VecDeque<Us>,
    /// Instante do último evento de sonda (mantém a carência por no mínimo
    /// `CARENCIA_SONDA_MIN_US`, e até `CARENCIA_MAX_US` enquanto houver fila).
    ultima_sonda: Option<Us>,
    /// Instante do último evento de keyframe (carência só enquanto houver fila, até o máximo).
    ultimo_keyframe: Option<Us>,
    /// Sobrecarga atual do detector: quando começou e se começou dentro da carência.
    overuse_desde: Option<(Us, bool)>,
}
impl Bwe {
    pub fn new(cfg: BweConfig) -> Self {
        let min = f64::from(cfg.min_kbps);
        let max = f64::from(cfg.max_kbps).max(min);
        let alvo = f64::from(cfg.start_kbps).clamp(min, max);
        Self {
            cfg,
            estimate_kbps: alvo.round() as u32,
            state: DelayState::Normal,
            alvo,
            detector: DelayDetector::new(),
            rate: RateMeter::new(JANELA_VAZAO_US),
            loss: LossMeter::new(JANELA_PERDA_US),
            loss_atual: 0.0,
            now_chegada: 0,
            primeiro_pacote: None,
            ultimo_update: None,
            ultima_reducao: None,
            ultima_reducao_perda: None,
            vazao_na_sobrecarga: None,
            ultima_sobrecarga: None,
            fase_inicial: true,
            descartes: std::collections::VecDeque::new(),
            ultima_sonda: None,
            ultimo_keyframe: None,
            overuse_desde: None,
        }
    }
    pub fn on_packet(&mut self, send_us: Us, arrival_us: Us, bytes: u32) {
        let antes = self.state;
        self.state = self.detector.on_packet(send_us, arrival_us, bytes);
        self.rate.add(arrival_us, bytes);
        self.primeiro_pacote.get_or_insert(arrival_us);
        self.now_chegada = arrival_us;
        if self.state != DelayState::Overuse {
            self.overuse_desde = None;
        } else if antes != DelayState::Overuse {
            self.overuse_desde = Some((arrival_us, self.em_carencia(arrival_us)));
        }
    }
    /// Um quadro de vídeo que o emissor descartou (o receptor vê um buraco de `seq`).
    /// `seen_us` = quando o buraco de `seq` foi visto (não quando venceu).
    pub fn on_frame_dropped(&mut self, seen_us: Us) {
        // Na carência o emissor está só reagindo à rajada própria: não conta.
        if self.em_carencia(seen_us) {
            return;
        }
        // Mantém ordenado: o `pop_front` da janela depende disso.
        let pos = self.descartes.partition_point(|&t| t <= seen_us);
        self.descartes.insert(pos, seen_us);
    }
    pub fn on_loss_fraction(&mut self, _now: Us, loss: f32) {
        // A janela de 2 s vem do relatório; aqui usamos a fração direto.
        self.loss_atual = if loss.is_nan() { 0.0 } else { loss.clamp(0.0, 1.0) };
    }
    pub fn update(&mut self, now: Us) -> u32 {
        let Some(ultimo) = self.ultimo_update else {
            self.ultimo_update = Some(now);
            return self.estimate_kbps;
        };
        let dt_us = now.saturating_sub(ultimo);
        if dt_us < INTERVALO_MIN_US {
            return self.estimate_kbps;
        }
        self.ultimo_update = Some(now);
        let dt_s = dt_us.min(DT_MAX_US) as f64 / 1_000_000.0;
        let vazao = self.rate.rate_bps(now) / 1000.0;
        let min = f64::from(self.cfg.min_kbps);
        let max = f64::from(self.cfg.max_kbps).max(min);

        let separado = |ultima: Option<Us>, espaco: Us| ultima.is_none_or(|u| now.saturating_sub(u) >= espaco);
        let passou_carencia = self
            .primeiro_pacote
            .is_some_and(|p| now.saturating_sub(p) >= CARENCIA_US);
        let perda_media = self.loss_atual >= PERDA_BAIXA && self.loss_atual <= PERDA_ALTA;

        while self
            .descartes
            .front()
            .is_some_and(|t| now.saturating_sub(*t) > JANELA_DESCARTE_US)
        {
            self.descartes.pop_front();
        }
        let descartando = self.descartes.len() >= DESCARTES_LIMITE && passou_carencia;
        let carencia = self.em_carencia(now);
        let ignora_overuse = self.ignora_overuse(now);

        match self.state {
            _ if descartando && !carencia => {
                // Vai para perto do que de fato chega, para baixo ou para cima: a rota carrega
                // pelo menos isso, e não mais (o resto o emissor está jogando fora).
                if separado(self.ultima_reducao, ESPACO_REDUCAO_US) {
                    self.alvo = FATOR_DESCARTE * vazao;
                    self.ultima_reducao = Some(now);
                    self.vazao_na_sobrecarga = Some(vazao);
                    self.ultima_sobrecarga = Some(now);
                    self.fase_inicial = false;
                }
            }
            DelayState::Overuse if !ignora_overuse => {
                if passou_carencia && separado(self.ultima_reducao, ESPACO_REDUCAO_US) {
                    self.alvo = self.alvo.min(FATOR_OVERUSE * vazao);
                    self.ultima_reducao = Some(now);
                    self.vazao_na_sobrecarga = Some(vazao);
                    self.ultima_sobrecarga = Some(now);
                    self.fase_inicial = false;
                }
            }
            // Sobrecarga da própria rajada (sonda/keyframe): não mexe em nada.
            DelayState::Overuse => {}
            DelayState::Underuse => {}
            DelayState::Normal => {
                if vazao >= FRACAO_APP_LIMITED * self.alvo && !perda_media && self.loss_atual <= PERDA_ALTA {
                    let esquecida = self
                        .ultima_sobrecarga
                        .is_none_or(|u| now.saturating_sub(u) >= ESQUECER_SOBRECARGA_US);
                    let mult = esquecida || self.vazao_na_sobrecarga.is_none_or(|v| self.alvo > FATOR_RETOMADA * v);
                    if self.fase_inicial {
                        self.alvo *= CRESCIMENTO_INICIAL.powf(dt_s);
                    } else if mult {
                        self.alvo *= CRESCIMENTO_MULT.powf(dt_s);
                    } else {
                        self.alvo += CRESCIMENTO_ADIT_KBPS.max(CRESCIMENTO_ADIT_FRAC * self.alvo) * dt_s;
                    }
                }
            }
        }

        if self.loss_atual >= PERDA_BAIXA {
            self.fase_inicial = false;
        }
        if self.loss_atual > PERDA_ALTA && separado(self.ultima_reducao_perda, ESPACO_REDUCAO_PERDA_US) {
            self.alvo *= 1.0 - 0.5 * f64::from(self.loss_atual);
            self.ultima_reducao_perda = Some(now);
        }

        self.alvo = self.alvo.clamp(min, max);
        self.estimate_kbps = self.alvo.round() as u32;
        self.estimate_kbps
    }
    /// Sonda chegou: igual a `on_packet` (detector e vazão) e marca a janela de sondagem.
    pub fn on_probe_packet(&mut self, send_us: Us, arrival_us: Us, bytes: u32) {
        self.on_packet(send_us, arrival_us, bytes);
        self.marca_sonda(arrival_us);
    }
    fn marca_sonda(&mut self, t: Us) {
        self.ultima_sonda = Some(self.ultima_sonda.map_or(t, |u| u.max(t)));
    }
    fn marca_keyframe(&mut self, t: Us) {
        self.ultimo_keyframe = Some(self.ultimo_keyframe.map_or(t, |u| u.max(t)));
    }
    /// Cabeçalho de um keyframe chegou (o quadro ainda vai chegar): evento de rajada (keyframe).
    pub fn on_keyframe_start(&mut self, arrival_us: Us) {
        self.marca_keyframe(arrival_us);
    }
    /// Keyframe terminou de chegar: `on_packet` + evento de rajada (keyframe) em `arrival_us`.
    pub fn on_keyframe_packet(&mut self, send_us: Us, arrival_us: Us, bytes: u32) {
        self.on_packet(send_us, arrival_us, bytes);
        self.marca_keyframe(arrival_us);
    }
    /// Fim de um grupo (`ProbeMeter`). Com perda <= PERDA_ALTA: `alvo = max(alvo, 0,95 x kbps)`,
    /// limitado a `max_kbps`; `estimate_kbps()` ja devolve o novo valor.
    pub fn on_probe_cluster(&mut self, now: Us, kbps: u32) {
        // Estende a carência a partir do fim do grupo, mesmo se a perda for alta.
        self.marca_sonda(now);
        if self.loss_atual > PERDA_ALTA {
            return;
        }
        let min = f64::from(self.cfg.min_kbps);
        let max = f64::from(self.cfg.max_kbps).max(min);
        self.alvo = self.alvo.max(FATOR_SONDA * f64::from(kbps)).clamp(min, max);
        self.estimate_kbps = self.alvo.round() as u32;
    }
    /// Rajada própria (sonda/keyframe) ainda em curso em `t`: até 1 s do último evento; nos
    /// primeiros 300 ms da última sonda sempre, depois só enquanto a fila não escoou.
    fn em_carencia(&self, t: Us) -> bool {
        let Some(ultimo) = self.ultima_sonda.max(self.ultimo_keyframe) else {
            return false;
        };
        if t.saturating_sub(ultimo) > CARENCIA_MAX_US {
            return false;
        }
        if self
            .ultima_sonda
            .is_some_and(|s| t.saturating_sub(s) <= CARENCIA_SONDA_MIN_US)
        {
            return true;
        }
        self.detector.fila_us() > FILA_ESCOADA_US
    }
    /// Sobrecarga que é da rajada própria em `t`: durante a carência, e também a que **começou**
    /// na carência e ainda dura, até 1 s do último evento de rajada e 1 s do início dela. A fila
    /// pode escoar (fim da carência) antes de a tendência do detector voltar ao normal: sem isso o
    /// resto da sobrecarga do keyframe cortava a estimativa para 0,85 × vazão logo depois dele
    /// (medido no M4c, `docs/planos/m4c-estabilizacao.md` §11). Sobrecarga real que começa junto
    /// da rajada continua sendo vista, no máximo 1 s depois.
    fn ignora_overuse(&self, t: Us) -> bool {
        if self.em_carencia(t) {
            return true;
        }
        let Some((inicio, na_carencia)) = self.overuse_desde else {
            return false;
        };
        let Some(ultimo) = self.ultima_sonda.max(self.ultimo_keyframe) else {
            return false;
        };
        na_carencia && t.saturating_sub(ultimo) <= CARENCIA_MAX_US && t.saturating_sub(inicio) <= CARENCIA_MAX_US
    }
    pub fn estimate_kbps(&self) -> u32 {
        self.estimate_kbps
    }
    pub fn state(&self) -> DelayState {
        // A rajada própria causa fila de propósito: não anuncia essa sobrecarga.
        if self.state == DelayState::Overuse && self.ignora_overuse(self.now_chegada) {
            return DelayState::Normal;
        }
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simula `seg` segundos: pacote de 1250 B a cada 10 ms (1 Mbps) e update a cada 50 ms.
    /// `atraso_us(i)` dá o atraso do pacote i. Devolve (bwe, vazão esperada em kbps no fim).
    fn rodar(bwe: &mut Bwe, seg: u64, passo_us: u64, mut atraso_us: impl FnMut(u64) -> u64) {
        let n = seg * 1_000_000 / passo_us;
        let mut ult = 0;
        for i in 0..n {
            let envio = i * passo_us;
            let chegada = (envio + atraso_us(i)).max(ult);
            ult = chegada;
            bwe.on_packet(envio, chegada, 1250);
            if envio % 50_000 == 0 {
                bwe.update(chegada);
            }
        }
    }

    #[test]
    fn cresce_sem_congestao() {
        let mut b = Bwe::new(BweConfig::default());
        rodar(&mut b, 10, 10_000, |_| 20_000);
        assert!(b.estimate_kbps() > 1500, "estimate = {}", b.estimate_kbps());
    }

    #[test]
    fn reduz_em_overuse() {
        let mut b = Bwe::new(BweConfig::default());
        // Atraso acelera (quadrático) com pacotes a 1 Mbps. O limiar adaptativo acaba
        // acompanhando a fila, então medimos o menor valor atingido durante a sobrecarga.
        let mut ult = 0;
        let mut minimo = u32::MAX;
        for i in 0..500u64 {
            let envio = i * 10_000;
            let chegada = (envio + 20_000 + i * i * 50).max(ult);
            ult = chegada;
            b.on_packet(envio, chegada, 1250);
            if i % 5 == 0 {
                b.update(chegada);
                minimo = minimo.min(b.estimate_kbps());
            }
        }
        assert!(f64::from(minimo) < 0.9 * 1000.0, "minimo = {minimo}");
    }

    #[test]
    fn nao_sobe_app_limited() {
        let mut b = Bwe::new(BweConfig::default());
        // 100 kbps contra alvo de 1000.
        rodar(&mut b, 10, 100_000, |_| 20_000);
        assert!(b.estimate_kbps() <= 1000, "estimate = {}", b.estimate_kbps());
    }

    #[test]
    fn perda_alta() {
        let mut b = Bwe::new(BweConfig::default());
        let mut t = 0;
        for i in 0..400u64 {
            t = i * 10_000;
            b.on_packet(t, t, 1250);
            b.on_loss_fraction(t, 0.3);
            b.update(t);
        }
        assert!(b.estimate_kbps() < 1000);
        assert!(b.estimate_kbps() >= 150);
        let _ = t;
    }

    #[test]
    fn clamp_max() {
        let cfg = BweConfig {
            min_kbps: 150,
            max_kbps: 1200,
            start_kbps: 1000,
        };
        let mut b = Bwe::new(cfg);
        rodar(&mut b, 20, 10_000, |_| 20_000);
        assert!(b.estimate_kbps() <= 1200);
    }

    #[test]
    fn descarte_do_emissor_prende_a_estimativa() {
        // Chega 1 Mbps sem fila nenhuma, mas o emissor está descartando quadros: a estimativa não
        // pode subir para o dobro (limite de "app-limited"); fica em ~0,95 × o que chega.
        let mut b = Bwe::new(BweConfig {
            min_kbps: 150,
            max_kbps: 50_000,
            start_kbps: 1_000,
        });
        for i in 0..1000u64 {
            let t = i * 10_000;
            b.on_packet(t, t + 20_000, 1250);
            if i % 10 == 0 {
                b.on_frame_dropped(t + 20_000);
            }
            if i % 5 == 0 {
                b.update(t + 20_000);
            }
        }
        assert!(b.estimate_kbps() <= 1_000, "estimate = {}", b.estimate_kbps());
        assert!(b.estimate_kbps() >= 850, "estimate = {}", b.estimate_kbps());
    }

    #[test]
    fn descarte_com_estimativa_baixa_sobe_ate_o_que_chega() {
        // Começa em 300 kbps, mas chegam 1 Mbps com descartes: sobe para ~0,95 Mbps.
        let mut b = Bwe::new(BweConfig {
            min_kbps: 150,
            max_kbps: 50_000,
            start_kbps: 300,
        });
        for i in 0..300u64 {
            let t = i * 10_000;
            b.on_packet(t, t + 20_000, 1250);
            if i % 10 == 0 {
                b.on_frame_dropped(t + 20_000);
            }
            if i % 5 == 0 {
                b.update(t + 20_000);
            }
        }
        assert!(
            (850..=1_000).contains(&b.estimate_kbps()),
            "estimate = {}",
            b.estimate_kbps()
        );
    }

    /// Vídeo de 400 kbps (1250 B a cada 25 ms) com update a cada 50 ms, de `ini` a `fim` (µs).
    /// Devolve o menor estimate visto nos updates.
    fn video_400(b: &mut Bwe, ini: u64, fim: u64) -> u32 {
        let mut minimo = u32::MAX;
        let mut t = ini;
        while t < fim {
            b.on_packet(t, t + 20_000, 1250);
            if t % 50_000 == 0 {
                minimo = minimo.min(b.update(t + 20_000));
            }
            t += 25_000;
        }
        minimo
    }

    #[test]
    fn bwe_sonda_sobe_para_camada_1() {
        let mut b = Bwe::new(BweConfig::default());
        video_400(&mut b, 0, 5_000_000);
        assert!(b.estimate_kbps() <= 1600, "estimate = {}", b.estimate_kbps());
        // Grupo de 200 ms a ~3,5 Mbps.
        let ini = 5_000_000u64;
        let mut t = ini;
        while t < ini + 200_000 {
            b.on_probe_packet(t, t + 20_000, 1200);
            t += 2_700;
        }
        let now = t + 20_000;
        b.on_probe_cluster(now, 3750);
        assert!(b.estimate_kbps() >= 3195, "estimate = {}", b.estimate_kbps());
        let mut minimo = u32::MAX;
        // 150 ms seguintes, depois 3 s só com o vídeo.
        let mut s = now;
        while s < now + 3_150_000 {
            if s < now + 150_000 {
                minimo = minimo.min(b.update(s));
            } else {
                b.on_packet(s, s + 20_000, 1250);
                minimo = minimo.min(b.update(s + 20_000));
            }
            s += 25_000;
        }
        assert!(minimo >= 3195, "minimo = {minimo}");
    }

    #[test]
    fn bwe_sonda_com_perda_alta_ignorada() {
        let mut b = Bwe::new(BweConfig::default());
        video_400(&mut b, 0, 2_000_000);
        let antes = b.estimate_kbps();
        b.on_loss_fraction(2_000_000, 0.3);
        b.on_probe_cluster(2_000_000, 4000);
        assert_eq!(b.estimate_kbps(), antes);
    }

    #[test]
    fn bwe_sonda_cluster_nao_reduz() {
        let mut b = Bwe::new(BweConfig::default());
        video_400(&mut b, 0, 2_000_000);
        let antes = b.estimate_kbps();
        b.on_probe_cluster(2_000_000, 500);
        assert_eq!(b.estimate_kbps(), antes);
    }

    #[test]
    fn bwe_carencia_ignora_overuse_e_descarte() {
        let mut b = Bwe::new(BweConfig::default());
        video_400(&mut b, 0, 3_000_000);
        let antes = b.estimate_kbps();
        // Atraso crescente (quadrático) dentro de 300 ms da última sonda.
        let base = 3_000_000u64;
        let mut ult = 0;
        let mut ultima_chegada = 0;
        for i in 0..28u64 {
            let envio = base + i * 10_000;
            let chegada = (envio + 20_000 + i * i * 400).max(ult);
            ult = chegada;
            ultima_chegada = chegada;
            b.on_probe_packet(envio, chegada, 1250);
            assert_eq!(b.state(), DelayState::Normal, "i = {i}");
            b.update(chegada);
            assert!(b.estimate_kbps() >= antes, "i = {i}: {}", b.estimate_kbps());
        }
        for k in 0..3 {
            b.on_frame_dropped(ultima_chegada + k * 1000);
        }
        assert!(b.descartes.is_empty());
        // Depois da carência, descarte volta a contar.
        let depois = ultima_chegada + CARENCIA_MAX_US + 1_000;
        for k in 0..3 {
            b.on_frame_dropped(depois + k * 1000);
        }
        assert_eq!(b.descartes.len(), 3);
        assert!(!b.em_carencia(depois));
    }

    /// Bwe aquecido com vídeo de 400 kbps por 3 s e uma sonda em t0 = 3 s (atraso base de 20 ms).
    fn bwe_com_sonda() -> (Bwe, u64) {
        let mut b = Bwe::new(BweConfig::default());
        video_400(&mut b, 0, 3_000_000);
        let t0 = 3_000_000u64;
        b.on_probe_packet(t0, t0 + 20_000, 1200);
        (b, t0 + 20_000)
    }

    #[test]
    fn descarte_visto_na_carencia_ignorado() {
        // Visto dentro da carência: ignorado, mesmo processado 300 ms depois.
        let (mut b, t0) = bwe_com_sonda();
        let antes = b.estimate_kbps();
        video_400(&mut b, 3_025_000, 3_300_000);
        for k in 0..3 {
            b.on_frame_dropped(t0 + 100_000 + k * 1000);
        }
        b.update(t0 + 300_000);
        assert!(b.estimate_kbps() >= antes, "estimate = {}", b.estimate_kbps());

        // Visto fora da carência: conta e puxa a estimativa para 0,95 x vazão.
        let (mut b, t0) = bwe_com_sonda();
        let antes = b.estimate_kbps();
        video_400(&mut b, 3_025_000, 5_300_000);
        for k in 0..3 {
            b.on_frame_dropped(t0 + 2_000_000 + k * 1000);
        }
        b.update(t0 + 2_300_000);
        assert!(b.estimate_kbps() < antes, "estimate = {}", b.estimate_kbps());
        assert!(b.estimate_kbps() <= 500, "estimate = {}", b.estimate_kbps());
    }

    #[test]
    fn carencia_fecha_em_1s_com_fila() {
        let mut b = Bwe::new(BweConfig::default());
        video_400(&mut b, 0, 3_000_000);
        // Sonda com atraso de 120 ms (fila de ~100 ms) e depois o atraso fica parado.
        let t0 = 3_000_000u64;
        let sonda = t0 + 120_000;
        b.on_probe_packet(t0, sonda, 1200);
        let (mut dentro, mut fora) = (0, 0);
        for i in 1..200u64 {
            let envio = t0 + i * 10_000;
            let chegada = envio + 120_000;
            b.on_packet(envio, chegada, 1250);
            if chegada <= sonda + 300_000 {
                continue;
            }
            if chegada <= sonda + CARENCIA_MAX_US {
                assert!(b.em_carencia(chegada), "chegada = {chegada}");
                dentro += 1;
            } else {
                assert!(!b.em_carencia(chegada), "chegada = {chegada}");
                fora += 1;
            }
        }
        assert!(dentro > 10 && fora > 10, "dentro = {dentro}, fora = {fora}");
    }

    #[test]
    fn keyframe_start_abre_carencia() {
        let rodar_atraso = |com_keyframe: bool| {
            let mut b = Bwe::new(BweConfig::default());
            video_400(&mut b, 0, 3_000_000);
            let t0 = 3_000_000u64;
            if com_keyframe {
                b.on_keyframe_start(t0);
            }
            let mut ult = 0;
            let mut viu_overuse = false;
            for i in 0..40u64 {
                let envio = t0 + i * 10_000;
                let chegada = (envio + 20_000 + i * i * 400).max(ult);
                ult = chegada;
                b.on_packet(envio, chegada, 1250);
                if com_keyframe && b.detector.fila_us() > FILA_ESCOADA_US && chegada - t0 <= CARENCIA_MAX_US {
                    assert_ne!(b.state(), DelayState::Overuse, "i = {i}");
                }
                viu_overuse |= b.state() == DelayState::Overuse;
            }
            viu_overuse
        };
        rodar_atraso(true);
        assert!(rodar_atraso(false), "sem keyframe deveria dar Overuse");
    }

    /// Bwe aquecido (vídeo de 400 kbps por 3 s) com uma sobrecarga do detector que começou num
    /// keyframe em `t0`, dentro da carência, e cuja fila já escoou.
    fn bwe_overuse_do_keyframe() -> (Bwe, u64) {
        let mut b = Bwe::new(BweConfig::default());
        video_400(&mut b, 0, 3_000_000);
        let t0 = 3_000_000u64;
        b.on_keyframe_start(t0);
        b.state = DelayState::Overuse;
        b.overuse_desde = Some((t0 + 100_000, true));
        assert!(b.detector.fila_us() < FILA_ESCOADA_US);
        assert!(!b.em_carencia(t0 + 200_000));
        (b, t0)
    }

    #[test]
    fn overuse_nascido_na_carencia_ignorado_ate_1s() {
        let (mut b, t0) = bwe_overuse_do_keyframe();
        let antes = b.estimate_kbps();
        b.now_chegada = t0 + 300_000;
        assert_eq!(b.state(), DelayState::Normal);
        b.update(t0 + 300_000);
        assert!(b.estimate_kbps() >= antes, "estimate = {}", b.estimate_kbps());
        // Passou 1 s do keyframe: é sobrecarga de verdade.
        b.now_chegada = t0 + 1_100_001;
        assert_eq!(b.state(), DelayState::Overuse);
    }

    #[test]
    fn overuse_nascido_fora_da_carencia_conta() {
        let (mut b, t0) = bwe_overuse_do_keyframe();
        b.overuse_desde = Some((t0 + 100_000, false));
        b.now_chegada = t0 + 300_000;
        assert_eq!(b.state(), DelayState::Overuse);
        let antes = b.estimate_kbps();
        b.update(t0 + 300_000);
        assert!(b.estimate_kbps() < antes, "estimate = {}", b.estimate_kbps());
    }

    #[test]
    fn overuse_longo_conta_mesmo_nascido_na_carencia() {
        let (mut b, t0) = bwe_overuse_do_keyframe();
        // Um keyframe novo não estende uma sobrecarga que já dura mais de 1 s.
        b.on_keyframe_start(t0 + 1_000_000);
        b.now_chegada = t0 + 1_200_000;
        assert_eq!(b.state(), DelayState::Overuse);
    }

    #[test]
    fn carencia_fecha_quando_fila_escoa() {
        let (mut b, t0) = bwe_com_sonda();
        video_400(&mut b, 3_025_000, 3_500_000);
        assert!(b.detector.fila_us() < FILA_ESCOADA_US);
        assert!(b.em_carencia(t0 + CARENCIA_SONDA_MIN_US));
        assert!(!b.em_carencia(t0 + CARENCIA_SONDA_MIN_US + 1));
        assert!(!b.em_carencia(t0 + 400_000));
    }

    #[test]
    fn bwe_sonda_acima_da_capacidade_nao_derruba() {
        use crate::sim::{Link, LinkConfig};
        let mut link = Link::new(
            LinkConfig {
                base_delay_ms: 20.0,
                jitter_ms: 0.0,
                loss: 0.0,
                queue_bytes: 50_000,
            },
            7,
        );
        link.set_capacity_kbps(0, 4000);
        // Teto na capacidade do enlace: sem ele a fase inicial segue crescendo por conta própria.
        let mut b = Bwe::new(BweConfig {
            max_kbps: 4000,
            ..BweConfig::default()
        });
        // (chegada, é_sonda, envio, bytes)
        let mut pend: Vec<(u64, bool, u64, u32)> = Vec::new();
        let burst_ini = 8_000_000u64;
        let burst_fim = burst_ini + 200_000;
        let fim = burst_fim + 1_000_000;
        let mut e0 = 0;
        let mut primeira_sonda: Option<u64> = None;
        let mut ultima_sonda = 0u64;
        let mut t = 0u64;
        while t <= fim {
            if t % 4_000 == 0 {
                if let Some(c) = link.send(t, 1250) {
                    pend.push((c, false, t, 1250));
                }
            }
            if (burst_ini..burst_fim).contains(&t) {
                // 1200 B por ms = 9600 kbps de sondas.
                if let Some(c) = link.send(t, 1200) {
                    pend.push((c, true, t, 1200));
                }
            }
            pend.sort();
            let n = pend.iter().take_while(|p| p.0 <= t).count();
            for (c, sonda, envio, bytes) in pend.drain(..n) {
                if sonda {
                    b.on_probe_packet(envio, c, bytes);
                    primeira_sonda.get_or_insert(c);
                    ultima_sonda = ultima_sonda.max(c);
                } else {
                    b.on_packet(envio, c, bytes);
                }
                if let Some(p) = primeira_sonda {
                    if c <= ultima_sonda.max(burst_fim + 200_000) + CARENCIA_SONDA_MIN_US {
                        let _ = p;
                        assert_ne!(b.state(), DelayState::Overuse, "t = {c}");
                    }
                }
            }
            if t % 50_000 == 0 {
                b.update(t);
            }
            if t == burst_ini {
                e0 = b.estimate_kbps();
            }
            if t == burst_fim {
                b.on_probe_cluster(t, 4000);
            }
            t += 1_000;
        }
        let e = b.estimate_kbps();
        assert!(e >= e0.min(2639), "e = {e}, e0 = {e0}");
        assert!(e <= e0.max(4180), "e = {e}, e0 = {e0}");
    }
}
