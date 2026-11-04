//! Teste de aceite: jitter (tarefa rate-14..18).
//!
//! Enlace com 30 ms de atraso base e 20 ms de desvio de jitter. O `PlayoutClock` automático
//! precisa estabilizar o atraso de reprodução: quase nenhum quadro chega depois da hora de
//! exibir e o atraso fica numa faixa razoável.

use zap_rate::jitter::{MediaKind, PlayoutClock, PlayoutConfig};
use zap_rate::sim::{Link, LinkConfig};

const INTERVALO_QUADRO_US: u64 = 16_667;
const DURACAO_US: u64 = 60_000_000;
const AQUECIMENTO_US: u64 = 10_000_000;
const PASSO_TICK_US: u64 = 10_000;
const BYTES_QUADRO: u32 = 5000;
const MAX_FRACAO_ATRASADOS: f64 = 0.01;
const ATRASO_MEDIO_MIN_MS: f64 = 35.0;
const ATRASO_MEDIO_MAX_MS: f64 = 80.0;
const ATRASO_MAX_MS: f64 = 150.0;

#[test]
fn jitter_20ms_estabiliza() {
    let mut link = Link::new(
        LinkConfig {
            base_delay_ms: 30.0,
            jitter_ms: 20.0,
            loss: 0.0,
            queue_bytes: 100_000_000,
        },
        7,
    );
    link.set_capacity_kbps(0, 50_000);

    // Gera (chegada, captura) de todos os quadros e ordena por chegada.
    let mut chegadas: Vec<(u64, u64)> = Vec::new();
    let mut n = 0u64;
    while n * INTERVALO_QUADRO_US < DURACAO_US {
        let captura = n * INTERVALO_QUADRO_US;
        if let Some(arrival) = link.send(captura, BYTES_QUADRO) {
            chegadas.push((arrival, captura));
        }
        n += 1;
    }
    chegadas.sort_unstable();

    let mut clock = PlayoutClock::new(PlayoutConfig {
        frame_interval_ms: 1000.0 / 60.0,
        ..PlayoutConfig::default()
    });

    let mut idx = 0;
    let mut proximo_tick = 0u64;
    let mut total = 0u64;
    let mut atrasados = 0u64;
    let mut soma_atraso = 0.0;
    let mut amostras = 0u64;
    let mut max_atraso = 0.0f64;

    // Ordena eventos por tempo: chegadas e ticks de 10 ms.
    while proximo_tick <= DURACAO_US + 1_000_000 {
        while idx < chegadas.len() && chegadas[idx].0 <= proximo_tick {
            let (arrival, captura) = chegadas[idx];
            if arrival >= AQUECIMENTO_US {
                if let Some(pt) = clock.play_time(captura) {
                    total += 1;
                    if arrival > pt {
                        atrasados += 1;
                    }
                }
            }
            clock.observe(arrival, captura, MediaKind::Video);
            idx += 1;
        }
        clock.tick(proximo_tick);
        if proximo_tick >= AQUECIMENTO_US {
            let d = clock.current_delay_ms();
            soma_atraso += d;
            amostras += 1;
            max_atraso = max_atraso.max(d);
        }
        proximo_tick += PASSO_TICK_US;
    }

    let fracao = atrasados as f64 / total as f64;
    let medio = soma_atraso / amostras as f64;
    println!("atrasados={atrasados}/{total} ({fracao:.4}) atraso medio={medio:.2} ms max={max_atraso:.2} ms");

    assert!(fracao < MAX_FRACAO_ATRASADOS, "fracao de atrasados {fracao}");
    assert!(
        (ATRASO_MEDIO_MIN_MS..=ATRASO_MEDIO_MAX_MS).contains(&medio),
        "atraso medio {medio} ms"
    );
    assert!(max_atraso <= ATRASO_MAX_MS, "atraso maximo {max_atraso} ms");
}
