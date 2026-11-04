//! Estimador de banda (detector de atraso + AIMD + perda).
#![allow(dead_code)]

use crate::trendline::DelayDetector;
use crate::window::{LossMeter, RateMeter};
use crate::Us;
use zap_proto::DelayState;

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
        }
    }
    pub fn on_packet(&mut self, send_us: Us, arrival_us: Us, bytes: u32) {
        self.state = self.detector.on_packet(send_us, arrival_us, bytes);
        self.rate.add(arrival_us, bytes);
        self.primeiro_pacote.get_or_insert(arrival_us);
        self.now_chegada = arrival_us;
    }
    /// Um quadro de vídeo que o emissor descartou (o receptor vê um buraco de `seq`).
    pub fn on_frame_dropped(&mut self, now: Us) {
        self.descartes.push_back(now);
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

        match self.state {
            _ if descartando => {
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
            DelayState::Overuse => {
                if passou_carencia && separado(self.ultima_reducao, ESPACO_REDUCAO_US) {
                    self.alvo = self.alvo.min(FATOR_OVERUSE * vazao);
                    self.ultima_reducao = Some(now);
                    self.vazao_na_sobrecarga = Some(vazao);
                    self.ultima_sobrecarga = Some(now);
                    self.fase_inicial = false;
                }
            }
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
    pub fn estimate_kbps(&self) -> u32 {
        self.estimate_kbps
    }
    pub fn state(&self) -> DelayState {
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
}
