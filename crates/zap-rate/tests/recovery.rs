//! Teste de aceite: volta de banda (tarefa rate-16).
//!
//! Enlace cai para 3 Mbps em t=20 s e volta para 8 Mbps em t=40 s. Depois da volta, a
//! estimativa precisa passar de 6 Mbps em até 15 s.

mod common;

use common::{run, Scenario};
use zap_rate::sim::LinkConfig;

const CAPACIDADE_INICIAL_KBPS: u32 = 8_000;
/// Instante em que o enlace volta para a capacidade inicial (µs).
const VOLTA_US: u64 = 40_000_000;
const PRAZO_US: u64 = 15_000_000;
const LIMIAR_KBPS: u32 = 6_000;

#[test]
fn volta_3_para_8_mbps() {
    let cenario = Scenario {
        link: LinkConfig {
            base_delay_ms: 30.0,
            jitter_ms: 2.0,
            loss: 0.0,
            queue_bytes: 100_000,
        },
        // (0, 8000) fixa o enlace inicial em 8 Mbps; depois cai para 3 Mbps e volta.
        capacity_schedule: vec![(0, CAPACIDADE_INICIAL_KBPS), (20_000_000, 3_000), (40_000_000, 8_000)],
        start_kbps: 1_000,
        max_kbps: 50_000,
        duration_s: 65,
        seed: 0x5EED_0016,
    };
    let trace = run(&cenario);

    let limite_us = VOLTA_US + PRAZO_US;
    let t_volta = trace.first_time(|s| s.t_us > VOLTA_US && s.t_us <= limite_us && s.estimate_kbps > LIMIAR_KBPS);
    assert!(
        t_volta.is_some(),
        "estimativa nunca passou de {LIMIAR_KBPS} kbps entre 40 s e 55 s; amostras: {:?}",
        trace
            .samples
            .iter()
            .filter(|s| s.t_us > VOLTA_US)
            .map(|s| (s.t_us, s.estimate_kbps))
            .collect::<Vec<_>>()
    );
}
