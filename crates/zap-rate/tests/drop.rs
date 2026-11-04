//! Teste de aceite: queda de banda (tarefa rate-15).
//!
//! Enlace de 8 Mbps que cai para 3 Mbps em t = 20 s. O `Bwe` precisa acusar sobrecarga logo,
//! baixar a estimativa e drenar a fila de volta para perto de zero sem deixar o atraso inchar.

mod common;

use common::{run, Scenario};
use zap_proto::DelayState;
use zap_rate::sim::LinkConfig;

/// Instante da queda de capacidade (µs).
const QUEDA_US: u64 = 20_000_000;
const CAPACIDADE_ANTES_KBPS: u32 = 8_000;
const CAPACIDADE_DEPOIS_KBPS: u32 = 3_000;
/// Sobrecarga detectada em até 700 ms após a queda (µs).
// 200 ms de filtro de rajada (o keyframe coincide com a queda) + 3 grupos + relatório e volta.
const PRAZO_SOBRECARGA_US: u64 = 700_000;
/// Estimativa abaixo deste valor em até 1,5 s após a queda (kbps).
const ESTIMATIVA_MAX_KBPS: u32 = 3_200;
const PRAZO_ESTIMATIVA_US: u64 = 1_500_000;
/// Fila de volta abaixo de 100 ms em até 3 s após a queda (µs).
const FILA_OK_US: u64 = 100_000;
const PRAZO_FILA_US: u64 = 3_000_000;
/// Fração máxima de amostras com fila alta depois que a fila se recupera.
const TOLERANCIA_FORA: f64 = 0.02;
const FIM_US: u64 = 40_000_000;

#[test]
fn queda_8_para_3_mbps() {
    let cenario = Scenario {
        link: LinkConfig {
            base_delay_ms: 30.0,
            jitter_ms: 2.0,
            loss: 0.0,
            queue_bytes: 100_000,
        },
        capacity_schedule: vec![(QUEDA_US, CAPACIDADE_DEPOIS_KBPS)],
        start_kbps: CAPACIDADE_ANTES_KBPS,
        max_kbps: 50_000,
        duration_s: 40,
        seed: 0x5EED_0015,
    };
    let trace = run(&cenario);

    // Sobrecarga (estado) aparece em até 500 ms após t = 20 s.
    let sobrecarga = trace.first_time(|s| s.t_us >= QUEDA_US && s.state == DelayState::Overuse);
    match sobrecarga {
        Some(t) if t <= QUEDA_US + PRAZO_SOBRECARGA_US => {}
        Some(t) => panic!(
            "sobrecarga só em {:.2} s (limite {:.2} s)",
            t as f64 / 1e6,
            (QUEDA_US + PRAZO_SOBRECARGA_US) as f64 / 1e6
        ),
        None => panic!("sobrecarga nunca detectada após a queda"),
    }

    // Estimativa abaixo de 3200 kbps em até 1,5 s após t = 20 s.
    let estimativa = trace.first_time(|s| s.t_us >= QUEDA_US && s.estimate_kbps < ESTIMATIVA_MAX_KBPS);
    match estimativa {
        Some(t) if t <= QUEDA_US + PRAZO_ESTIMATIVA_US => {}
        Some(t) => panic!(
            "estimativa < {ESTIMATIVA_MAX_KBPS} kbps só em {:.2} s (limite {:.2} s)",
            t as f64 / 1e6,
            (QUEDA_US + PRAZO_ESTIMATIVA_US) as f64 / 1e6
        ),
        None => panic!("estimativa nunca ficou abaixo de {ESTIMATIVA_MAX_KBPS} kbps"),
    }

    // A fila precisa subir acima de 100 ms depois da queda e então voltar a ficar abaixo disso.
    let pico = trace.first_time(|s| s.t_us >= QUEDA_US && s.queue_delay_us >= FILA_OK_US);
    let fila_ok = pico.and_then(|p| trace.first_time(|s| s.t_us > p && s.queue_delay_us < FILA_OK_US));
    let t_fila_ok = match fila_ok {
        Some(t) if t <= QUEDA_US + PRAZO_FILA_US => t,
        Some(t) => panic!(
            "fila só voltou a < 100 ms em {:.2} s (limite {:.2} s)",
            t as f64 / 1e6,
            (QUEDA_US + PRAZO_FILA_US) as f64 / 1e6
        ),
        None => panic!("fila nunca voltou a < 100 ms após a queda"),
    };

    // A partir daí, a fila permanece < 100 ms até 40 s (tolerando 2% das amostras fora).
    let pos: Vec<_> = trace
        .samples
        .iter()
        .filter(|s| s.t_us >= t_fila_ok && s.t_us < FIM_US)
        .collect();
    assert!(!pos.is_empty(), "trilha sem amostras entre o retorno da fila e 40 s");
    let fora = pos.iter().filter(|s| s.queue_delay_us >= FILA_OK_US).count();
    let fracao_fora = fora as f64 / pos.len() as f64;
    assert!(
        fracao_fora <= TOLERANCIA_FORA,
        "{fora} de {} amostras ({:.1}%) com fila >= 100 ms entre {:.2} s e 40 s (limite {:.0}%)",
        pos.len(),
        fracao_fora * 100.0,
        t_fila_ok as f64 / 1e6,
        TOLERANCIA_FORA * 100.0
    );
}
