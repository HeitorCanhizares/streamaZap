//! Teste de aceite: perda aleatória de 1% (tarefa rate-16).
//!
//! Enlace de 20 Mbps com 1% de perda aleatória, fonte limitada a 12 Mbps (sem gargalo).
//! Depois de atingir o pico M, a estimativa não pode cair abaixo de 0.85*M, tolerando 2% das
//! amostras fora da faixa.

mod common;

use common::{run, Scenario};
use zap_rate::sim::LinkConfig;

const CAPACIDADE_KBPS: u32 = 20_000;
const MAX_KBPS: u32 = 12_000;
const START_KBPS: u32 = 1_000;
/// Fração de M abaixo da qual a estimativa não pode cair depois do pico.
const PISO_RELATIVO: f64 = 0.85;
/// Fração máxima de amostras fora de 0.85*M depois do pico.
const TOLERANCIA_FORA: f64 = 0.02;

#[test]
fn perda_aleatoria_1pc() {
    let cenario = Scenario {
        link: LinkConfig {
            base_delay_ms: 30.0,
            jitter_ms: 2.0,
            loss: 0.01,
            queue_bytes: 100_000,
        },
        capacity_schedule: vec![(0, CAPACIDADE_KBPS)],
        start_kbps: START_KBPS,
        max_kbps: MAX_KBPS,
        duration_s: 40,
        seed: 0x5EED_0017,
    };
    let trace = run(&cenario);

    let pico = trace.samples.iter().map(|s| s.estimate_kbps).max().unwrap_or(0);
    let t_pico = trace.first_time(|s| s.estimate_kbps == pico).unwrap_or(0);
    let piso = PISO_RELATIVO * f64::from(pico);

    let depois: Vec<_> = trace.samples.iter().filter(|s| s.t_us >= t_pico).collect();
    assert!(!depois.is_empty(), "trilha sem amostras depois do pico");
    let fora = depois.iter().filter(|s| f64::from(s.estimate_kbps) < piso).count();
    let fracao_fora = fora as f64 / depois.len() as f64;
    assert!(
        fracao_fora <= TOLERANCIA_FORA,
        "pico M={pico} kbps em {:.1} s; {fora} de {} amostras ({:.1}%) abaixo de {piso:.0} kbps \
         (0.85*M) depois do pico (limite {:.0}%); valores: {:?}",
        t_pico as f64 / 1e6,
        depois.len(),
        fracao_fora * 100.0,
        TOLERANCIA_FORA * 100.0,
        depois.iter().map(|s| s.estimate_kbps).collect::<Vec<_>>()
    );
}
