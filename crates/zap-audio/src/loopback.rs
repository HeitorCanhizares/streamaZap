//! Um cliente de loopback por processo (WASAPI `VAD\Process_Loopback`).

use crate::com::ComGuard;
use crate::AudioError;
use std::mem::ManuallyDrop;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use windows::core::{implement, Interface, HRESULT};
use windows::Win32::Foundation::{CloseHandle, E_INVALIDARG, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::{
    ActivateAudioInterfaceAsync, IActivateAudioInterfaceAsyncOperation, IActivateAudioInterfaceCompletionHandler,
    IActivateAudioInterfaceCompletionHandler_Impl, IAudioCaptureClient, IAudioClient, AUDCLNT_BUFFERFLAGS_SILENT,
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM, AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
    AUDCLNT_STREAMFLAGS_LOOPBACK, AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, AUDIOCLIENT_ACTIVATION_PARAMS,
    AUDIOCLIENT_ACTIVATION_PARAMS_0, AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK, AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS,
    PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE, PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
    VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, WAVEFORMATEX,
};
use windows::Win32::System::Com::StructuredStorage::{PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0};
use windows::Win32::System::Com::{IAgileObject, IAgileObject_Impl, BLOB};
use windows::Win32::System::Threading::{
    AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW, CreateEventW, SetEvent, WaitForSingleObject,
};
use windows::Win32::System::Variant::VT_BLOB;

const MSG_VERSAO: &str = "loopback por processo requer Windows 10 build 20348+ ou Windows 11";

pub struct LoopbackClient {
    pub(crate) pid: u32,
    pub(crate) stop: Arc<AtomicBool>,
    pub(crate) thread: Option<std::thread::JoinHandle<()>>,
    dead: Arc<AtomicBool>,
    erro: Arc<Mutex<Option<String>>>,
}

/// Falha na fase de ativação, antes de a captura começar.
enum FalhaInicio {
    ProcessoInvalido,
    Backend(String),
}

impl LoopbackClient {
    /// `include_tree = true`: INCLUDE_TARGET_PROCESS_TREE de `pid`; `false`: EXCLUDE_TARGET_PROCESS_TREE
    /// (usado com o próprio PID para "tudo menos eu"). A thread de captura (MMCSS "Pro Audio")
    /// escreve f32 estéreo 48 kHz intercalado no anel devolvido (capacidade 1 s).
    pub fn start(pid: u32, include_tree: bool) -> Result<(Self, rtrb::Consumer<f32>), AudioError> {
        let (producer, consumer) = rtrb::RingBuffer::<f32>::new(48_000 * 2);
        let stop = Arc::new(AtomicBool::new(false));
        let dead = Arc::new(AtomicBool::new(false));
        let erro: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let (tx, rx) = std::sync::mpsc::sync_channel::<Result<(), FalhaInicio>>(1);

        let (t_stop, t_dead, t_erro) = (stop.clone(), dead.clone(), erro.clone());
        let thread = std::thread::Builder::new()
            .name("zap-loopback".into())
            .spawn(move || {
                let resultado = capturar(pid, include_tree, producer, &t_stop, &tx);
                if let Err(msg) = resultado {
                    if let Ok(mut g) = t_erro.lock() {
                        *g = Some(msg);
                    }
                }
                t_dead.store(true, Ordering::SeqCst);
            })
            .map_err(|e| AudioError::Backend(format!("thread de loopback: {e}")))?;

        let cliente = LoopbackClient {
            pid,
            stop,
            thread: Some(thread),
            dead,
            erro,
        };
        match rx.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(())) => Ok((cliente, consumer)),
            Ok(Err(FalhaInicio::ProcessoInvalido)) => Err(AudioError::ProcessNotFound(pid)),
            Ok(Err(FalhaInicio::Backend(m))) => Err(AudioError::Backend(m)),
            // Thread morreu sem avisar ou demorou demais: o Drop do cliente encerra e dá join.
            Err(_) => Err(AudioError::Backend(
                "tempo esgotado ativando o loopback por processo".into(),
            )),
        }
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// A thread terminou (app fechou ou erro). Mensagem do erro, se houver, em `take_error`.
    pub fn is_dead(&self) -> bool {
        self.dead.load(Ordering::SeqCst)
    }

    pub fn take_error(&self) -> Option<String> {
        self.erro.lock().ok().and_then(|mut g| g.take())
    }
}

impl Drop for LoopbackClient {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Converte bytes f32 little-endian em amostras. Devolve quantas amostras foram escritas
/// (limitado ao que cabe em `src` e em `dst`; bytes sobrando no fim são ignorados).
pub(crate) fn bytes_para_f32(src: &[u8], dst: &mut [f32]) -> usize {
    let mut n = 0;
    for (c, d) in src.chunks_exact(4).zip(dst.iter_mut()) {
        *d = f32::from_le_bytes([c[0], c[1], c[2], c[3]]);
        n += 1;
    }
    n
}

/// Evento Win32 fechado no Drop.
struct Evento(HANDLE);

impl Evento {
    fn novo() -> windows::core::Result<Self> {
        // SAFETY: cria evento anônimo de reset automático; o handle é fechado no Drop.
        unsafe { CreateEventW(None, false, false, None) }.map(Evento)
    }
}

impl Drop for Evento {
    fn drop(&mut self) {
        // SAFETY: handle válido criado em `novo` e fechado uma única vez.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Sinaliza um evento quando a ativação assíncrona termina.
#[implement(IActivateAudioInterfaceCompletionHandler, IAgileObject)]
struct Handler {
    // HANDLE guardado como inteiro para o objeto COM ser Send/Sync.
    evento: isize,
}

// Marcador: o objeto pode ser chamado de qualquer thread do pool do WASAPI.
impl IAgileObject_Impl for Handler_Impl {}

impl IActivateAudioInterfaceCompletionHandler_Impl for Handler_Impl {
    fn ActivateCompleted(
        &self,
        _operacao: windows::core::Ref<IActivateAudioInterfaceAsyncOperation>,
    ) -> windows::core::Result<()> {
        // SAFETY: o evento vive até a thread de captura terminar de esperar por ele.
        unsafe { SetEvent(HANDLE(self.evento as *mut _)) }
    }
}

/// Reverte o MMCSS ao sair da thread.
struct Mmcss(Option<HANDLE>);

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

fn hr_pid_invalido(hr: HRESULT) -> bool {
    // E_INVALIDARG, ERROR_INVALID_PARAMETER (87) e ERROR_NOT_FOUND (1168) como HRESULT.
    hr == E_INVALIDARG || hr == HRESULT(0x8007_0490_u32 as i32)
}

fn falha_ativacao(e: &windows::core::Error) -> FalhaInicio {
    if hr_pid_invalido(e.code()) {
        FalhaInicio::ProcessoInvalido
    } else {
        FalhaInicio::Backend(format!("{MSG_VERSAO} ({e})"))
    }
}

/// Ativa o `IAudioClient` de loopback do processo e o devolve inicializado.
fn ativar(pid: u32, include_tree: bool) -> Result<(IAudioClient, Evento), FalhaInicio> {
    let backend = |e: windows::core::Error| FalhaInicio::Backend(e.to_string());

    let mut params = AUDIOCLIENT_ACTIVATION_PARAMS {
        ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
        Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
            ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
                TargetProcessId: pid,
                ProcessLoopbackMode: if include_tree {
                    PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE
                } else {
                    PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE
                },
            },
        },
    };
    // ManuallyDrop: o blob aponta para a pilha, PropVariantClear não pode tentar liberá-lo.
    let pv = ManuallyDrop::new(PROPVARIANT {
        Anonymous: PROPVARIANT_0 {
            Anonymous: ManuallyDrop::new(PROPVARIANT_0_0 {
                vt: VT_BLOB,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: PROPVARIANT_0_0_0 {
                    blob: BLOB {
                        cbSize: std::mem::size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32,
                        pBlobData: &mut params as *mut _ as *mut u8,
                    },
                },
            }),
        },
    });

    let ev_ativacao = Evento::novo().map_err(backend)?;
    let handler: IActivateAudioInterfaceCompletionHandler = Handler {
        evento: ev_ativacao.0 .0 as isize,
    }
    .into();

    // SAFETY: `pv` e `params` vivem até o fim da função; o handler é um objeto COM válido.
    let operacao = unsafe {
        ActivateAudioInterfaceAsync(
            VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
            &IAudioClient::IID,
            Some(&*pv as *const PROPVARIANT),
            &handler,
        )
    }
    .map_err(|e| falha_ativacao(&e))?;

    // SAFETY: handle de evento válido.
    if unsafe { WaitForSingleObject(ev_ativacao.0, 3000) } != WAIT_OBJECT_0 {
        // O handler ainda pode disparar depois: vaza o evento em vez de fechá-lo.
        std::mem::forget(ev_ativacao);
        return Err(FalhaInicio::Backend(
            "tempo esgotado ativando o loopback por processo".into(),
        ));
    }

    let mut hr_ativacao = HRESULT(0);
    let mut iface: Option<windows::core::IUnknown> = None;
    // SAFETY: ponteiros de saída válidos; a operação já foi concluída.
    unsafe { operacao.GetActivateResult(&mut hr_ativacao, &mut iface) }.map_err(|e| falha_ativacao(&e))?;
    if hr_ativacao.is_err() {
        return Err(falha_ativacao(&windows::core::Error::from(hr_ativacao)));
    }
    let cliente: IAudioClient = iface
        .ok_or_else(|| FalhaInicio::Backend(MSG_VERSAO.into()))?
        .cast()
        .map_err(backend)?;

    // f32 estéreo 48 kHz; AUTOCONVERTPCM faz o WASAPI converter o que for preciso.
    let fmt = WAVEFORMATEX {
        wFormatTag: 3, // WAVE_FORMAT_IEEE_FLOAT
        nChannels: 2,
        nSamplesPerSec: 48_000,
        nAvgBytesPerSec: 384_000,
        nBlockAlign: 8,
        wBitsPerSample: 32,
        cbSize: 0,
    };
    // SAFETY: `fmt` é válido durante a chamada; cliente recém-ativado.
    unsafe {
        cliente
            .Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK
                    | AUDCLNT_STREAMFLAGS_EVENTCALLBACK
                    | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
                    | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
                2_000_000, // 200 ms em unidades de 100 ns
                0,
                &fmt,
                None,
            )
            .map_err(backend)?;
    }
    let ev_captura = Evento::novo().map_err(backend)?;
    // SAFETY: evento válido que vive mais que o cliente (retornado junto).
    unsafe { cliente.SetEventHandle(ev_captura.0) }.map_err(backend)?;
    Ok((cliente, ev_captura))
}

/// Corpo da thread: ativa, avisa `tx` e captura até `stop` ou erro. `Err` = mensagem de erro.
fn capturar(
    pid: u32,
    include_tree: bool,
    mut producer: rtrb::Producer<f32>,
    stop: &AtomicBool,
    tx: &std::sync::mpsc::SyncSender<Result<(), FalhaInicio>>,
) -> Result<(), String> {
    let _com = ComGuard::new();
    let mut indice = 0u32;
    // SAFETY: nome constante terminado em nulo; `indice` é um ponteiro de saída válido.
    let mmcss = Mmcss(unsafe { AvSetMmThreadCharacteristicsW(windows::core::w!("Pro Audio"), &mut indice).ok() });

    let (cliente, ev_captura) = match ativar(pid, include_tree) {
        Ok(v) => v,
        Err(f) => {
            let msg = match &f {
                FalhaInicio::ProcessoInvalido => format!("processo {pid} inválido"),
                FalhaInicio::Backend(m) => m.clone(),
            };
            let _ = tx.send(Err(f));
            drop(mmcss);
            return Err(msg);
        }
    };
    let captura: IAudioCaptureClient = match unsafe { cliente.GetService() } {
        // SAFETY (acima): cliente inicializado; GetService só lê o objeto.
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(Err(FalhaInicio::Backend(e.to_string())));
            return Err(e.to_string());
        }
    };
    // SAFETY: cliente inicializado com evento configurado.
    if let Err(e) = unsafe { cliente.Start() } {
        let _ = tx.send(Err(FalhaInicio::Backend(e.to_string())));
        return Err(e.to_string());
    }
    let _ = tx.send(Ok(()));

    let mut amostras: Vec<f32> = Vec::with_capacity(4096);
    let resultado = loop {
        if stop.load(Ordering::SeqCst) {
            break Ok(());
        }
        // SAFETY: handle de evento válido.
        unsafe { WaitForSingleObject(ev_captura.0, 100) };
        // Drena todos os pacotes pendentes.
        loop {
            // SAFETY: cliente de captura válido nesta thread.
            let tamanho = match unsafe { captura.GetNextPacketSize() } {
                Ok(n) => n,
                Err(e) => return finalizar(&cliente, Err(e.to_string())),
            };
            if tamanho == 0 {
                break;
            }
            let mut dados: *mut u8 = std::ptr::null_mut();
            let mut quadros = 0u32;
            let mut flags = 0u32;
            // SAFETY: ponteiros de saída válidos; ReleaseBuffer logo abaixo com o mesmo nº de quadros.
            if let Err(e) = unsafe { captura.GetBuffer(&mut dados, &mut quadros, &mut flags, None, None) } {
                return finalizar(&cliente, Err(e.to_string()));
            }
            let n = quadros as usize * 2;
            amostras.clear();
            if flags & (AUDCLNT_BUFFERFLAGS_SILENT.0 as u32) != 0 || dados.is_null() {
                amostras.resize(n, 0.0);
            } else {
                amostras.resize(n, 0.0);
                // SAFETY: o WASAPI garante `quadros * 8` bytes legíveis até o ReleaseBuffer.
                let bytes = unsafe { std::slice::from_raw_parts(dados, n * 4) };
                bytes_para_f32(bytes, &mut amostras);
            }
            // SAFETY: devolve exatamente o que GetBuffer entregou.
            let _ = unsafe { captura.ReleaseBuffer(quadros) };
            // Anel cheio: descarta o excedente.
            let cabe = amostras.len().min(producer.slots());
            if cabe > 0 {
                if let Ok(chunk) = producer.write_chunk_uninit(cabe) {
                    chunk.fill_from_iter(amostras[..cabe].iter().copied());
                }
            }
        }
    };
    finalizar(&cliente, resultado)
}

fn finalizar(cliente: &IAudioClient, r: Result<(), String>) -> Result<(), String> {
    // SAFETY: cliente válido; Stop em cliente já parado é inofensivo.
    unsafe {
        let _ = cliente.Stop();
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_para_f32_converte_little_endian() {
        let mut src = Vec::new();
        for v in [1.0f32, -0.5, 0.25] {
            src.extend_from_slice(&v.to_le_bytes());
        }
        src.push(0xFF); // byte sobrando é ignorado
        let mut dst = [0.0f32; 4];
        let n = bytes_para_f32(&src, &mut dst);
        assert_eq!(n, 3);
        assert_eq!(&dst[..3], &[1.0, -0.5, 0.25]);
        assert_eq!(dst[3], 0.0);
    }

    #[test]
    fn bytes_para_f32_limita_pelo_destino() {
        let src = [0u8; 16];
        let mut dst = [1.0f32; 2];
        assert_eq!(bytes_para_f32(&src, &mut dst), 2);
        assert_eq!(dst, [0.0, 0.0]);
    }

    #[test]
    fn loopback_pid_inexistente_nao_quebra() {
        // O Windows aceita loopback de um PID que não existe (só captura silêncio), então não dá
        // para exigir erro aqui (visto no CI Windows). O que importa: não entra em pânico e, se
        // abrir, fecha limpo ao soltar.
        if let Ok((cliente, _rx)) = LoopbackClient::start(0xFFFF_FFF0, true) {
            drop(cliente);
        }
    }

    #[test]
    fn loopback_proprio_processo_exclude() {
        match LoopbackClient::start(std::process::id(), false) {
            Ok((c, _rx)) => {
                std::thread::sleep(Duration::from_millis(300));
                assert!(!c.is_dead());
                drop(c);
            }
            Err(e) => eprintln!("loopback indisponível, pulando: {e}"),
        }
    }
}
