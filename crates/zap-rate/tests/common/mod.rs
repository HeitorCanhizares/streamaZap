//! Harness compartilhado dos testes de aceite (tarefa rate-13).
//!
//! Simulação em tempo virtual (passos de 1 ms): fonte de vídeo -> `Link` -> `Bwe` no receptor,
//! com relatório de perda e estimativa chegando ao emissor 50 ms depois.
#![allow(dead_code)]

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

use zap_proto::DelayState;
use zap_rate::bwe::{Bwe, BweConfig};
use zap_rate::sim::{Link, LinkConfig};
use zap_rate::Us;

/// Passo da simulação (µs).
const PASSO_US: Us = 1_000;
/// Intervalo entre quadros a 60 fps (µs).
const QUADRO_US: Us = 16_667;
/// Tamanho máximo de pacote (bytes).
const MTU: u32 = 1_200;
/// Intervalo entre amostras do `Trace` (µs).
const AMOSTRA_US: Us = 100_000;
/// Intervalo entre relatórios do receptor (µs).
const RELATORIO_US: Us = 100_000;
/// Atraso do canal de retorno (µs).
const RETORNO_US: Us = 50_000;
/// Intervalo das chamadas a `Bwe::update` (µs).
const UPDATE_US: Us = 25_000;
/// Janela da fração de perda reportada (µs).
const JANELA_PERDA_US: Us = 2_000_000;
/// Keyframe a cada 10 s, com 8x o tamanho de um quadro normal.
const KEYFRAME_US: Us = 10_000_000;
const KEYFRAME_FATOR: u64 = 8;

#[derive(Debug, Clone)]
pub struct Scenario {
    pub link: LinkConfig,
    /// (instante, kbps): troca de capacidade do enlace.
    pub capacity_schedule: Vec<(Us, u32)>,
    pub start_kbps: u32,
    pub max_kbps: u32,
    pub duration_s: u32,
    pub seed: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct Sample {
    pub t_us: Us,
    pub estimate_kbps: u32,
    pub state: DelayState,
    pub queue_delay_us: Us,
    /// Pacotes descartados pelo enlace até aqui (acumulado).
    pub loss_events: u32,
}

#[derive(Debug, Clone)]
pub struct Trace {
    pub samples: Vec<Sample>,
}

impl Trace {
    /// Amostra mais próxima de `t_s` segundos (limitada ao intervalo da trilha).
    pub fn at(&self, t_s: f64) -> &Sample {
        let n = self.samples.len();
        let idx = ((t_s * 1e6 / AMOSTRA_US as f64).round().max(0.0) as usize)
            .saturating_sub(1)
            .min(n.saturating_sub(1));
        &self.samples[idx]
    }

    /// Média do atraso de fila (ms) das amostras com `from_s <= t < to_s`.
    pub fn mean_queue_ms(&self, from_s: f64, to_s: f64) -> f64 {
        let (de, ate) = ((from_s * 1e6) as Us, (to_s * 1e6) as Us);
        let (soma, n) = self
            .samples
            .iter()
            .filter(|s| s.t_us >= de && s.t_us < ate)
            .fold((0.0, 0usize), |(a, n), s| (a + s.queue_delay_us as f64 / 1000.0, n + 1));
        if n == 0 {
            0.0
        } else {
            soma / n as f64
        }
    }

    /// Primeiro instante em que `pred` é verdadeiro.
    pub fn first_time(&self, pred: impl Fn(&Sample) -> bool) -> Option<Us> {
        self.samples.iter().find(|s| pred(s)).map(|s| s.t_us)
    }
}

/// Executa o cenário e devolve a trilha (uma amostra a cada 100 ms).
pub fn run(s: &Scenario) -> Trace {
    let mut link = Link::new(s.link, s.seed);
    let mut bwe = Bwe::new(BweConfig {
        min_kbps: 150.min(s.start_kbps),
        max_kbps: s.max_kbps,
        start_kbps: s.start_kbps,
    });
    let fim = u64::from(s.duration_s) * 1_000_000;
    let mut agenda = s.capacity_schedule.clone();
    agenda.sort_by_key(|&(t, _)| t);
    let mut prox_agenda = 0usize;

    // Chegadas pendentes: (chegada, envio, bytes).
    let mut chegadas: BinaryHeap<Reverse<(Us, Us, u32)>> = BinaryHeap::new();
    // Histórico por pacote enviado: (instante de envio, descartado).
    let mut envios: Vec<(Us, bool)> = Vec::new();
    // Relatórios em trânsito: (entrega ao emissor, fração de perda).
    let mut relatorios: VecDeque<(Us, f32)> = VecDeque::new();
    // Estimativa por passo de 1 ms, para o emissor ver a de 50 ms atrás.
    let mut hist_est: Vec<u32> = Vec::new();
    let mut perdas: u32 = 0;
    let mut prox_quadro: Us = 0;
    let mut n_quadro: u64 = 0;
    let mut prox_relatorio = RELATORIO_US;
    let mut prox_update = UPDATE_US;
    let mut prox_amostra = AMOSTRA_US;
    let mut samples = Vec::with_capacity((fim / AMOSTRA_US) as usize);

    let mut now: Us = 0;
    while now < fim {
        while prox_agenda < agenda.len() && agenda[prox_agenda].0 <= now {
            link.set_capacity_kbps(now, agenda[prox_agenda].1);
            prox_agenda += 1;
        }

        // Estimativa que o emissor enxerga (50 ms atrás).
        let atras = (RETORNO_US / PASSO_US) as usize;
        let visto = if hist_est.len() > atras {
            hist_est[hist_est.len() - 1 - atras]
        } else {
            s.start_kbps
        };
        // Piso do emissor = mesmo piso do Bwe (150 kbps), não o start_kbps.
        let alvo_kbps = visto.clamp(150.min(s.start_kbps).min(s.max_kbps), s.max_kbps);

        // Fonte de vídeo: um quadro fragmentado em pacotes, enviados no mesmo instante.
        while now >= prox_quadro {
            let mut tam = u64::from(alvo_kbps) * 1000 / 8 / 60;
            if n_quadro > 0 && (n_quadro * QUADRO_US) % KEYFRAME_US < QUADRO_US {
                tam *= KEYFRAME_FATOR;
            }
            let mut resta = tam.max(1);
            while resta > 0 {
                let bytes = resta.min(u64::from(MTU)) as u32;
                resta -= u64::from(bytes);
                match link.send(now, bytes) {
                    Some(chegada) => {
                        chegadas.push(Reverse((chegada, now, bytes)));
                        envios.push((now, false));
                    }
                    None => {
                        perdas += 1;
                        envios.push((now, true));
                    }
                }
            }
            n_quadro += 1;
            prox_quadro = n_quadro * QUADRO_US;
        }

        // Entrega ao receptor (em ordem de chegada).
        while let Some(&Reverse((chegada, envio, bytes))) = chegadas.peek() {
            if chegada > now {
                break;
            }
            chegadas.pop();
            bwe.on_packet(envio, chegada, bytes);
        }

        // Relatório do receptor: perda dos últimos 2 s, chega ao emissor 50 ms depois.
        if now >= prox_relatorio {
            let ini = now.saturating_sub(JANELA_PERDA_US);
            let (total, perdidos) = envios
                .iter()
                .rev()
                .take_while(|(t, _)| *t >= ini)
                .fold((0u32, 0u32), |(n, p), (_, d)| (n + 1, p + u32::from(*d)));
            let frac = if total == 0 {
                0.0
            } else {
                perdidos as f32 / total as f32
            };
            relatorios.push_back((now + RETORNO_US, frac));
            prox_relatorio += RELATORIO_US;
        }
        while let Some(&(entrega, frac)) = relatorios.front() {
            if entrega > now {
                break;
            }
            relatorios.pop_front();
            bwe.on_loss_fraction(now, frac);
        }

        if now >= prox_update {
            bwe.update(now);
            prox_update += UPDATE_US;
        }
        hist_est.push(bwe.estimate_kbps());

        // Amostra ao final do intervalo (instante 100 ms, 200 ms, ...).
        if now + PASSO_US >= prox_amostra {
            samples.push(Sample {
                t_us: prox_amostra,
                estimate_kbps: bwe.estimate_kbps(),
                state: bwe.state(),
                queue_delay_us: link.queue_delay_us(now),
                loss_events: perdas,
            });
            prox_amostra += AMOSTRA_US;
        }
        now += PASSO_US;
    }
    Trace { samples }
}
