//! Testes de integração da carência de rajada própria do `Bwe` sobre `sim::Link` (tarefa rate-03).
//!
//! Tempo virtual com passo de 1 ms: vídeo (e sondas) -> `Link` -> `Bwe`, entregue em ordem de chegada.
use std::cmp::Reverse;
use std::collections::BinaryHeap;

use zap_proto::DelayState;
use zap_rate::bwe::{Bwe, BweConfig};
use zap_rate::sim::{Link, LinkConfig};
use zap_rate::Us;

const MTU: u32 = 1_200;
const QUADRO_BYTES: u32 = 10_417;
const KEYFRAME_BYTES: u32 = 83_333;
const QUADRO_US: Us = 33_333;
const UPDATE_US: Us = 50_000;
/// Piso exigido da estimativa (kbps), definido na spec.
const PISO_KBPS: u32 = 2_639;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Tipo {
    Comum,
    KeyInicio,
    KeyFim,
    Sonda,
}

struct Cenario {
    fila_bytes: usize,
    duracao_us: Us,
    /// (instante, kbps) de troca de capacidade.
    queda: Option<(Us, u32)>,
    /// Rajada de sondas: (início, duração, kbps). `marcar` diz se usa `on_probe_packet`.
    sonda: Option<(Us, Us, u32)>,
    marcar_sonda: bool,
    /// Espalha os pacotes de cada quadro pelo intervalo do quadro (taxa constante).
    paceado: bool,
    /// Sem keyframes o vídeo é de taxa constante (a fila de 50 kB não aguenta um keyframe).
    keyframes: bool,
}

#[derive(Default)]
struct Resultado {
    perdidos: u32,
    overuse: bool,
    /// (instante, estimativa) em cada update.
    estimativas: Vec<(Us, u32)>,
}

fn rodar(c: &Cenario) -> Resultado {
    let mut link = Link::new(
        LinkConfig {
            base_delay_ms: 20.0,
            jitter_ms: 0.0,
            loss: 0.0,
            queue_bytes: c.fila_bytes,
        },
        1,
    );
    link.set_capacity_kbps(0, 4_000);
    let mut bwe = Bwe::new(BweConfig {
        min_kbps: 150,
        max_kbps: 50_000,
        start_kbps: 1_000,
    });
    bwe.on_probe_cluster(0, 3_790);

    let mut r = Resultado::default();
    // (chegada, envio, bytes, tipo)
    let mut fila: BinaryHeap<Reverse<(Us, Us, u32, Tipo)>> = BinaryHeap::new();
    // Envios agendados: (instante de envio, bytes, tipo).
    let mut envios: BinaryHeap<Reverse<(Us, u32, Tipo)>> = BinaryHeap::new();
    let mut prox_quadro = 0u64;
    let mut prox_sonda = c.sonda.map_or(Us::MAX, |s| s.0);
    let mut ultima_chegada = 0;
    let mut fim_sonda_enviado = false;

    let mut t: Us = 0;
    while t <= c.duracao_us {
        if let Some((quando, kbps)) = c.queda {
            if t == quando {
                link.set_capacity_kbps(t, kbps);
            }
        }
        // Quadros de vídeo a 30 fps; keyframe em 0,5 s, 10,5 s, 20,5 s.
        while prox_quadro * QUADRO_US <= t {
            let envio = prox_quadro * QUADRO_US;
            let key = c.keyframes && envio >= 500_000 && (envio - 500_000) % 10_000_000 < QUADRO_US;
            let total = if key { KEYFRAME_BYTES } else { QUADRO_BYTES };
            let n = total.div_ceil(MTU);
            for i in 0..n {
                let bytes = if i + 1 == n { total - MTU * (n - 1) } else { MTU };
                let tipo = match (key, i) {
                    (false, _) => Tipo::Comum,
                    (true, 0) => Tipo::KeyInicio,
                    (true, i) if i + 1 == n => Tipo::KeyFim,
                    _ => Tipo::Comum,
                };
                let desloc = if c.paceado {
                    u64::from(i) * QUADRO_US / u64::from(n)
                } else {
                    0
                };
                envios.push(Reverse((envio + desloc, bytes, tipo)));
            }
            prox_quadro += 1;
        }
        // Sondas a taxa constante durante a rajada.
        if let Some((ini, dur, kbps)) = c.sonda {
            let intervalo = u64::from(MTU) * 8 * 1000 / u64::from(kbps);
            while prox_sonda <= t && prox_sonda < ini + dur {
                envios.push(Reverse((prox_sonda, MTU, Tipo::Sonda)));
                prox_sonda += intervalo;
            }
            if !fim_sonda_enviado && t >= ini + dur {
                fim_sonda_enviado = true;
                if c.marcar_sonda {
                    bwe.on_probe_cluster(t, 5_200);
                }
            }
        }
        while let Some(&Reverse((envio, bytes, tipo))) = envios.peek() {
            if envio > t {
                break;
            }
            envios.pop();
            match link.send(envio, bytes) {
                Some(arr) => fila.push(Reverse((arr, envio, bytes, tipo))),
                None => r.perdidos += 1,
            }
        }
        // Entrega em ordem de chegada.
        while let Some(&Reverse((arr, envio, bytes, tipo))) = fila.peek() {
            if arr > t {
                break;
            }
            fila.pop();
            ultima_chegada = ultima_chegada.max(arr);
            match tipo {
                Tipo::Comum => bwe.on_packet(envio, arr, bytes),
                Tipo::KeyInicio => bwe.on_keyframe_start(arr),
                Tipo::KeyFim => bwe.on_keyframe_packet(envio, arr, bytes),
                Tipo::Sonda if c.marcar_sonda => bwe.on_probe_packet(envio, arr, bytes),
                Tipo::Sonda => bwe.on_packet(envio, arr, bytes),
            }
        }
        if t % UPDATE_US == 0 {
            let now = t.max(ultima_chegada);
            let est = bwe.update(now);
            r.estimativas.push((t, est));
            r.overuse |= bwe.state() == DelayState::Overuse;
        }
        t += 1_000;
    }
    r
}

fn base(duracao_us: Us) -> Cenario {
    Cenario {
        fila_bytes: 150_000,
        duracao_us,
        queda: None,
        sonda: None,
        marcar_sonda: true,
        paceado: false,
        keyframes: true,
    }
}

#[test]
fn keyframe_no_enlace_justo_nao_derruba() {
    let r = rodar(&base(30_000_000));
    assert_eq!(r.perdidos, 0);
    assert!(!r.overuse, "Overuse anunciado");
    let minimo = r.estimativas.iter().map(|e| e.1).min().unwrap_or(0);
    assert!(minimo >= PISO_KBPS, "estimativa mínima = {minimo}");
}

#[test]
fn congestionamento_real_ainda_derruba() {
    let mut c = base(30_000_000);
    c.queda = Some((12_000_000, 2_000));
    let r = rodar(&c);
    let caiu = r
        .estimativas
        .iter()
        .any(|&(t, e)| (12_000_000..=14_500_000).contains(&t) && e < PISO_KBPS);
    assert!(caiu, "estimativa não caiu em 2,5 s: {:?}", &r.estimativas[240..290]);
}

#[test]
fn sonda_em_degrau_nao_derruba() {
    let mut c = base(5_000_000);
    c.fila_bytes = 50_000;
    c.paceado = true;
    c.keyframes = false;
    c.sonda = Some((1_000_000, 200_000, 3_125));
    let r = rodar(&c);
    assert_eq!(r.perdidos, 0, "pacotes perdidos no Link");
    assert!(!r.overuse, "Overuse anunciado");
    let minimo = r
        .estimativas
        .iter()
        .filter(|e| e.0 <= 1_200_000 + 3_000_000)
        .map(|e| e.1)
        .min()
        .unwrap_or(0);
    assert!(minimo >= PISO_KBPS, "estimativa mínima = {minimo}");
}

#[test]
fn rajada_acima_do_gargalo_sem_carencia_derruba() {
    let mut c = base(5_000_000);
    c.fila_bytes = 50_000;
    c.paceado = true;
    c.keyframes = false;
    c.sonda = Some((1_000_000, 200_000, 3_125));
    c.marcar_sonda = false;
    let r = rodar(&c);
    let minimo = r.estimativas.iter().map(|e| e.1).min().unwrap_or(0);
    assert!(r.overuse || minimo < 3_600, "sem carência nada reagiu (mín = {minimo})");
}
