# M2: mídia no Windows

Plano do arquiteto. Base: `docs/arquitetura-v3.md` (seções 2, 5 e 6) e `docs/api/balanceamento.md`
(escada de camadas). Objetivo: do pixel na tela até o H.264 de cada camada **sem passar pela RAM**,
e do som de cada app até o Opus, com tempos medidos por etapa.

## Decisão nova: `zap-media` vira 5 crates

Para as frentes andarem em paralelo (a agenda nunca põe duas tarefas no mesmo crate) e para
isolar o `unsafe`:

| Crate | Conteúdo | Plataforma |
|---|---|---|
| `zap-gpu` | dispositivo D3D11, anel de texturas, conversor BGRA/NV12 → NV12 (Video Processor), leitura para testes | Windows (stub em outros) |
| `zap-capture` | listas de monitores e janelas, captura de monitor (DXGI Desktop Duplication), de janela (WGC), câmera (MF Source Reader), cursor | Windows (stub) |
| `zap-encode` | codificador H.264 por Media Foundation (MFT de hardware assíncrono + MFT de software) | Windows (stub) |
| `zap-audio` | Opus (todas), loopback por processo WASAPI + mixer (Windows), saída de áudio com mixer lock-free e reamostragem (Windows) | parte portável |
| `zap-media` | tipos comuns, traits, **pipeline de vídeo** (captura → camadas → codificadores, threads), pipeline de áudio, **backend sintético** para testes | portável |

Dependências: `zap-capture → zap-gpu`, `zap-encode → zap-gpu`, `zap-media → todos`.
`zap-core` só usa `zap-media`.

### Monitor por DXGI, janela por WGC (por quê)

No Windows 10 a captura WGC desenha uma **borda amarela** em volta do que é capturado (só dá para
tirar no Windows 11). Para monitor inteiro, o Desktop Duplication (DXGI) não tem borda e é o mais
eficiente; o cursor é desenhado por GDI direto na textura (`GDI_COMPATIBLE`). Para janela, WGC é a
única opção na GPU (captura atrás de outras janelas). No Windows 11 desligamos a borda
(`IsBorderRequired = false`).

## Dependência `windows` (0.62)

Features necessárias (somar no `Cargo.toml` de cada crate só as que ele usa):
`Win32_Foundation`, `Win32_Graphics_Direct3D`, `Win32_Graphics_Direct3D11`, `Win32_Graphics_Dxgi`,
`Win32_Graphics_Dxgi_Common`, `Win32_Graphics_Gdi`, `Win32_UI_WindowsAndMessaging`,
`Win32_Graphics_Dwm`, `Win32_System_Threading`, `Win32_System_Com`,
`Win32_System_WinRT`, `Win32_System_WinRT_Direct3D11`, `Win32_System_WinRT_Graphics_Capture`,
`Graphics_Capture`, `Graphics_DirectX`, `Graphics_DirectX_Direct3D11`, `Foundation`,
`Win32_Media_MediaFoundation`, `Win32_Media_Audio`, `Win32_Media_KernelStreaming`,
`Win32_Media_Multimedia`, `Win32_System_Com_StructuredStorage`, `Win32_System_Variant`,
`Win32_UI_Shell_PropertiesSystem`, `Win32_System_ProcessStatus`, `Win32_Security`.

Outras: `opus = "0.3"` (compila a libopus com cmake; testado em Linux e `windows-gnu`),
`rtrb`, `rubato`, `cpal` (só Windows), `crossbeam-channel`.

## Regras de `unsafe` e threads

- Ponteiros COM do `windows` não são `Send`. Onde precisarmos mandar textura/dispositivo para
  outra thread, usar um wrapper `Send` com `// SAFETY:` explicando que o dispositivo foi criado
  com `ID3D10Multithread::SetMultithreadProtected(TRUE)` (D3D11 serializa as chamadas).
- O **contexto imediato** só é usado pela thread de vídeo da trilha e pela thread de captura de
  câmera, sempre sob a proteção multithread do dispositivo.
- COM inicializado por thread: `CoInitializeEx(COINIT_MULTITHREADED)` em toda thread que usa COM
  (guardião RAII `ComGuard` em `zap-gpu`). `MFStartup` uma vez por processo (guardião com
  contagem em `zap-encode`).
- Threads de áudio com MMCSS (`AvSetMmThreadCharacteristicsW("Pro Audio")`); thread de captura e
  de vídeo com `THREAD_PRIORITY_ABOVE_NORMAL`.

---

## Frente `gpu` (`crates/zap-gpu`)

```rust
pub struct GpuDevice { /* ID3D11Device, ID3D11DeviceContext, ID3D11VideoDevice, ID3D11VideoContext, IMFDXGIDeviceManager? (não: fica no encode) */ }
pub struct AdapterInfo { pub name: String, pub vendor: Vendor, pub luid: i64, pub dedicated_vram_mb: u32 }
pub enum Vendor { Nvidia /* 0x10DE */, Amd /* 0x1002 */, Intel /* 0x8086 */, Microsoft /* WARP/Basic 0x1414 */, Other(u32) }
impl GpuDevice {
    pub fn create_default() -> Result<Self, GpuError>;            // adaptador padrão, HARDWARE, BGRA_SUPPORT|VIDEO_SUPPORT
    pub fn create_for_adapter(luid: i64) -> Result<Self, GpuError>; // monitor em outra GPU
    pub fn adapter(&self) -> &AdapterInfo;
    // acesso cru para os outros crates (feature "raw"):
    pub fn d3d_device(&self) -> &ID3D11Device; pub fn context(&self) -> &ID3D11DeviceContext;
}
pub struct Texture { /* ID3D11Texture2D + formato + tamanho */ } // Send
pub enum PixelFormat { Bgra8, Nv12 }
pub struct TextureRing { /* N texturas iguais, devolve a próxima livre em rodízio */ }
impl TextureRing { pub fn new(dev: &GpuDevice, w: u32, h: u32, fmt: PixelFormat, n: usize, gdi_compatible: bool) -> Result<Self, GpuError>; pub fn next(&mut self) -> &Texture; }
pub struct Converter { /* ID3D11VideoProcessor por (entrada, saída) */ }
impl Converter {
    pub fn new(dev: &GpuDevice, in_w: u32, in_h: u32, in_fmt: PixelFormat, out_w: u32, out_h: u32) -> Result<Self, GpuError>;
    /// Converte/escala `src` para NV12 em `dst` (mesma GPU). Mantém proporção (as dimensões já
    /// vêm certas da escada). Espaço de cor: RGB 0-255 → YCbCr BT.709 faixa limitada.
    pub fn convert(&mut self, dev: &GpuDevice, src: &Texture, dst: &Texture) -> Result<(), GpuError>;
}
pub fn read_back(dev: &GpuDevice, tex: &Texture) -> Result<Vec<u8>, GpuError>; // testes/miniaturas: copia para staging e lê (NV12: Y + UV intercalado)
pub fn upload(dev: &GpuDevice, tex: &Texture, data: &[u8], stride: u32) -> Result<(), GpuError>; // testes e câmera sem D3D
```
Teste (Windows): textura BGRA preenchida com (R,G,B) = (255,0,0), (0,255,0), (0,0,255), branco e
preto → NV12 → Y/U/V iguais à fórmula BT.709 limitada ±3. Em Linux o crate compila com `GpuDevice`
que retorna `GpuError::Unsupported`.

## Frente `capture` (`crates/zap-capture`)

```rust
pub struct MonitorInfo { pub id: isize /* HMONITOR */, pub index: u32 /* 1.. */, pub name: String, pub x: i32, pub y: i32, pub width: u32, pub height: u32, pub primary: bool, pub adapter_luid: i64 }
pub struct WindowInfo { pub hwnd: isize, pub title: String, pub pid: u32, pub process_name: String, pub width: u32, pub height: u32 }
pub fn list_monitors() -> Result<Vec<MonitorInfo>, CaptureError>;
pub fn list_windows() -> Result<Vec<WindowInfo>, CaptureError>; // visíveis, não "cloaked", com título, sem as do próprio processo, sem tool windows
pub struct CapturedFrame { pub texture: zap_gpu::Texture /* BGRA ou NV12 */, pub width: u32, pub height: u32, pub capture_us: u64 /* relógio monotônico do processo (QueryPerformanceCounter → µs) */, pub new_content: bool }
pub trait FrameSink: Send + 'static { fn on_frame(&self, frame: CapturedFrame); fn on_error(&self, err: CaptureError); }
pub struct CaptureHandle { /* para a thread ao dropar */ }
pub fn capture_monitor(dev: &zap_gpu::GpuDevice, monitor: &MonitorInfo, sink: impl FrameSink) -> Result<CaptureHandle, CaptureError>;  // DXGI
pub fn capture_window(dev: &zap_gpu::GpuDevice, hwnd: isize, sink: impl FrameSink) -> Result<CaptureHandle, CaptureError>;             // WGC
pub struct CameraInfo { pub id: String /* symbolic link */, pub name: String }
pub fn list_cameras() -> Result<Vec<CameraInfo>, CaptureError>;
pub fn capture_camera(dev: &zap_gpu::GpuDevice, id: &str, want_w: u32, want_h: u32, want_fps: u32, sink: impl FrameSink) -> Result<CaptureHandle, CaptureError>;
```
Comportamento:
- **DXGI**: thread própria; `IDXGIOutput5::DuplicateOutput1` (BGRA) com fallback para
  `IDXGIOutput1::DuplicateOutput`; `AcquireNextFrame(timeout 100 ms)`; quando há imagem nova
  (`LastPresentTime != 0`) copia para o anel (`gdi_compatible = true`, 3 texturas) e libera o quadro
  na hora. Cursor: posição/visibilidade por `GetCursorInfo`, desenhado com `DrawIconEx` no DC da
  textura (`IDXGISurface1::GetDC`). Só o mouse mexeu (`LastPresentTime == 0` mas
  `LastMouseUpdateTime != 0`) → reaproveita a última imagem e redesenha o cursor.
  `DXGI_ERROR_ACCESS_LOST` (troca de resolução, UAC, jogo exclusivo) → recria a duplicação em
  até 1 s; `WAIT_TIMEOUT` → nada. Sem quadro novo por 1 s → reentrega o último com
  `new_content = false` (mantém o espectador vivo).
- **WGC**: `IGraphicsCaptureItemInterop::CreateForWindow`; `Direct3D11CaptureFramePool::CreateFreeThreaded`
  (2 buffers, BGRA); `IsCursorCaptureEnabled = true`; `IsBorderRequired = false` (ignorar erro no
  Windows 10). Em `FrameArrived`: copia a superfície para o anel e devolve o quadro ao pool;
  `ContentSize` diferente → `Recreate` do pool e do anel. Janela fechada (`Closed`) →
  `on_error(WindowClosed)`. Janela minimizada → nenhum quadro (o pipeline reentrega o último).
- **Câmera**: `MFEnumDeviceSources`; `MFCreateSourceReaderFromMediaSource` com
  `MF_SOURCE_READER_D3D_MANAGER`, `MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS` e
  `MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING`; tipo de saída NV12 no tamanho/fps nativo mais
  próximo do pedido (preferir ≥ pedido e ≤ 30 fps). `ReadSample` síncrono numa thread. Buffer DXGI
  → textura; buffer de memória → `upload` num anel NV12.
- `capture_us`: `QueryPerformanceCounter` convertido para µs (mesmo relógio para tudo no processo;
  função `zap_gpu::now_us()`).

Testes (Windows): listas não vazias (monitores) e sem erro (janelas, câmeras); DXGI entrega quadro
em ≤ 1 s com o mouse se movendo (`SetCursorPos` no teste) e imagem não toda preta; WGC captura uma
janela criada pelo teste (pular se `GraphicsCaptureSession::IsSupported()` for falso); câmera:
pular se a lista estiver vazia.

## Frente `encode` (`crates/zap-encode`)

```rust
pub struct EncoderInfo { pub name: String, pub vendor: zap_gpu::Vendor, pub hardware: bool }
pub fn list_encoders() -> Result<Vec<EncoderInfo>, EncodeError>;
pub enum EncoderChoice { Auto, Hardware, Software }
pub struct EncoderConfig { pub width: u32, pub height: u32, pub fps: u32, pub kbps: u32, pub gop_frames: u32 /* fps × 10 */, pub choice: EncoderChoice }
pub struct EncodedPicture { pub data: bytes::Bytes /* Annex B */, pub keyframe: bool, pub has_params: bool, pub capture_us: u64, pub encode_us: u32 /* tempo dentro do codificador */ }
pub struct H264Encoder { /* thread do MFT + canais */ }
impl H264Encoder {
    pub fn new(dev: &zap_gpu::GpuDevice, cfg: EncoderConfig, out: crossbeam_channel::Sender<EncodedPicture>) -> Result<Self, EncodeError>;
    /// Não bloqueia. Fila de 2: cheia = descarta este quadro e conta (`dropped()`).
    pub fn submit(&self, nv12: zap_gpu::Texture, capture_us: u64, force_keyframe: bool) -> bool;
    /// Muda o bitrate sem reabrir (CODECAPI_AVEncCommonMeanBitRate). Se o MFT recusar, reabre e o
    /// próximo quadro sai keyframe.
    pub fn set_bitrate(&self, kbps: u32);
    pub fn info(&self) -> &EncoderInfo;
    pub fn dropped(&self) -> u64;
}
```
Comportamento:
- `MFTEnumEx(MFT_CATEGORY_VIDEO_ENCODER, HARDWARE|SORTANDFILTER, entrada NV12, saída H264)`;
  ordem `Auto`: hardware do mesmo fabricante da GPU do `GpuDevice` → qualquer hardware → software
  (`MFT_ENUM_FLAG_SYNCMFT`).
- MFT de hardware (assíncrono): `MF_TRANSFORM_ASYNC_UNLOCK = 1`; `MFT_MESSAGE_SET_D3D_MANAGER`
  com um `IMFDXGIDeviceManager` do `GpuDevice`; tipo de **saída primeiro** (H264, tamanho, fps,
  `MF_MT_AVG_BITRATE`, progressivo, perfil High), depois **entrada** NV12.
  `ICodecAPI`: `AVEncCommonRateControlMode = CBR` (cair para `LowDelayVBR`/padrão se recusar),
  `AVEncCommonMeanBitRate`, `AVEncMPVGOPSize`, `AVLowLatencyMode = true`,
  `AVEncMPVDefaultBPictureCount = 0`, `AVEncCommonBufferSize = kbps × 1000 × 0,25`. Opções
  recusadas são logadas uma vez e ignoradas (menos o bitrate).
- Loop de eventos numa thread por codificador (`IMFMediaEventGenerator::GetEvent`):
  `METransformNeedInput` → pega o próximo quadro da fila (espera até 1 intervalo de quadro);
  `METransformHaveOutput` → `ProcessOutput` → `EncodedPicture`. Keyframe =
  `MFSampleExtension_CleanPoint`. Se o keyframe não tiver SPS/PPS (NAL 7/8), prefixar com
  `MF_MT_MPEG_SEQUENCE_HEADER` do tipo de saída.
- Entrada: `MFCreateSample` + `MFCreateDXGISurfaceBuffer(ID3D11Texture2D, textura, 0, FALSE)`,
  tempo em unidades de 100 ns a partir de `capture_us`. Keyframe forçado:
  `ICodecAPI::SetValue(AVEncVideoForceKeyFrame, 1)` antes do `ProcessInput`.
- MFT de software (síncrono, `CLSID_MSH264EncoderMFT`): entrada em memória (`read_back` do NV12),
  `ProcessInput`/`ProcessOutput` diretos. Usado em PC sem GPU (e no CI).

Testes (Windows; pular com mensagem se `list_encoders()` vier vazio): 60 quadros 1280×720 de cores
que mudam → primeiro quadro é keyframe com SPS/PPS; `set_bitrate` no meio não gera keyframe (no
hardware); `force_keyframe` no quadro 30 gera keyframe; tamanho médio dos quadros não-chave fica a
±50% de `kbps/fps`. Teste portável: parser de NAL (`nal.rs`: achar start codes, tipos 1/5/7/8).

## Frente `audio` (`crates/zap-audio`)

```rust
pub const RATE: u32 = 48_000; pub const CHANNELS: usize = 2; pub const FRAME: usize = 960; // 20 ms
pub struct OpusEnc; impl OpusEnc { pub fn new(kbps: u32) -> Result<Self, AudioError>; pub fn encode(&mut self, pcm: &[f32; FRAME * CHANNELS]) -> Result<bytes::Bytes, AudioError>; }
pub struct OpusDec; impl OpusDec { pub fn new() -> Result<Self, AudioError>; pub fn decode(&mut self, packet: Option<&[u8]>, fec_next: Option<&[u8]>, out: &mut [f32; FRAME * CHANNELS]) -> Result<(), AudioError>; }
// Windows:
pub struct AudioApp { pub pid: u32, pub name: String /* "cs2.exe" */ }
pub fn list_audio_apps() -> Result<Vec<AudioApp>, AudioError>;      // sessões de áudio ativas no dispositivo padrão
pub enum AudioSelection { None, All /* tudo menos o próprio processo */, Apps(Vec<u32>) }
pub struct AppCapture; // thread MMCSS; entrega quadros de 20 ms já mixados
impl AppCapture { pub fn start(sel: AudioSelection, out: crossbeam_channel::Sender<CapturedAudio>) -> Result<Self, AudioError>; pub fn set_selection(&self, sel: AudioSelection); pub fn errors(&self) -> Vec<String>; }
pub struct CapturedAudio { pub pcm: Box<[f32; FRAME * CHANNELS]>, pub capture_us: u64, pub level: f32 }
pub struct AudioOut; // saída padrão via cpal, mixer lock-free
impl AudioOut { pub fn start() -> Result<Self, AudioError>; pub fn add_track(&self, id: u64) -> TrackInput; pub fn remove_track(&self, id: u64); pub fn set_master_volume(&self, v: f32); }
pub struct TrackInput { /* produtor rtrb */ }
impl TrackInput { pub fn push(&mut self, pcm: &[f32]) -> usize; pub fn set_volume(&self, v: f32); pub fn set_rate(&self, r: f64 /* 0,98..1,02 do PlayoutClock */); pub fn buffered_ms(&self) -> f32; }
```
Comportamento:
- Opus: 48 kHz estéreo, `Application::Audio`, 128 kbps, FEC em banda ligado, perda esperada 5%,
  complexidade 8. Decodificação com `packet = None` usa FEC do próximo se houver, senão PLC.
- Loopback por processo: `ActivateAudioInterfaceAsync("VAD\\Process_Loopback", IAudioClient,
  AUDIOCLIENT_ACTIVATION_PARAMS)` com `INCLUDE_TARGET_PROCESS_TREE` (um cliente por app) ou
  `EXCLUDE_TARGET_PROCESS_TREE` com o próprio PID (`All`). Handler de conclusão com
  `#[implement(IActivateAudioInterfaceCompletionHandler, IAgileObject)]`. `Initialize` em modo
  compartilhado com `LOOPBACK | EVENTCALLBACK | AUTOCONVERTPCM | SRC_DEFAULT_QUALITY`, formato
  float32 48 kHz estéreo. Cada cliente escreve num anel; o mixer soma blocos de 960 amostras
  (app sem dados = silêncio), limita em ±1,0 e entrega `CapturedAudio` com `capture_us` do início
  do bloco. App que fechou → removido e registrado em `errors()`.
- `AudioOut`: dispositivo padrão, f32. Se a taxa do dispositivo ≠ 48 kHz, reamostra com `rubato`.
  Callback do cpal: para cada trilha, lê do `rtrb` (sem lock), aplica a taxa `set_rate` com
  reamostrador de razão variável (`rubato`, ±2%), volume (atômico), soma, limita. Falta de dados =
  silêncio com fade de 5 ms (sem estalo).

Testes: Opus ida e volta de um seno de 1 kHz (SNR > 20 dB) e PLC sem pânico (portáveis);
Windows: `list_audio_apps` sem erro; `AppCapture::start(All)` entrega ≥ 40 blocos em 1 s;
`AudioOut` abre o dispositivo (pular se não houver saída, comum no CI).

## Frente `media` (`crates/zap-media`)

```rust
pub struct EncodedFrame { pub track: u16, pub layer: u8, pub seq: u32, pub keyframe: bool, pub has_params: bool, pub width: u16, pub height: u16, pub capture_us: u64, pub data: bytes::Bytes }
pub struct EncodedAudio { pub track: u16, pub seq: u32, pub capture_us: u64, pub data: bytes::Bytes }
pub enum VideoSourceSpec { Monitor(MonitorRef), Window(isize), Camera(String), Synthetic { width: u32, height: u32 } }
pub struct VideoPipelineConfig { pub track: u16, pub source: VideoSourceSpec, pub ladder: Vec<zap_proto::LayerSpec>, pub encoder: EncoderChoiceSpec }
pub struct VideoPipeline { /* thread de vídeo + capturador + codificadores por camada */ }
impl VideoPipeline {
    pub fn start(cfg: VideoPipelineConfig, out: crossbeam_channel::Sender<EncodedFrame>) -> Result<Self, MediaError>;
    pub fn set_requests(&self, reqs: &[zap_rate::LayerRequest]); // liga/desliga camadas e ajusta bitrate
    pub fn request_keyframe(&self, layer: u8);
    pub fn set_ladder(&self, ladder: Vec<zap_proto::LayerSpec>);  // troca de qualidade ao vivo
    pub fn stats(&self) -> PipelineStats; // fps por camada, tempos (captura, conversão, codificação) média e p95, quadros descartados, codificador em uso
}
pub struct AudioPipeline; // AppCapture → Opus → EncodedAudio
```
Thread de vídeo (Windows): recebe `CapturedFrame` num slot "o mais novo vence" (sem fila); a
cada quadro, para cada camada **ligada** cujo intervalo (1/fps da camada) já passou: `Converter`
para o NV12 da camada (anel de 4) → `H264Encoder::submit`. Saídas dos codificadores viram
`EncodedFrame` com `seq` por camada. Sem quadro novo por 1 s → reenvia o último (keyframe só se
pedido).

**Backend sintético** (todas as plataformas, usado pelos testes de `zap-core`): gera
`EncodedFrame` com tamanho seguindo o bitrate pedido (keyframe 8× maior, a cada 10 s ou quando
pedido) e conteúdo `[0,0,0,1, 0x65|0x41, …]` (start code + tipo de NAL certo), e `EncodedAudio`
de 20 ms com payload Opus de verdade (silêncio codificado).

## Testes de aceite do marco

1. CI Windows: todos os testes acima passam (os que dependem de GPU/câmera/saída de áudio pulam
   com mensagem quando o recurso não existe no runner).
2. Exemplo `cargo run -p zap-media --example bancada -- --monitor 1 --preset 1080p60 --segundos 10`
   imprime, por camada: fps real, kbps real, tempo médio/p95 de captura, conversão e codificação.
   Meta num PC com GPU dedicada: captura+conversão+codificação p95 ≤ 8 ms na camada 0.
3. `cargo check --workspace --target x86_64-pc-windows-gnu` sem avisos.

## Riscos

| Risco | Mitigação |
|---|---|
| MFT de hardware de fabricante X não aceita alguma opção | Opções opcionais ignoradas; teste por fabricante na bancada; software de reserva |
| Runner do CI sem GPU e talvez sem Media Foundation | Testes pulam com mensagem clara; bancada roda em PC real |
| Borda amarela do WGC no Windows 10 para janelas | Documentar; monitor usa DXGI (sem borda) |
| Câmera ocupada por outro app | Erro claro para a interface ("câmera em uso por outro programa") |

## Estado do fechamento (revisor)

**Pronto e verificado em Linux**: `zap-gpu`, `zap-capture`, `zap-audio`, `zap-encode` e `zap-media`
compilam; a lógica portável (escolha de modo de câmera, ordenação de monitores, Opus, backend
sintético, pipeline sintético) tem testes que passam. `cargo fmt --check`, `cargo clippy
--workspace --all-targets -D warnings` e `cargo test --workspace` (exceto o item abaixo) verdes.
Para `x86_64-pc-windows-gnu`: `cargo check`, `cargo clippy --all-targets -D warnings` e
`cargo test --no-run` verdes (código Windows compila e linka os testes).

**Só será validado no CI Windows / bancada**: tudo que usa WGC/DXGI, D3D11, Media Foundation
(MFT de hardware e software), WASAPI/captura de app e câmera, e os testes `#![cfg(windows)]`
(`zap-capture/tests`, `zap-encode/tests/windows.rs`, `zap-media` em Windows). A meta de p95 <= 8 ms
na camada 0 e o exemplo `bancada` exigem PC real com GPU.

**Riscos e pendências**
- `zap-net/tests/media_loopback.rs` (M1, fora do M2) falha em Linux neste ambiente
  ("quadro N atrasado em loopback"), inclusive no HEAD limpo sem as mudanças do M2: problema
  preexistente, a investigar na rede (admissão por backlog em `media_tx/video.rs`).
- Opus: o teste de ida e volta usa complexity 4 porque com >= 5 o pré-filtro de pitch do CELT
  reduz o SNR de um seno puro a ~16 dB; produção usa complexity 8 (não coberta por esse teste).
- Testes de integração Windows rodam com `allow(unwrap/expect)` (convenção do workspace).
- Riscos de MFT por fabricante, runner sem GPU e câmera ocupada seguem como na tabela acima.

**Rodada 3 (m4c), fechamento**: bancada release, mediana de 5, 1080p60, monitor 1, máquina ociosa: camada 0 a 60,0 fps, 2214 kbps, codificação 3,69 / 6,23 ms (média/p95), p95 somado 6,5 ms. A meta de 8 ms foi atingida na mediana (4 de 5 execuções; uma deu 9,1 ms) e 60 fps >= 57. Descartes de captura mediana 27, de codificação 0. Etapa dominante: codificação; conversão (0,08 ms) e espera de lock (0,17 ms) são desprezíveis; etapa B não necessária. Tabela completa em `m4c-estabilizacao.md` seção 10.

### Revisão do arquiteto
- Corrigido no fechamento: `zap-net` soltava o stream do quadro antes do FIN, o quinn mandava
  STOP_SENDING e o emissor (que espera a confirmação) contava o quadro como não entregue. Agora o
  receptor lê até o FIN (teto de 64 KiB de sobra). Era a falha intermitente do `media_loopback`.
- A seguir (não bloqueia): o laço do MFT de hardware consulta `GetEvent` sem bloquear com
  `sleep(1 ms)`; trocar por `GetEvent` bloqueante + `QueueEvent` para acordar no `stop`.
- O roundtrip do Opus mede SNR com complexidade 4 (o pré-filtro de pitch do CELT, ligado a partir
  da 5, deixa resíduo num tom puro); produção usa 8.

### Primeira rodada numa GPU real (2026-11-04, GTX 1060, NVENC)
Diagnóstico e correções em `docs/planos/m4b-pendencias.md` (§2.3–2.5, frentes `gpu`, `captura`,
`encode`, `media`): `zap-capture --lib` caía (MTA do COM morrendo com fábrica WinRT em uso); a
bancada caía em release (ponteiro do `GpuDevice` pendurado nas três capturas); os timers do
Windows (15,6 ms) derrubavam fps e latência; `tamanho_medio_nao_chave` media cor sólida. O laço do
MFT de hardware passa a ter thread de eventos (item "A seguir" acima). Números da bancada depois
das correções: seção 8 daquele plano (a frente `media` também os copia aqui).
