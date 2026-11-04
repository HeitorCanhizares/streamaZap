//! Relógio de reprodução / suavização.
#![allow(unused_variables, dead_code)]

use crate::window::{SlidingMin, SlidingQuantile};
use crate::Us;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Video,
    Audio,
}

#[derive(Debug, Clone, Copy)]
pub struct PlayoutConfig {
    pub auto: bool,
    /// Padrão 200.
    pub manual_delay_ms: u32,
    /// Padrão 1500.
    pub max_delay_ms: u32,
    pub frame_interval_ms: f64,
}
impl Default for PlayoutConfig {
    fn default() -> Self {
        Self {
            auto: true,
            manual_delay_ms: 200,
            max_delay_ms: 1500,
            frame_interval_ms: 1000.0 / 60.0,
        }
    }
}

/// Janela do trânsito mínimo (base): 10 s.
const JANELA_BASE_US: Us = 10_000_000;
/// Janela do jitter: 5 s.
const JANELA_JITTER_US: Us = 5_000_000;
/// Tempo mínimo com alvo abaixo do atraso antes de começar a descer: 5 s.
const ESPERA_DESCIDA_US: Us = 5_000_000;
/// Velocidades da rampa em ms/s.
const VEL_SUBIDA: f64 = 20.0;
const VEL_DESCIDA: f64 = 15.0;
/// Folga acima do alvo a partir da qual o atraso pula direto para o alvo.
const FOLGA_PULO_MS: f64 = 300.0;

#[derive(Debug, Clone)]
pub struct PlayoutClock {
    cfg: PlayoutConfig,
    /// Mínimo do trânsito (relógios distintos: pode ser negativo).
    transito_min: SlidingMin<i64>,
    jitter: SlidingQuantile,
    base: Option<i64>,
    /// Atraso do modo automático, em ms.
    atraso: f64,
    /// Velocidade atual da rampa em ms/s (+20, -15 ou 0).
    vel: f64,
    abaixo_desde: Option<Us>,
    ultimo_tick: Option<Us>,
    ultimo_now: Us,
}
impl PlayoutClock {
    pub fn new(cfg: PlayoutConfig) -> Self {
        Self {
            cfg,
            transito_min: SlidingMin::new(JANELA_BASE_US),
            jitter: SlidingQuantile::new(JANELA_JITTER_US),
            base: None,
            atraso: cfg.frame_interval_ms + 5.0,
            vel: 0.0,
            abaixo_desde: None,
            ultimo_tick: None,
            ultimo_now: 0,
        }
    }

    fn max_ms(&self) -> f64 {
        f64::from(self.cfg.max_delay_ms)
    }

    /// Alvo automático: p95 do jitter + intervalo de quadro + folga.
    fn alvo(&self) -> f64 {
        let fi = self.cfg.frame_interval_ms;
        let p95 = self.jitter.quantile(0.95).unwrap_or(0.0);
        // max/min em vez de clamp: não entra em pânico se max < fi.
        (p95 + fi + 5.0).min(self.max_ms()).max(fi)
    }

    /// Atraso efetivamente usado (manual ignora a rampa).
    fn atraso_uso(&self) -> f64 {
        if self.cfg.auto {
            self.atraso
        } else {
            f64::from(self.cfg.manual_delay_ms)
        }
    }

    pub fn observe(&mut self, now: Us, capture_us: Us, kind: MediaKind) {
        let transito = now as i64 - capture_us as i64;
        // Instante previsto antes de esta amostra mexer na base.
        let previsto = self.play_time(capture_us);
        let base = self.transito_min.push(now, transito);
        self.base = Some(base);
        let jitter_ms = (transito - base) as f64 / 1000.0;
        self.jitter.push(now, jitter_ms);
        self.ultimo_now = now;

        match previsto {
            None => {
                // Primeira observação: parte do alvo inicial.
                self.atraso = self.cfg.frame_interval_ms + 5.0;
                self.vel = 0.0;
            }
            Some(pt) if self.cfg.auto && now > pt => {
                // Mídia atrasada: sobe o bastante para cobrir e zera a rampa de descida.
                let falta_ms = (now - pt) as f64 / 1000.0;
                self.atraso = (self.atraso + falta_ms).min(self.max_ms());
                self.abaixo_desde = None;
                self.vel = 0.0;
            }
            Some(_) => {}
        }
    }

    pub fn play_time(&self, capture_us: Us) -> Option<Us> {
        let base = self.base?;
        let atraso_us = (self.atraso_uso() * 1000.0) as i64;
        let t = (capture_us as i64).saturating_add(base).saturating_add(atraso_us);
        Some(t.max(0) as u64)
    }

    pub fn audio_rate(&self) -> f64 {
        if !self.cfg.auto {
            return 1.0;
        }
        (1.0 + self.vel / 1000.0).clamp(0.98, 1.02)
    }

    pub fn current_delay_ms(&self) -> f64 {
        self.atraso_uso()
    }

    pub fn tick(&mut self, now: Us) {
        let dt_s = match self.ultimo_tick {
            Some(t) => (now.saturating_sub(t) as f64 / 1e6).min(0.1),
            None => 0.0,
        };
        self.ultimo_tick = Some(now);
        if !self.cfg.auto {
            self.vel = 0.0;
            return;
        }
        let alvo = self.alvo();
        if alvo > self.atraso {
            self.abaixo_desde = None;
            self.atraso += (alvo - self.atraso).min(VEL_SUBIDA * dt_s);
            self.vel = VEL_SUBIDA;
        } else if alvo < self.atraso {
            let desde = *self.abaixo_desde.get_or_insert(now);
            if now.saturating_sub(desde) >= ESPERA_DESCIDA_US {
                self.atraso -= (self.atraso - alvo).min(VEL_DESCIDA * dt_s);
                self.vel = -VEL_DESCIDA;
            } else {
                self.vel = 0.0;
            }
        } else {
            self.abaixo_desde = None;
            self.vel = 0.0;
        }
        if self.atraso > alvo + FOLGA_PULO_MS {
            self.atraso = alvo;
            self.vel = 0.0;
        }
    }

    pub fn set_auto(&mut self, auto: bool) {
        if auto && !self.cfg.auto {
            // Parte do atraso que estava em uso (o manual).
            self.atraso = f64::from(self.cfg.manual_delay_ms);
            self.vel = 0.0;
            self.abaixo_desde = None;
        }
        self.cfg.auto = auto;
    }

    pub fn set_manual_delay(&mut self, ms: u32) {
        self.cfg.manual_delay_ms = ms;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME_US: Us = 16_667;

    fn estavel(c: &mut PlayoutClock, n: u64, ini: Us) -> Us {
        let mut cap = ini;
        for _ in 0..n {
            cap += FRAME_US;
            c.observe(cap + 40_000, cap, MediaKind::Video);
            c.tick(cap + 40_000);
        }
        cap
    }

    #[test]
    fn auto_enlace_estavel() {
        let mut c = PlayoutClock::new(PlayoutConfig::default());
        estavel(&mut c, 300, 0);
        let fi = 1000.0 / 60.0;
        let d = c.current_delay_ms();
        assert!(d >= fi + 5.0 - 0.01 && d <= fi + 10.0, "atraso = {d}");
    }

    #[test]
    fn manual() {
        let mut c = PlayoutClock::new(PlayoutConfig::default());
        c.set_auto(false);
        c.set_manual_delay(200);
        assert_eq!(c.current_delay_ms(), 200.0);
        assert_eq!(c.audio_rate(), 1.0);
    }

    #[test]
    fn sobe_rapido_com_atraso() {
        let mut c = PlayoutClock::new(PlayoutConfig::default());
        let cap = estavel(&mut c, 100, 0) + FRAME_US;
        let antes = c.current_delay_ms();
        c.observe(cap + 140_000, cap, MediaKind::Video);
        let d = c.current_delay_ms();
        assert!(d > antes + 70.0, "antes {antes} depois {d}");
        let pt = c.play_time(cap).unwrap_or(0);
        assert!(pt + 1000 >= cap + 140_000, "pt = {pt}");
    }

    #[test]
    fn desce_devagar() {
        let mut c = PlayoutClock::new(PlayoutConfig::default());
        let cap = estavel(&mut c, 100, 0) + FRAME_US;
        let t0 = cap + 140_000;
        c.observe(t0, cap, MediaKind::Video);
        let alto = c.current_delay_ms();
        // Dentro dos 5 s de espera: não desce.
        let mut t = t0;
        while t < t0 + 4_900_000 {
            t += 100_000;
            c.tick(t);
        }
        assert!((c.current_delay_ms() - alto).abs() < 1e-9);
        // Depois, desce, no máximo 15 ms/s.
        let ref_t = t;
        let ref_d = c.current_delay_ms();
        while t < ref_t + 2_000_000 {
            t += 100_000;
            c.tick(t);
        }
        let d = c.current_delay_ms();
        assert!(d < ref_d, "não desceu: {d}");
        assert!(ref_d - d <= 15.0 * 2.0 + 1e-6, "desceu rápido: {}", ref_d - d);
    }

    #[test]
    fn pula_para_alvo() {
        let mut c = PlayoutClock::new(PlayoutConfig::default());
        c.set_auto(false);
        c.set_manual_delay(1000);
        c.set_auto(true);
        c.tick(1_000_000);
        let alvo = 1000.0 / 60.0 + 5.0;
        assert!((c.current_delay_ms() - alvo).abs() < 1e-6);
    }

    #[test]
    fn audio_rate_em_faixa() {
        let mut c = PlayoutClock::new(PlayoutConfig::default());
        let mut cap = 0;
        for i in 0..400u64 {
            cap += FRAME_US;
            let extra = if i % 37 == 0 { 120_000 } else { 0 };
            c.observe(cap + 40_000 + extra, cap, MediaKind::Audio);
            c.tick(cap + 40_000);
            let r = c.audio_rate();
            assert!((0.98..=1.02).contains(&r), "taxa = {r}");
        }
    }

    #[test]
    fn play_time_none_antes_de_observar() {
        let c = PlayoutClock::new(PlayoutConfig::default());
        assert_eq!(c.play_time(1000), None);
    }
}
