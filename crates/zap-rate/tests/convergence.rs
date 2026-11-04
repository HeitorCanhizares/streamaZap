//! Teste de aceite: convergencia (tarefa rate-14).
//!
//! Enlace de 8 Mbps: a estimativa do `Bwe` precisa achar a capacidade em até 10 s e ficar
//! nela até o fim, com fila média baixa (sem inchar o atraso).

mod common;

use common::{run, Scenario};
use zap_rate::sim::LinkConfig;

const CAPACIDADE_KBPS: u32 = 8_000;
// O AIMD oscila em dente de serra entre ~0,85 e ~1,05 × a capacidade (reduz a 0,85 × a vazão na
// sobrecarga e sobe até a fila voltar a crescer): a faixa aceita é 0,8–1,1 × a capacidade.
const FAIXA_MIN_KBPS: u32 = 6_400;
const FAIXA_MAX_KBPS: u32 = 8_800;
/// Fração máxima de amostras fora da faixa depois de 10 s.
const TOLERANCIA_FORA: f64 = 0.05;
/// Fila média máxima entre 10 s e 30 s (ms).
const FILA_MAX_MS: f64 = 60.0;

#[test]
fn convergencia_8mbps() {
    let cenario = Scenario {
        link: LinkConfig {
            base_delay_ms: 30.0,
            jitter_ms: 2.0,
            loss: 0.0,
            queue_bytes: 100_000,
        },
        capacity_schedule: vec![(0, CAPACIDADE_KBPS)],
        start_kbps: 1_000,
        max_kbps: 50_000,
        duration_s: 30,
        seed: 0x5EED_0014,
    };
    let trace = run(&cenario);

    // Até 10 s, em algum instante a estimativa cai dentro da faixa.
    let convergiu = trace
        .samples
        .iter()
        .filter(|s| s.t_us <= 10_000_000)
        .any(|s| (FAIXA_MIN_KBPS..=FAIXA_MAX_KBPS).contains(&s.estimate_kbps));
    assert!(
        convergiu,
        "estimativa nunca entrou em [{FAIXA_MIN_KBPS}, {FAIXA_MAX_KBPS}] kbps até 10 s; \
         amostras: {:?}",
        trace
            .samples
            .iter()
            .filter(|s| s.t_us <= 10_000_000)
            .map(|s| s.estimate_kbps)
            .collect::<Vec<_>>()
    );

    // De 10 s a 30 s, a estimativa permanece na faixa (tolerando 2% das amostras fora).
    let pos_10s: Vec<_> = trace
        .samples
        .iter()
        .filter(|s| s.t_us >= 10_000_000 && s.t_us < 30_000_000)
        .collect();
    assert!(!pos_10s.is_empty(), "trilha sem amostras entre 10 s e 30 s");
    let fora = pos_10s
        .iter()
        .filter(|s| !(FAIXA_MIN_KBPS..=FAIXA_MAX_KBPS).contains(&s.estimate_kbps))
        .count();
    let fracao_fora = fora as f64 / pos_10s.len() as f64;
    assert!(
        fracao_fora <= TOLERANCIA_FORA,
        "{fora} de {} amostras ({:.1}%) fora de [{FAIXA_MIN_KBPS}, {FAIXA_MAX_KBPS}] kbps entre \
         10 s e 30 s (limite {:.0}%)",
        pos_10s.len(),
        fracao_fora * 100.0,
        TOLERANCIA_FORA * 100.0
    );

    // Fila média entre 10 s e 30 s.
    let fila = trace.mean_queue_ms(10.0, 30.0);
    assert!(
        fila < FILA_MAX_MS,
        "fila média de {fila:.1} ms entre 10 s e 30 s (limite {FILA_MAX_MS} ms)"
    );
}
