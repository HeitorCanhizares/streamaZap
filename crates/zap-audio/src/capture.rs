//! Captura de áudio por aplicativo: mistura os clientes de loopback em quadros de 20 ms.
//!
//! Um cliente que cai (erro do WASAPI, troca de dispositivo) volta sozinho depois de uma espera
//! crescente, e um app que foi fechado e aberto de novo é achado pelo executável.
use crate::loopback::LoopbackClient;
use crate::{app_tree, apps, clock, mix, AudioError, AudioSelection, CapturedAudio, FRAME_SAMPLES};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows::Win32::System::Threading::{AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW};

const BLOCO: Duration = Duration::from_millis(20);
/// Acima disso (3 blocos) o consumer está atrasado: descartamos o excesso antigo.
const LIMITE_ATRASO: usize = FRAME_SAMPLES * 3;
const MAX_ERROS: usize = 50;
/// Chave especial do cliente único de "tudo menos eu".
const PID_TUDO: u32 = 0;
/// Primeira espera para recriar um cliente que caiu; dobra a cada falha seguida, até o teto.
const ESPERA_INICIAL: Duration = Duration::from_secs(1);
const ESPERA_MAX: Duration = Duration::from_secs(5);
/// Cliente que durou isso já não conta como "falha seguida": a espera volta ao início.
const VIDA_ESTAVEL: Duration = Duration::from_secs(10);
/// A cada quantos blocos (2 s) confere se os apps escolhidos ainda estão abertos.
const CONFERIR_APPS: u32 = 100;

/// Espera crescente: 1 s, 2 s, 4 s, 5 s, 5 s...
pub(crate) fn espera_para(falhas_seguidas: u32) -> Duration {
    let fator = 1u32 << falhas_seguidas.min(8);
    (ESPERA_INICIAL * fator).min(ESPERA_MAX)
}

pub struct AppCapture {
    pub(crate) shared: Arc<CaptureShared>,
    pub(crate) thread: Option<std::thread::JoinHandle<()>>,
}

pub(crate) struct CaptureShared {
    pub selection: std::sync::Mutex<AudioSelection>,
    pub errors: std::sync::Mutex<Vec<String>>,
    pub stop: std::sync::atomic::AtomicBool,
}

impl CaptureShared {
    fn push_erro(&self, msg: String) {
        if let Ok(mut e) = self.errors.lock() {
            e.push(msg);
            let excesso = e.len().saturating_sub(MAX_ERROS);
            if excesso > 0 {
                e.drain(..excesso);
            }
        }
    }
}

/// Mistura os blocos (cada um com até 1920 amostras; falta vira silêncio), limita e devolve o pico.
pub fn mix_block(consumers_data: &[&[f32]], out: &mut [f32; FRAME_SAMPLES]) -> f32 {
    out.fill(0.0);
    for d in consumers_data {
        mix::add_into(out, d);
    }
    mix::limit(out);
    mix::peak(out)
}

struct Mmcss(Option<windows::Win32::Foundation::HANDLE>);

impl Drop for Mmcss {
    fn drop(&mut self) {
        if let Some(h) = self.0 {
            // SAFETY: handle devolvido por AvSetMmThreadCharacteristicsW nesta mesma thread.
            unsafe {
                let _ = AvRevertMmThreadCharacteristics(h);
            }
        }
    }
}

impl AppCapture {
    pub fn start(sel: AudioSelection, out: crossbeam_channel::Sender<CapturedAudio>) -> Result<Self, AudioError> {
        let shared = Arc::new(CaptureShared {
            selection: Mutex::new(sel),
            errors: Mutex::new(Vec::new()),
            stop: AtomicBool::new(false),
        });
        let t_shared = shared.clone();
        let thread = std::thread::Builder::new()
            .name("zap-audio-capture".into())
            .spawn(move || laco(&t_shared, &out))
            .map_err(|e| AudioError::Backend(format!("thread de captura: {e}")))?;
        Ok(AppCapture {
            shared,
            thread: Some(thread),
        })
    }

    pub fn set_selection(&self, sel: AudioSelection) {
        if let Ok(mut g) = self.shared.selection.lock() {
            *g = sel;
        }
    }

    pub fn errors(&self) -> Vec<String> {
        self.shared.errors.lock().map(|e| e.clone()).unwrap_or_default()
    }
}

impl Drop for AppCapture {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Chaves (pids; 0 = tudo) que a seleção pede, sem repetição.
fn chaves_desejadas(sel: &AudioSelection) -> Vec<u32> {
    match sel {
        AudioSelection::None => Vec::new(),
        AudioSelection::All => vec![PID_TUDO],
        AudioSelection::Apps(v) => {
            let mut r: Vec<u32> = Vec::with_capacity(v.len());
            for p in v {
                if !r.contains(p) {
                    r.push(*p);
                }
            }
            r
        }
    }
}

/// Cliente vivo de uma chave da seleção.
struct Cliente {
    lb: LoopbackClient,
    cons: rtrb::Consumer<f32>,
    desde: Instant,
}

/// Processo que uma chave (pid escolhido na lista) captura agora, e o executável dele para achar
/// o app de novo se ele for reaberto.
struct Alvo {
    pid: u32,
    exe: Option<String>,
}

/// Quando tentar recriar uma chave que caiu.
struct Espera {
    ate: Instant,
    falhas_seguidas: u32,
}

fn laco(shared: &CaptureShared, out: &crossbeam_channel::Sender<CapturedAudio>) {
    let mut indice = 0u32;
    // SAFETY: nome constante terminado em nulo; `indice` é um ponteiro de saída válido.
    let _mmcss = Mmcss(unsafe { AvSetMmThreadCharacteristicsW(windows::core::w!("Pro Audio"), &mut indice).ok() });

    let meu_pid = std::process::id();
    let mut clientes: HashMap<u32, Cliente> = HashMap::new();
    let mut alvos: HashMap<u32, Alvo> = HashMap::new();
    let mut esperas: HashMap<u32, Espera> = HashMap::new();
    let mut ultima_sel: Option<AudioSelection> = None;
    let mut buffers: HashMap<u32, Vec<f32>> = HashMap::new();
    let mut saida = [0.0_f32; FRAME_SAMPLES];
    let mut descartados: u64 = 0;

    let inicio = Instant::now();
    let mut n: u32 = 0;
    while !shared.stop.load(Ordering::SeqCst) {
        n = n.wrapping_add(1);
        let alvo = inicio + BLOCO * n;
        // Dorme até ~1 ms antes e faz spin curto, sem acumular deriva.
        loop {
            if shared.stop.load(Ordering::SeqCst) {
                return;
            }
            let agora = Instant::now();
            if agora >= alvo {
                break;
            }
            let falta = alvo - agora;
            if falta > Duration::from_millis(2) {
                std::thread::sleep((falta - Duration::from_millis(1)).min(Duration::from_millis(5)));
            } else {
                std::hint::spin_loop();
            }
        }
        let t_bloco = clock::now_us();

        let sel = shared
            .selection
            .lock()
            .map(|g| g.clone())
            .unwrap_or(AudioSelection::None);
        let desejadas = chaves_desejadas(&sel);
        if ultima_sel.as_ref() != Some(&sel) {
            // Seleção nova: tenta tudo de novo já, e anota o executável das chaves novas.
            esperas.clear();
            alvos.retain(|k, _| desejadas.contains(k));
            let novas: Vec<u32> = desejadas
                .iter()
                .copied()
                .filter(|k| *k != PID_TUDO && !alvos.contains_key(k))
                .collect();
            if !novas.is_empty() {
                let tabela = apps::process_table();
                for k in novas {
                    let exe = tabela.get(&k).map(|(_, e)| e.clone());
                    alvos.insert(k, Alvo { pid: k, exe });
                }
            }
            ultima_sel = Some(sel.clone());
        }

        // Derruba os que saíram da seleção.
        clientes.retain(|k, _| desejadas.contains(k));
        buffers.retain(|k, _| desejadas.contains(k));

        // App fechado e aberto de novo: acha pelo executável e troca o processo capturado.
        if n % CONFERIR_APPS == 0 {
            let fechados: Vec<u32> = alvos
                .iter()
                .filter(|(_, a)| a.exe.is_some() && !apps::is_alive(a.pid))
                .map(|(k, _)| *k)
                .collect();
            if !fechados.is_empty() {
                let tabela = apps::process_table();
                for k in fechados {
                    let Some(alvo) = alvos.get_mut(&k) else {
                        continue;
                    };
                    let novo = alvo
                        .exe
                        .as_deref()
                        .and_then(|exe| app_tree::root_by_exe(exe, &tabela, meu_pid));
                    if let Some(novo) = novo.filter(|p| *p != alvo.pid) {
                        tracing::info!(
                            antes = alvo.pid,
                            agora = novo,
                            "app reaberto: capturando o processo novo"
                        );
                        alvo.pid = novo;
                        clientes.remove(&k);
                        buffers.remove(&k);
                        esperas.remove(&k);
                    }
                }
            }
        }

        // Os que caíram voltam depois de uma espera (crescente se caírem de novo logo).
        let mortos: Vec<u32> = clientes
            .iter()
            .filter(|(_, c)| c.lb.is_dead())
            .map(|(k, _)| *k)
            .collect();
        for k in mortos {
            let Some(c) = clientes.remove(&k) else {
                continue;
            };
            buffers.remove(&k);
            let msg = c.lb.take_error().unwrap_or_else(|| "captura encerrada".into());
            let anteriores = if c.desde.elapsed() >= VIDA_ESTAVEL {
                0
            } else {
                esperas.get(&k).map_or(0, |e| e.falhas_seguidas)
            };
            let espera = espera_para(anteriores);
            tracing::warn!(chave = k, erro = %msg, espera_ms = espera.as_millis() as u64, "captura de som caiu; tentando de novo");
            shared.push_erro(format!("pid {}: {msg}", c.lb.pid()));
            esperas.insert(
                k,
                Espera {
                    ate: Instant::now() + espera,
                    falhas_seguidas: anteriores + 1,
                },
            );
        }

        // Cria os que faltam (os que estão esperando, só quando der a hora).
        for k in &desejadas {
            if clientes.contains_key(k) || esperas.get(k).is_some_and(|e| Instant::now() < e.ate) {
                continue;
            }
            let r = if *k == PID_TUDO {
                LoopbackClient::start(meu_pid, false)
            } else {
                LoopbackClient::start(alvos.get(k).map_or(*k, |a| a.pid), true)
            };
            match r {
                Ok((lb, cons)) => {
                    clientes.insert(
                        *k,
                        Cliente {
                            lb,
                            cons,
                            desde: Instant::now(),
                        },
                    );
                }
                Err(e) => {
                    let anteriores = esperas.get(k).map_or(0, |e| e.falhas_seguidas);
                    shared.push_erro(format!("pid {k}: {e}"));
                    esperas.insert(
                        *k,
                        Espera {
                            ate: Instant::now() + espera_para(anteriores),
                            falhas_seguidas: anteriores + 1,
                        },
                    );
                }
            }
        }

        if sel == AudioSelection::None {
            continue;
        }

        // Lê até um bloco de cada consumer.
        for (k, Cliente { cons, .. }) in clientes.iter_mut() {
            let mut disp = cons.slots();
            if disp > LIMITE_ATRASO {
                // Mantém só 1 bloco: descarta o mais antigo.
                let jogar = disp - FRAME_SAMPLES;
                if let Ok(chunk) = cons.read_chunk(jogar) {
                    chunk.commit_all();
                }
                disp = FRAME_SAMPLES;
            }
            let buf = buffers.entry(*k).or_default();
            buf.clear();
            for _ in 0..disp.min(FRAME_SAMPLES) {
                match cons.pop() {
                    Ok(v) => buf.push(v),
                    Err(_) => break,
                }
            }
        }
        let dados: Vec<&[f32]> = buffers.values().map(|b| b.as_slice()).collect();
        let level = mix_block(&dados, &mut saida);

        let bloco = CapturedAudio {
            pcm: Box::new(saida),
            capture_us: t_bloco.saturating_sub(20_000),
            level,
        };
        match out.try_send(bloco) {
            Ok(()) => {}
            Err(crossbeam_channel::TrySendError::Full(_)) => descartados += 1,
            Err(crossbeam_channel::TrySendError::Disconnected(_)) => break,
        }
    }
    if descartados > 0 {
        tracing::debug!("captura: {descartados} blocos descartados (canal cheio)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn mix_block_soma_e_limita() {
        let a = vec![0.4_f32; FRAME_SAMPLES];
        let b = vec![0.5_f32; 100];
        let c = vec![0.6_f32; FRAME_SAMPLES];
        let mut out = [9.0_f32; FRAME_SAMPLES];
        let pico = mix_block(&[&a, &b, &c], &mut out);
        assert_eq!(out[0], 1.0);
        assert_eq!(out[100], 1.0);
        assert!((out[FRAME_SAMPLES - 1] - 1.0).abs() < 1e-6);
        assert_eq!(pico, 1.0);
        let pico = mix_block(&[], &mut out);
        assert_eq!(pico, 0.0);
        assert!(out.iter().all(|x| *x == 0.0));
    }

    #[test]
    fn espera_cresce_ate_o_teto() {
        assert_eq!(espera_para(0), Duration::from_secs(1));
        assert_eq!(espera_para(1), Duration::from_secs(2));
        assert_eq!(espera_para(2), Duration::from_secs(4));
        assert_eq!(espera_para(3), Duration::from_secs(5));
        assert_eq!(espera_para(40), Duration::from_secs(5));
    }

    #[test]
    fn app_capture_none_nao_envia() {
        let (tx, rx) = crossbeam_channel::bounded(16);
        let cap = AppCapture::start(AudioSelection::None, tx).expect("start");
        std::thread::sleep(Duration::from_millis(200));
        assert!(rx.try_recv().is_err());
        drop(cap);
    }

    #[test]
    fn app_capture_all_entrega_blocos() {
        let (tx, rx) = crossbeam_channel::bounded(256);
        let cap = match AppCapture::start(AudioSelection::All, tx) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("pulando: start falhou: {e}");
                return;
            }
        };
        std::thread::sleep(Duration::from_secs(1));
        let n = rx.try_iter().count();
        let erros = cap.errors();
        if !erros.is_empty() {
            eprintln!("pulando: clientes falharam: {erros:?}");
            return;
        }
        assert!(n >= 40, "recebeu só {n} blocos");
    }

    #[test]
    fn set_selection_nao_panica() {
        let (tx, _rx) = crossbeam_channel::bounded(16);
        let cap = AppCapture::start(AudioSelection::None, tx).expect("start");
        cap.set_selection(AudioSelection::Apps(vec![u32::MAX]));
        std::thread::sleep(Duration::from_millis(60));
        cap.set_selection(AudioSelection::None);
        let _ = cap.errors();
    }

    #[test]
    fn drop_encerra_rapido() {
        let (tx, _rx) = crossbeam_channel::bounded(16);
        let cap = AppCapture::start(AudioSelection::None, tx).expect("start");
        let t = Instant::now();
        drop(cap);
        assert!(t.elapsed() < Duration::from_millis(500));
    }
}
