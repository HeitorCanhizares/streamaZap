# M2b: conversão para NV12 pela CPU (rodada 4)

Plano do arquiteto. Continua `docs/planos/m2-midia-windows.md` (frente `gpu`: conversor por Video
Processor; frente `media`: pipeline de vídeo) e segue `docs/arquitetura-v3.md` §2 (linha "Conversão
de cor/escala", atualizada junto com este plano) e §5. Estado de partida: branch `m4-online`, commit
`fa019f0` (rodada 3 fechada; ver `docs/planos/m4c-estabilizacao.md` §10 e §11).

PC de referência: Windows 10, MSVC, GTX 1060 (NVENC, com Video Processor), **Intel Core i7-6700K
(4 núcleos, 8 threads, 4,0 GHz)**, monitor 1 1920×1080 a 164 Hz (primário), monitor 2 à esquerda.

## 0. Resumo

Hoje um PC sem Video Processor do D3D11 (GPU sem suporte a vídeo, driver básico, máquina virtual,
Área de Trabalho Remota) **não transmite**: `Converter::new` devolve `Unsupported`, `criar_pipe`
falha e a camada tenta de novo a cada 2 s (`RETENTAR_US`) para sempre. Achado novo (2.5): uma
**janela de largura ou altura ímpar** também não transmite, mesmo com Video Processor, porque
`Converter::new` recusa entrada ímpar.

Correção:
1. `zap-gpu` ganha um conversor de CPU (`CpuScaler`, Rust seguro, portável) que reproduz o Video
   Processor da NVIDIA: **bilinear com centros alinhados, luma na grade da saída e croma reamostrado
   na própria grade (metade)**, BT.709 faixa limitada. No protótipo, a diferença contra a GPU foi de
   no máximo 1 nível em todos os padrões e tamanhos (2.2).
2. `zap-gpu` ganha leitura e escrita por staging persistente **sem segurar o lock do dispositivo**
   enquanto a GPU copia (`ReadbackStaging`, `UploadStaging`), `GpuDevice::format_support`,
   `GpuDevice::create_basic`, e o `Converter` passa a aceitar entrada BGRA ímpar.
3. `zap-media` escolhe GPU ou CPU por camada (`ConversionSpec::{Auto, Gpu, Cpu}`, `Auto` padrão):
   uma leitura da origem por quadro, compartilhada pelas camadas na CPU; a conversão escreve direto
   na staging de escrita, que é copiada para a textura NV12 do anel. O codificador não muda. A
   bancada ganha `--conversao`.

## 1. Escopo

Entra:
- `zap-gpu`: módulo portável `cpu` (algoritmo, threads, validação); `staging.rs` (Windows) e stub;
  `FormatSupport` e `GpuDevice::format_support`; `GpuDevice::create_basic`; `Converter::new` aceita
  BGRA ímpar; exemplo `cpu_nv12`; testes portáveis e do Windows (CPU × GPU, adaptador básico, lock).
- `Cargo.toml` da raiz: `[profile.dev.package.zap-gpu] opt-level = 3` (dono: frente `gpu`).
- `zap-media`: `ConversionSpec`, `ConverterKind`, `VideoPipelineOptions` e
  `VideoPipeline::start_with`; escolha GPU/CPU por camada; leitura única por quadro; estatísticas
  novas; bancada `--conversao` e CPU do processo; testes do pipeline com CPU, da troca automática e
  do adaptador básico com o codificador de software; medição.

Fica de fora:
- **Entregar NV12 em memória ao codificador de software** (sem subir para textura e ler de volta):
  portão em 5.2 g, com previsão de ficar fechado.
- **Expor a opção ou o conversor em uso no app** (Configurações ou Diagnóstico). `zap-core` e o app
  não mudam nesta rodada: o núcleo usa `VideoPipeline::start` (= `Auto`). O conversor aparece no log
  (`info`) e nas estatísticas do `zap-media` (bancada e testes). Próximo passo natural: levar
  `LayerStats::converter` ao `PublisherStats` do núcleo e ao Diagnóstico.
- **Filtro de área para reduções grandes** (melhor para a miniatura 360p com texto): só faria
  sentido nos dois caminhos juntos, porque a GPU continua bilinear (decisão em 3).
- Câmera entregando memória direto ao caminho da CPU (hoje o quadro NV12 de memória sobe para
  textura e a CPU lê de volta; 720p30, ~0,3 ms por quadro).
- Contrato de rede: **sem mudança**. CI Windows com a cadeia de dispositivos: continua pendente.

Regras para todas as frentes: uma frente por crate (trava por crate); não afrouxar testes existentes
(o único teste que muda de entrada é o do tamanho ímpar, com motivo em 3); `unsafe` só onde já há
FFI do Windows (`zap-gpu/src/staging.rs`, `zap-media/src/win_video.rs` e a bancada), sempre com
`// SAFETY:`; o módulo `cpu` é `#[forbid(unsafe_code)]`; sem dependência nova; sem commit, push ou
troca de branch; todo comando de cargo começa com
`export PATH="$HOME/.cargo/bin:/c/Program Files/CMake/bin:$PATH";` e, no lugar do
`--target x86_64-pc-windows-gnu`, roda `cargo check --workspace --all-targets`; caminhos absolutos
(raiz `C:/Users/heito/Desktop/Projetos/streamazap`). Só `Core::start` grava
`settings::PRODUCTION_SERVER`; nenhum teste fala com o servidor de produção. Desempenho só vale com
movimento na tela e máquina ociosa (`tasklist | findstr /i "cargo rustc"` vazio): a medida final é
do fechamento. Saídas de medição vão para o scratchpad
`C:/Users/heito/AppData/Local/Temp/claude/a126e729ff4629a0/acd536d0-6c64-4aca-8e82-23354094a821/scratchpad`.

## 2. Medidas do protótipo (2026-11-04, este PC)

Protótipo descartável no scratchpad (pasta `m2b_proto/`, índice em `m2b_LEIAME.txt`), com `zap-gpu`
como dependência. Build release, máquina ociosa. As saídas brutas são os `m2b_*.txt` do scratchpad.
A forma dos laços que vetorizam está em `m2b_proto/src/v2.rs` (`faixa_identidade`) e
`m2b_proto/src/v4.rs` (`linha`). Serve de referência, não de código para copiar: falta validação e
há `unwrap`.

### 2.1 Adaptador básico e WARP (`m2b_probe.txt`)

| Adaptador | `VIDEO_SUPPORT` | NV12 em `CheckFormatSupport` | Textura NV12 `DEFAULT` com `RENDER_TARGET \| SHADER_RESOURCE` | Staging NV12 de leitura e de escrita |
|---|---|---|---|---|
| GTX 1060 | ok; VP aceita BGRA e NV12 na entrada e na saída | `TEXTURE2D`, `RENDER_TARGET`, `SHADER_SAMPLE`, `CPU_LOCKABLE`, `VP_IN`, `VP_OUT`, `VIDEO_ENCODER`, `DECODER_OUT` | ok | ok |
| Microsoft Basic Render Driver (adaptador 1, `DXGI_ADAPTER_FLAG_SOFTWARE`) | **falha** (`DXGI_ERROR_UNSUPPORTED`, 0x887A0004) | `TEXTURE2D`, `RENDER_TARGET`, `SHADER_SAMPLE`, `CPU_LOCKABLE`, `VIDEO_ENCODER` (sem `VP_IN`/`VP_OUT`) | ok | ok |
| `D3D_DRIVER_TYPE_WARP` | falha (idem) | idem ao básico | ok | ok |

Conclusões:
- Num PC sem VP a **textura NV12 do anel existe**. O codificador continua recebendo textura, então
  entregar memória a ele **não é obrigatório**.
- `GpuDevice::create_for_adapter(LUID do básico)` já funciona aqui (`has_video() == false`). Dá para
  testar o caminho sem VP neste PC, que tem VP.

### 2.2 Como o Video Processor da NVIDIA converte (`m2b_cmp*.txt`)

Origem BGRA 1920×1080, comparação CPU × GPU (`Converter` + `read_back`). Padrões: cores sólidas,
gradiente e "tela" (blocos de cor de 37×23, linhas brancas de 1 px, tabuleiro de 1 px em 1 de cada
3 blocos). A tabela mostra a diferença máxima por plano no padrão "tela":

| Modelo testado na CPU | 1080p (1:1) | 720p (1,5:1) | 540p (2:1) | 360p (3:1) |
|---|---|---|---|---|
| Croma = média dos 4 pixels já escalados | máx 1 | Y máx 1; U/V máx 86 (26 dB) | — | U/V máx 80 |
| Área (box) em luma e croma | máx 1 | Y máx 26 (32 dB); U/V máx 84 | Y máx 1; U/V máx 43 | **Y máx 182 (16 dB)** |
| **Bilinear com centros alinhados; luma na grade da saída; croma reamostrado na grade `out/2`** | **máx 1 (≥ 60 dB)** | **máx 1 (≥ 61 dB)** | **máx 1** | **máx 1 (≥ 61 dB)** |

O gradiente dá máximo 1 em todos os modelos. As cores sólidas dão 0. NV12 → NV12 com bilinear por
plano (1280×720 para 1280×720, 960×540 e 640×360): no 1:1 é cópia exata; nos outros, máximo 1
(≥ 54 dB) (`m2b_cmp5.txt`).

Conclusões:
- O VP da NVIDIA usa **bilinear sem pré-filtro** (serrilha em 3:1).
- O croma fica no **centro do bloco 2×2**, apesar de a saída declarar `..._LEFT_P709`, e é amostrado
  direto na grade do croma.

### 2.3 Tempo na CPU (release, padrão "tela" 1920×1080 BGRA em memória, mediana de 40, ms)

| Saída | 1 thread | 2 | 4 | 8 |
|---|---|---|---|---|
| 1080p (identidade, aritmética inteira) | 3,1 | 1,7 | **1,0–1,1** | 1,0 |
| 720p (bilinear, croma na grade própria) | 9,0 | 4,7 | **3,4** | 2,7 |
| 360p (idem) | 2,4 | 1,3 | **1,0** | 0,9 |

- A primeira versão, com f32 sem vetorizar e pixel por pixel, dava 10,2 ms em 1080p com 1 thread.
  A **passada vertical "plana"** (um f32 por byte, inclusive o alfa) vetoriza e corta ~40% no
  escalado. A **identidade em inteiro** é 3× mais rápida que em f32.
- `std::thread::scope` com 3 threads novas custa **144–152 µs** por chamada. Pilha de 256 KiB não
  muda isso.
- Converter lendo direto da staging de leitura mapeada e escrevendo direto na de escrita custa o
  mesmo que memória comum (1080p identidade, 4 threads: memória → memória 1,15 ms; staging →
  memória 1,30; staging → staging 1,22).

### 2.4 Leitura e escrita por staging persistente, 1080p (`m2b_io*.txt`, ms)

| Operação | GTX 1060 | Básico (WARP) |
|---|---|---|
| `CopyResource` BGRA (8,3 MB) + `Map` bloqueante | 0,77 (p95 1,26) | 0,88 (p95 1,12) |
| O mesmo com `DO_NOT_WAIT` + `yield_now` | 0,76 (p95 0,96), ~880 tentativas | 0,87 (p95 1,50) |
| NV12 (3,1 MB): `Map` de escrita + escrita + `Unmap` + `CopyResource`, 2 stagings alternadas | 0,32 (p95 0,56); o `Map` nunca esperou | 0,57 (p95 0,65) |

O `Map` bloqueante **segura o lock do dispositivo durante a cópia**: ~0,8 ms por quadro a 60 fps em
que captura e MFTs de hardware esperam. Com `DO_NOT_WAIT`, o lock só fica preso durante cada
tentativa (µs).

### 2.5 Entrada ímpar (`m2b_impar.txt`)

O WGC entrega a janela no tamanho real (`ContentSize`), que pode ser ímpar. A escada arredonda a
saída para par (`build_ladder`), mas `Converter::new` recusa entrada ímpar, então a camada nunca
liga. O VP da NVIDIA aceita BGRA 641×361, 1001×777 e 1919×1079: enumerador, processador e visão de
entrada são criados.

## 3. Decisões

| Decisão | Motivo |
|---|---|
| Filtro de escala da CPU: **bilinear com centros alinhados** (`s = (o + 0,5)·in/out − 0,5`, preso em `[0, in − 1]`). Luma na grade da saída; U e V a partir do RGB reamostrado na grade `out/2` (entrada NV12: plano UV reamostrado na grade `out/2`) | 2.2: fica igual ao VP da NVIDIA dentro de 1 nível em qualquer conteúdo. Quem assiste vê a mesma imagem, venha ela da GPU ou da CPU, e o teste CPU × GPU pode ser estrito. 2 taps fixos é o mais barato e o mais simples. A **média de área para reduções grandes foi descartada nesta rodada**: só o caminho da CPU mudaria (a GPU continua bilinear, com a mesma serrilha), e o híbrido (área a partir de 2:1) mediu 75% mais lento no 360p (1,84 contra 1,04 ms com 4 threads; `m2b_v4.txt` e `m2b_v4_bilinear.txt`) |
| Identidade (mesmo tamanho, BGRA) em aritmética inteira; o resto em f32, com a passada vertical plana | 2.3 |
| Threads com `std::thread::scope`: `cpu_threads() = clamp(paralelismo / 2, 1, 4)` (4 aqui). Faixas de 64 linhas de luma distribuídas em rodízio entre as threads. **No máximo 2 escopos por quadro**: a camada da CPU de menor índice primeiro, sozinha; depois as outras juntas | Com 4 threads, 1080p fica em ~1 ms e 720p em ~3,4 ms. Metade dos núcleos fica para o codificador (o de software é pesado) e para o sistema. O escopo custa ~0,15 ms (2.3): 2 por quadro a 60 fps dão ~2% de um núcleo. **Sem dependência nova**: o `rayon` pouparia esses ~0,3 ms por quadro, o que não paga a dependência. A camada 0 sozinha primeiro reduz o atraso dela nos quadros em que a camada 1 também está na vez (~1,1 ms em vez de ~4,5 ms) |
| Leitura: uma `ReadbackStaging` persistente por pipeline. `CopyResource` + `Flush`, depois `Map(DO_NOT_WAIT)`: `yield_now` até 2 ms, `sleep(1 ms)` depois, desiste em 100 ms. A conversão lê direto do mapeamento | 2.4: o lock não fica preso durante a cópia, e não há memcpy extra |
| Escrita: **duas stagings NV12 alternadas por camada**. A conversão escreve direto no mapeamento; depois `Unmap` + `CopyResource` para a textura do anel; o codificador recebe textura, como hoje | 2.1: NV12 existe no WARP. 2.4: com 2 stagings, o `Map` nunca esperou |
| Entrega de memória ao codificador de software: **não nesta rodada** (portão 5.2 g) | Economia possível de ~1 ms por quadro 1080p: subida (~0,3 ms), mais a leitura e as duas cópias dentro do codificador (~0,7 ms). A codificação por software deve passar de 10 ms. Exige mudar `zap-encode` (`Job` com memória) e `zap-media` |
| Modo `Auto`: **qualquer erro** de `Converter::new` (ou 3 falhas seguidas de `convert`) leva a camada para a CPU. `Gpu`: nunca cai para a CPU (repete a cada 2 s, como hoje). `Cpu`: nem tenta a GPU | O erro do VP varia por driver (`Unsupported`, `Os`). O caminho da CPU funciona com qualquer tamanho e formato válidos |
| A opção vai em `VideoPipelineOptions` (`VideoPipeline::start_with`), **não em `VideoPipelineConfig`** | `VideoPipelineConfig` é montado em 9 lugares do `zap-core`. Um campo novo obrigaria uma frente `core` só para escrever `Auto`. O app não expõe a opção (seção 1) |
| `Converter::new` aceita BGRA de qualquer tamanho ≥ 1×1. Exige par só no NV12 (entrada) e na saída | 2.5. O teste `new_tamanho_impar_recusa` (BGRA 641×360) vira dois: NV12 ímpar recusa e saída ímpar recusa. Entra também um teste de cores com BGRA ímpar. A regra que continua valendo segue testada, e o conjunto fica mais estrito, não mais frouxo |
| `[profile.dev.package.zap-gpu] opt-level = 3` | Os testes rodam em debug. Sem otimização, o laço de pixels (sem vetorização e com checagem de limites) fica dezenas de vezes mais lento, e os testes do pipeline com `Cpu` (camada 720p a 30 fps) não teriam fps. Custo: só a compilação do `zap-gpu` em dev |

## 4. Frentes

| Id | Crate | Depende de | Resumo |
|---|---|---|---|
| `gpu` | `crates/zap-gpu` (mais a seção `[profile.dev.package.zap-gpu]` do `Cargo.toml` da raiz) | — | Conversor de CPU portável (`CpuScaler`, `convert_batch`, `cpu_threads`); stagings persistentes sem segurar o lock; `format_support`; `create_basic`; BGRA ímpar no `Converter`; exemplo `cpu_nv12`; testes portáveis e do Windows |
| `media` | `crates/zap-media` | `gpu` | `ConversionSpec`, `ConverterKind`, `VideoPipelineOptions`/`start_with`; escolha GPU/CPU por camada; leitura única por quadro em 2 fases; estatísticas; bancada `--conversao`; testes do pipeline com CPU, da troca automática e do adaptador básico; medição |

`media` pode começar quando a API da seção 5.1 existir no esqueleto (assinaturas reais, compilando).
Os testes de `media` que rodam o pipeline precisam de `gpu` pronto.

## 5. Frentes em detalhe

### 5.1 `gpu` (`crates/zap-gpu`)

**a) Algoritmo: `src/cpu/`, portável.** Em `lib.rs`: `#[forbid(unsafe_code)] mod cpu;` sem `cfg`
(compila e é testado no Linux), e
`pub use cpu::{convert_batch, cpu_threads, BatchOptions, CpuScaler, Nv12Mut, SrcImage};`.
Arquivos:
- `cpu/mod.rs`: API, validação, lote e threads;
- `cpu/eixo.rs`: pesos;
- `cpu/bgra.rs`: BGRA → NV12;
- `cpu/nv12.rs`: NV12 → NV12.

```rust
use crate::{GpuError, PixelFormat};

/// Imagem de origem na memória (pode ser uma staging mapeada).
#[derive(Clone, Copy, Debug)]
pub enum SrcImage<'a> {
    /// B, G, R, A por pixel (alfa ignorado). `stride` em bytes, >= width * 4.
    Bgra { data: &'a [u8], stride: usize, width: u32, height: u32 },
    /// Plano Y e plano UV intercalado (Cb, Cr), BT.709 faixa limitada. `width` e `height` pares;
    /// strides >= width.
    Nv12 { y: &'a [u8], y_stride: usize, uv: &'a [u8], uv_stride: usize, width: u32, height: u32 },
}
impl SrcImage<'_> {
    pub fn width(&self) -> u32;
    pub fn height(&self) -> u32;
    pub fn format(&self) -> PixelFormat;
}

/// Destino NV12 na memória (pode ser uma staging mapeada).
#[derive(Debug)]
pub struct Nv12Mut<'a> {
    pub y: &'a mut [u8],
    pub y_stride: usize,
    pub uv: &'a mut [u8],
    pub uv_stride: usize,
    pub width: u32,
    pub height: u32,
}
impl<'a> Nv12Mut<'a> {
    /// Buffer único no formato do mapeamento de uma textura NV12 e do `read_back`: `height` linhas
    /// de Y e `height / 2` de UV, todas com `stride` bytes, UV começando em `stride * height`.
    /// Precisa de ao menos `stride * (height + height / 2 - 1) + width` bytes. Buffer curto,
    /// `stride < width`, tamanho zero ou ímpar: `InvalidArgument`.
    pub fn from_contiguous(buf: &'a mut [u8], stride: usize, width: u32, height: u32) -> Result<Self, GpuError>;
}

/// Conversor/escalador de CPU para um par (entrada, saída), com os pesos já calculados.
#[derive(Debug, Clone)]
pub struct CpuScaler { /* tamanhos, formato de entrada e 4 eixos: luma x/y, croma x/y */ }
impl CpuScaler {
    /// Entrada BGRA: qualquer tamanho >= 1×1; NV12: par >= 2×2. Saída: par >= 2×2. Cada lado
    /// <= 16384. Fora disso: `InvalidArgument`.
    pub fn new(in_w: u32, in_h: u32, in_fmt: PixelFormat, out_w: u32, out_h: u32) -> Result<Self, GpuError>;
    pub fn input(&self) -> (u32, u32, PixelFormat);
    pub fn output(&self) -> (u32, u32);
    /// Atalho para `convert_batch` com um trabalho só.
    pub fn convert(&self, src: &SrcImage<'_>, dst: &mut Nv12Mut<'_>, opts: BatchOptions) -> Result<(), GpuError>;
}

#[derive(Clone, Copy, Debug)]
pub struct BatchOptions {
    /// Threads ao todo, contando a que chama; 0 conta como 1.
    pub threads: usize,
    /// Roda no início de cada thread criada (não na que chama). O `zap-media` usa para a prioridade.
    pub on_thread_start: Option<fn()>,
}
impl Default for BatchOptions { /* threads: cpu_threads(), on_thread_start: None */ }

/// Converte várias camadas do mesmo quadro numa passada só. Valida tudo antes (com qualquer
/// trabalho inválido, `InvalidArgument` e nada escrito): a entrada do `CpuScaler` igual a `src`
/// (tamanho e formato), o destino do tamanho da saída, as fatias longas o bastante para os strides.
pub fn convert_batch(src: &SrcImage<'_>, jobs: &mut [(&CpuScaler, Nv12Mut<'_>)], opts: BatchOptions) -> Result<(), GpuError>;

/// `clamp(available_parallelism / 2, 1, 4)`; 2 se o sistema não informar.
pub fn cpu_threads() -> usize;
```

Comportamento (números exatos):
- **Pesos** (`eixo.rs`): `pub(crate) struct Tap { pub i0: u32, pub i1: u32, pub f: f32 }` e
  `pub(crate) fn bilinear(inp: u32, out: u32) -> Vec<Tap>`. Para cada `o`, em f64:
  `s = ((o + 0,5)·inp/out − 0,5).clamp(0, inp − 1)`, `i0 = floor(s)`, `i1 = min(i0 + 1, inp − 1)`,
  `f = s − i0` (guardado em f32). Peso de `i0` = `1 − f`; peso de `i1` = `f`.
- **Eixos**:
  - BGRA: luma `bilinear(in_w, out_w)` × `bilinear(in_h, out_h)`; croma `bilinear(in_w, out_w/2)` ×
    `bilinear(in_h, out_h/2)`.
  - NV12: luma igual; croma `bilinear(in_w/2, out_w/2)` × `bilinear(in_h/2, out_h/2)` sobre o
    plano UV.
- **Cor** (R, G, B de 0 a 255 em f32):
  - `Y = 16 + KYR·R + KYG·G + KYB·B`, com `KYR = 0,2126·219/255`, `KYG = 0,7152·219/255` e
    `KYB = 0,0722·219/255`;
  - `Yl = 0,2126·R + 0,7152·G + 0,0722·B`;
  - `U = 128 + (224/255)·(B − Yl)/1,8556`;
  - `V = 128 + (224/255)·(R − Yl)/1,5748`.

  Para virar u8: `(x + 0.5) as u8`. O `as` satura em 0 e 255; não usar `round()`. A mesma regra vale
  em todo o módulo.
- **BGRA identidade** (`in == out`), em aritmética inteira:
  - `Y = (11966·R + 40255·G + 4064·B + (16 << 16) + 32768) >> 16`, em u32;
  - croma de cada bloco 2×2 a partir das somas `Sb`, `Sg`, `Sr` dos 4 pixels, em i32:
    `U = (1799·Sb − 412·Sr − 1387·Sg + (128 << 14) + 8192) >> 14` e
    `V = (1799·Sr − 1634·Sg − 165·Sb + (128 << 14) + 8192) >> 14`, presos em 0..=255.

  É o mesmo que o bilinear em 2:1, onde `f = 0,5`.
- **BGRA com escala.** Para cada linha `y` de luma da faixa:
  1. mistura vertical das linhas `i0` e `i1` da origem num rascunho `col` de `in_w·4` f32:
     `col[k] = (1 − f)·a[k] + f·b[k]` sobre os bytes, inclusive o alfa. É um laço plano, que
     vetoriza. Com `f == 0`, usa só `a`;
  2. horizontal: para cada `x`, `p = (1 − f)·col[i0] + f·col[i1]` com `[f32; 4]`;
  3. `Y`.

  Para cada linha `c` de croma da faixa (de `y0/2` a `(y0 + linhas)/2`), o mesmo com os eixos de
  croma, gerando `U` e `V`.
- **NV12 → NV12**: na identidade, copia as linhas. Com escala, o plano Y (1 canal) e o plano UV
  (2 canais intercalados) passam pelo mesmo esquema (vertical plano + 2 taps horizontais),
  arredondando com `(x + 0.5) as u8`.
- **Lote**:
  - cada destino é dividido em faixas de 64 linhas de luma (32 de UV; a última pode ser menor e
    sempre é par), com `chunks_mut` de `y` e de `uv`;
  - as faixas ficam em ordem (trabalho 0 inteiro, depois o 1, e assim por diante), e a faixa `i` vai
    para a thread `i % threads`;
  - `std::thread::scope` cria `threads − 1` threads; cada uma chama `on_thread_start` e faz a sua
    parte, e a que chama faz a parte 0. Com `threads == 1` não há escopo;
  - cada thread aloca o rascunho uma vez (`max(in_w)·4` f32 e duas linhas `[f32; 4]` do maior
    `out_w`);
  - sem `Mutex`, sem `unsafe`, sem `unwrap`/`expect`.
- O resultado **não depende** do número de threads (cada linha de saída só depende da origem).

Testes portáveis (`cargo test -p zap-gpu`, no Linux e no Windows), nos `#[cfg(test)]` de cada
arquivo:
1. `eixo_casos_conhecidos`:
   - `bilinear(n, n)`: `i0 = o`, `f = 0`;
   - `bilinear(2n, n)`: `i0 = 2o`, `i1 = 2o + 1`, `f = 0,5`;
   - `bilinear(1080, 360)`: `i0 = 3o + 1`, `f = 0`;
   - `bilinear(1920, 1280)`: `o = 0` fica preso em 0 (`f = 0`); `o = 1` dá `s = 1,75` (`i0 = 1`,
     `f = 0,75`);
   - `bilinear(1, 4)`: tudo 0.
2. `cores_puras`: cores (255,0,0), (0,255,0), (0,0,255), (255,255,255), (0,0,0), (128,128,128) e
   (10,200,90), nos tamanhos 64×64 → 64×64, 1920×1080 → 1280×720, 1920×1080 → 640×360,
   1001×777 → 1002×778 e 641×361 → 642×360. Todo Y, U e V fica a ±1 da fórmula em f64,
   arredondada.
3. `identidade_igual_formula_inteira`: BGRA 64×32 pseudoaleatório (LCG de semente fixa). Cada Y, U
   e V é exatamente igual às fórmulas inteiras.
4. `escala_3_1_pega_o_pixel_do_meio`: 1920×1080 → 640×360.
   - Branco só onde `x % 3 == 1` e `y % 3 == 1` (o resto preto): todo Y = 235.
   - Branco só onde `x % 3 == 0` e `y % 3 == 0`: todo Y = 16.

   Prova os centros alinhados.
5. `gradiente_monotono`: `R = x·255/(w − 1)`, 1920×1080 → 1280×720. Em cada linha, Y nunca diminui
   da esquerda para a direita.
6. `threads_nao_mudam_nada`: conteúdo pseudoaleatório. BGRA 1920×1080 → 1080p, 720p e 360p, e NV12
   1280×720 → 640×360, com 1, 3, 4 e 8 threads: bytes idênticos.
7. `lote_igual_separado`: `convert_batch` com 3 trabalhos dá o mesmo que 3 `convert` separados.
8. `stride_e_pitch`: origem com stride `w·4 + 64` e destino com stride `w + 32` (via
   `from_contiguous`, preenchido antes com 0xEE). As posições úteis ficam iguais às do caso
   compacto, e os bytes de sobra continuam 0xEE.
9. `nv12_entrada`:
   - NV12 sólido (63, 102, 240) 1280×720 → 640×360 e → 960×540: tudo igual à cor;
   - identidade de conteúdo aleatório: cópia exata.
10. `argumentos_invalidos`: saída ímpar ou zero; NV12 de entrada ímpar; lado acima de 16384; origem
    de outro tamanho ou formato; destino de outro tamanho; buffer curto; stride menor que a linha.
    Em todos os casos: `InvalidArgument` e destino intacto.
11. `from_contiguous_separa`: `y` cobre `stride·height` e `uv` começa em `stride·height`.
12. `cpu_threads_entre_1_e_4`.

**b) Stagings: `src/staging.rs` (Windows), com os mesmos nomes em `stub.rs`.** Reexportar em
`lib.rs` (no Windows, de `staging`; nos outros sistemas, do stub):
`ReadbackStaging, MappedFrame, ReadbackTiming, UploadStaging, MappedUpload, UploadTiming`.

```rust
#[derive(Debug, Clone, Copy, Default)]
pub struct ReadbackTiming {
    /// Do `CopyResource` ao `Map` que deu certo (µs): cópia na GPU + espera.
    pub copy_wait_us: f64,
    /// Soma das esperas pelo lock do dispositivo nas chamadas desta leitura (µs).
    pub lock_wait_us: f64,
    /// Tentativas de `Map`.
    pub map_attempts: u32,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct UploadTiming {
    /// Soma das esperas pelo lock (`Map`, `Unmap`, `CopyResource`) (µs).
    pub lock_wait_us: f64,
    pub map_attempts: u32,
}

/// Staging de leitura persistente (`D3D11_USAGE_STAGING`, `CPU_ACCESS_READ`) de um tamanho e formato.
pub struct ReadbackStaging { /* textura, largura, altura, formato */ }
impl ReadbackStaging {
    pub fn new(dev: &GpuDevice, width: u32, height: u32, format: PixelFormat) -> Result<Self, GpuError>;
    pub fn matches(&self, width: u32, height: u32, format: PixelFormat) -> bool;
    /// Copia `src` (mesmo tamanho e formato; senão `InvalidArgument`) e mapeia para leitura.
    pub fn map<'s>(&'s mut self, dev: &'s GpuDevice, src: &Texture) -> Result<(MappedFrame<'s>, ReadbackTiming), GpuError>;
}
/// Leitura mapeada; `Unmap` no `Drop`.
pub struct MappedFrame<'s> { /* dispositivo, staging, ponteiro, RowPitch, tamanho, formato */ }
impl MappedFrame<'_> {
    /// Bgra: `stride = RowPitch`. Nv12: Y e UV com `RowPitch`, UV em `RowPitch * height`.
    pub fn image(&self) -> SrcImage<'_>;
}

/// Duas stagings NV12 de escrita (`CPU_ACCESS_WRITE`) usadas em rodízio.
pub struct UploadStaging { /* 2 texturas, próxima, largura, altura */ }
impl UploadStaging {
    /// Tamanho par (senão `InvalidArgument`).
    pub fn new(dev: &GpuDevice, width: u32, height: u32) -> Result<Self, GpuError>;
    pub fn size(&self) -> (u32, u32);
    /// Mapeia para escrita a próxima staging do rodízio.
    pub fn map<'s>(&'s mut self, dev: &'s GpuDevice) -> Result<MappedUpload<'s>, GpuError>;
}
/// Escrita mapeada; `Unmap` no `Drop` se não houve `commit`.
pub struct MappedUpload<'s> { /* ... */ }
impl MappedUpload<'_> {
    /// Destino para o conversor: Y em `[0, RowPitch·h)`, UV logo depois (`split_at_mut`).
    pub fn nv12(&mut self) -> Nv12Mut<'_>;
    /// `Unmap` + `CopyResource(dst, staging)`. `dst` deve ser NV12 do mesmo tamanho; se não for,
    /// faz o `Unmap` e devolve `InvalidArgument`.
    pub fn commit(self, dst: &Texture) -> Result<UploadTiming, GpuError>;
}
```

Comportamento:
- **Lock.** Cada chamada (`CopyResource` + `Flush` juntos, cada `Map`, o `Unmap`, o `CopyResource`
  do `commit`) fica entre `dev.lock()` e o fim do guarda, e a espera entra na soma `lock_wait_us`.
- **`Map` sem bloquear.** `Map(.., D3D11_MAP_FLAG_DO_NOT_WAIT)`:
  - com `DXGI_ERROR_WAS_STILL_DRAWING`, solta o lock e espera: `std::thread::yield_now()` enquanto a
    espera total for menor que 2 ms, `std::thread::sleep(1 ms)` depois;
  - passou de 100 ms: `GpuError::Os { context: "Map (leitura)" ou "Map (escrita)", message: "tempo
    esgotado (100 ms)" }`;
  - outro erro: `Os`.

  A escrita usa `D3D11_MAP_WRITE` com a mesma espera.
- **Fatias.** A partir de `pData`, Bgra usa `pitch·(h − 1) + w·4` bytes e Nv12 usa
  `pitch·(h + h/2 − 1) + w` bytes. `// SAFETY:` o D3D11 mapeia o subrecurso inteiro com `RowPitch`
  por linha (o mesmo cálculo do `read_back`), e a fatia vive enquanto o guarda (`'s`) vive.
  `MappedFrame` e `MappedUpload` não são `Send` (guardam COM). As fatias que entregam são comuns
  (`&[u8]` e `&mut [u8]`) e podem ir para as threads do `scope`.
- `read_back` e `upload` não mudam (testes e câmera).
- **Stub**: `new` devolve `Unsupported`; `MappedFrame` e `MappedUpload` têm um campo
  `std::convert::Infallible` e os métodos fazem `match self.never {}` (sem `unreachable!`).

**c) Dispositivo e `Converter`.**
```rust
// types.rs (portável; reexportado em lib.rs)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FormatSupport {
    pub texture2d: bool,
    pub render_target: bool,
    pub cpu_lockable: bool,
    pub video_processor_input: bool,
    pub video_processor_output: bool,
}
// device.rs
impl GpuDevice {
    /// `ID3D11Device::CheckFormatSupport`; em erro, tudo `false`.
    pub fn format_support(&self, format: PixelFormat) -> FormatSupport;
    /// Primeiro adaptador com `DXGI_ADAPTER_FLAG_SOFTWARE` ("Microsoft Basic Render Driver", WARP),
    /// criado sem vídeo. Existe em todo Windows 10/11; serve para testar o caminho sem Video
    /// Processor num PC que tem um.
    pub fn create_basic() -> Result<Self, GpuError>;
}
```
- Stub: `format_support` devolve `FormatSupport::default()`; `create_basic` devolve `Unsupported`.
- `Converter::new` valida `in_w, in_h >= 1`; NV12 na entrada: par; saída: par e >= 2. O resto do
  `Converter` não muda.
- Testes:
  - `new_tamanho_impar_recusa` vira `new_nv12_impar_recusa` (NV12 641×360 → `InvalidArgument`) e
    `new_saida_impar_recusa` (BGRA 640×360 → 641×360 → `InvalidArgument`);
  - `tests/windows_gpu.rs` ganha `cores_bt709_entrada_impar`: BGRA 641×361 → 642×360 com as 5 cores
    do `cores_bt709`, ±3 como lá.

**d) `Cargo.toml` da raiz:** acrescentar, depois de `[profile.dev.package."*"]`:
```toml
# Conversor de CPU (laços de pixel): sem otimização os testes do caminho da CPU ficam lentos demais.
[profile.dev.package.zap-gpu]
opt-level = 3
```

**e) Exemplo `examples/cpu_nv12.rs`** (portável, sem argumentos).
- Gera o padrão "tela" 1920×1080: blocos de 37×23 de cor pseudoaleatória (LCG de semente 12345),
  linhas brancas de 1 px nas bordas dos blocos e tabuleiro preto de 1 px nos blocos em que
  `(bx + by) % 3 == 0`.
- Mede 3 aquecimentos + 40 medidas de cada caso: 1080p, 720p, 360p, "720p + 360p" num lote, "as 3"
  num lote e NV12 1280×720 → 640×360.
- Cada caso roda com 1 thread e com `cpu_threads()`.
- Imprime média, mediana e p95 em ms, e o número de threads.

Uso no fechamento: `cargo run --release -p zap-gpu --example cpu_nv12`.

**f) Testes do Windows: `tests/cpu_windows.rs`** (`#![cfg(windows)]`, com o mesmo
`allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)` do `windows_gpu.rs`). Sem GPU, ou
sem VP onde o teste precisa dele, pula com `eprintln!("PULANDO: ...")`. PSNR por plano:
`10·log10(255² / EQM)`; 99 se `EQM == 0`.
1. `cpu_igual_gpu_cores_e_gradiente`:
   - VP do adaptador padrão; origem BGRA 1920×1080 subida com `upload`;
   - saídas 1920×1080, 1280×720, 960×540 e 640×360;
   - GPU (`Converter` + `read_back`) × CPU (`ReadbackStaging` da mesma textura + `CpuScaler`);
   - as 5 cores sólidas do `cores_bt709`: diferença máxima ≤ 1 em Y, U e V;
   - gradiente (`R = x·255/(w − 1)`, `G = y·255/(h − 1)`, `B = (x + y)·255/(w + h − 2)`): máxima ≤ 2
     e PSNR ≥ 50 dB em cada plano (medido: máx 1, 60–65 dB);
   - imprime os números.
2. `cpu_igual_gpu_conteudo_nitido`: padrão "tela" (o mesmo de 5.1 e). Com `Vendor::Nvidia`: máxima
   ≤ 2 e PSNR ≥ 50 dB por plano (medido: 1 e ≥ 61 dB). Com outro fabricante, só imprime (o filtro
   do VP varia; ver 2.2).
3. `nv12_cpu_igual_gpu`: NV12 1280×720 (gradiente e "tela" convertidos pela CPU, subidos com
   `upload`) → 1280×720, 960×540 e 640×360. 1:1 igual byte a byte; nos outros, máxima ≤ 2 e PSNR
   ≥ 50 dB (medido: 1 e ≥ 54 dB).
4. `cpu_igual_gpu_entrada_impar`: BGRA 1001×777 → 1002×778 e 641×361 → 642×360 com as 5 cores
   sólidas: máxima ≤ 1. Depende de 5.1 c.
5. `readback_igual_read_back`: BGRA 640×360 e NV12 640×360 com conteúdo aleatório.
   `MappedFrame::image()`, linha a linha, fica igual ao `read_back`. Duas leituras seguidas da mesma
   staging com conteúdos diferentes: a segunda vê o novo.
6. `upload_staging_alterna`: 10 iterações de `map`, escrita de um padrão diferente por iteração via
   `Nv12Mut`, `commit` numa textura NV12 e `read_back`. O lido é igual ao padrão. Em pelo menos 9
   das 10 iterações, o `map` dá certo na primeira tentativa.
7. `readback_nao_segura_o_lock`:
   - origem BGRA 3840×2160 (33 MB, cópia de ~3 ms);
   - 20 leituras seguidas numa thread, enquanto outra faz `dev.lock()` a cada 200 µs e guarda a
     espera;
   - p95 da espera da outra thread ≤ 0,5 ms (com `Map` bloqueante o lock ficaria preso ~3 ms por
     leitura e o p95 passaria de 2 ms);
   - em toda leitura, `copy_wait_us > 0` e `map_attempts >= 1`.
8. `adaptador_basico_sem_vp`: `GpuDevice::create_basic()` (pula se der `Err`).
   - `has_video() == false`;
   - `Converter::new(..)` devolve `Unsupported`;
   - `format_support(PixelFormat::Nv12)` tem `texture2d` e `cpu_lockable`;
   - caminho inteiro no básico: BGRA 640×360 (gradiente) por `upload` → `ReadbackStaging` →
     `CpuScaler` (640×360 → 640×360 e → 320×180) → `UploadStaging` → textura de um `TextureRing`
     NV12 → `read_back`. O lido é igual, byte a byte, à conversão feita direto na memória.
9. `format_support_nv12`: no adaptador padrão, NV12 com `texture2d`; se `has_video()`, também
   `video_processor_output`.

Verificação da frente: `cargo test -p zap-gpu`, `cargo clippy -p zap-gpu --all-targets -- -D
warnings`, `cargo fmt --all --check` e `cargo run --release -p zap-gpu --example cpu_nv12` (só para
conferir que roda; a medida vale no fechamento).

### 5.2 `media` (`crates/zap-media`)

**a) Tipos** (`types.rs`, exportados em `lib.rs`):
```rust
/// Onde converter para NV12.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ConversionSpec {
    /// GPU (Video Processor) quando der; senão CPU.
    #[default]
    Auto,
    /// Só GPU: sem Video Processor a camada não liga (tenta de novo a cada 2 s, como hoje).
    Gpu,
    /// Só CPU (testes e bancada).
    Cpu,
}

/// Conversor em uso numa camada.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConverterKind { Gpu, Cpu }

/// Opções do pipeline que o núcleo não precisa conhecer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VideoPipelineOptions {
    pub conversion: ConversionSpec,
}

pub struct LayerStats {
    /* como hoje + */
    /// Conversor da camada; `None` = desligada (ou backend sintético).
    pub converter: Option<ConverterKind>,
}
pub struct PipelineStats {
    /* como hoje + */
    /// Leitura da origem para a CPU: do `CopyResource` ao `Map` (µs). Vazio sem camada na CPU.
    pub cpu_readback: StageStats,
    /// Threads da conversão na CPU (0 = nenhuma camada na CPU).
    pub cpu_threads: u32,
}
```
`video.rs`:
```rust
impl VideoPipeline {
    /// `start_with(cfg, out, VideoPipelineOptions::default())`. Assinatura e comportamento de hoje.
    pub fn start(cfg: VideoPipelineConfig, out: Sender<EncodedFrame>) -> Result<Self, MediaError>;
    /// O backend sintético ignora `opts`.
    pub fn start_with(cfg: VideoPipelineConfig, out: Sender<EncodedFrame>, opts: VideoPipelineOptions) -> Result<Self, MediaError>;
    /// Só nos testes do crate: finge que não há Video Processor (`Converter::new` nem é chamado; a
    /// tentativa devolve `GpuError::Unsupported`).
    #[cfg(all(test, windows))]
    pub(crate) fn start_sem_vp(cfg: VideoPipelineConfig, out: Sender<EncodedFrame>, opts: VideoPipelineOptions) -> Result<Self, MediaError>;
}
```
`win_video::spawn` passa a receber `opts: VideoPipelineOptions` e `simular_sem_vp: bool`. No
`synth_video.rs`, os literais de `LayerStats` ganham `converter: None`.

**b) Escolha** (`logic.rs`, portável, privada do crate):
```rust
pub(crate) enum Escolhido<T> { Gpu(T), Cpu { motivo: Option<String> } }
/// `Auto`: chama `tentar_gpu`; `Ok` → `Gpu`; qualquer erro → `Cpu` com o texto do erro.
/// `Gpu`: `Ok` → `Gpu`; erro → `Err` (sobe). `Cpu`: não chama `tentar_gpu`; `Cpu { motivo: None }`.
pub(crate) fn escolher_conversor<T>(
    modo: ConversionSpec,
    tentar_gpu: impl FnOnce() -> Result<T, zap_gpu::GpuError>,
) -> Result<Escolhido<T>, zap_gpu::GpuError>;
```
Testes portáveis: `Auto` com `Ok`, com `Unsupported` e com `Os`; `Gpu` com `Ok` e com `Unsupported`;
`Cpu` sem chamar a closure (flag num `Cell<bool>`).

**c) `win_video.rs`.**
```rust
enum Conversor { Gpu(Converter), Cpu(CamadaCpu) }
struct CamadaCpu { scaler: CpuScaler, upload: UploadStaging }
// `Pipe` como hoje, com `conv: Conversor`.
// `Camada` ganha `falhas_gpu: u32` e `forcar_cpu: bool` (zerados em `Camada::nova`).
```

`criar_conversor(dev, modo, simular_sem_vp, layer, in_w, in_h, in_fmt, out_w, out_h) ->
Result<Conversor, MediaError>`, usado por `criar_pipe` e pelo ramo "entrada mudou" de `processar`
(que hoje chama `Converter::new` direto):
1. Modo efetivo: `Cpu` se `cam.forcar_cpu`; senão o de `opts`.
2. `escolher_conversor(modo, || if simular_sem_vp { Err(GpuError::Unsupported) } else {
   Converter::new(..) })`.
3. `Gpu(c)`: `enable_gpu_timing`, como hoje.
4. `Cpu { motivo }`:
   - exige `dev.format_support(PixelFormat::Nv12)` com `texture2d && cpu_lockable`. Sem isso:
     `warn!("camada {layer}: o adaptador não tem textura NV12; sem conversão possível")` e erro
     `MediaError::Gpu(GpuError::Unsupported)` (a camada tenta de novo em 2 s);
   - cria `CpuScaler::new(in_w, in_h, in_fmt, out_w, out_h)` e `UploadStaging::new(dev, out_w,
     out_h)`;
   - `info!("camada {layer}: conversão pela CPU ({motivo ou \"pedida\"}), {threads} threads")`.

Estado novo em `executar`: `leitura: Option<ReadbackStaging>`, `leitura_s: Samples`,
`threads = zap_gpu::cpu_threads()` e `ultimo_aviso_leitura_us: u64`.

`processar`, na ordem:
1. Garante os pipes como hoje, criando o conversor com `criar_conversor`.
2. `devidas`: índices das camadas pedidas, com pipe, cujo `gate.ready(agora)` deu `true`. O gate é
   consultado uma vez por camada, como hoje.
3. **GPU**: para cada camada devida com `Conversor::Gpu`, faz o que faz hoje (convert, estatísticas,
   submit).
   - Uma falha de `convert` soma `falhas_gpu`; um sucesso zera.
   - Com 3 falhas seguidas no modo `Auto`: `warn!`, `cam.forcar_cpu = true` e troca `p.conv` por
     `criar_conversor(..)`, que vai dar CPU. O codificador fica, e o próximo quadro sai keyframe
     (`cam.primeiro = true`).
   - `forcar_cpu` volta a `false` só quando a escada é refeita (`construir`).
4. **CPU**: `cpu` = devidas com `Conversor::Cpu`, em ordem de camada. Se não estiver vazia:
   1. Se a `leitura` não existe ou não bate (`matches(w, h, fmt)`): `ReadbackStaging::new`. Em erro:
      aviso limitado (ver abaixo) e as camadas da CPU pulam este quadro.
   2. `t0 = Instant::now()`; `leitura.map(dev, &frame.texture)` → `(mapeado, rt)`;
      `leitura_s.push(rt.copy_wait_us)`. Em erro: **aviso limitado** (o primeiro e depois no máximo 1
      por segundo, por `ultimo_aviso_leitura_us`) e as camadas da CPU pulam este quadro. Os pedidos
      de keyframe ficam pendentes, porque `take_keyframe` só é chamado no submit.
   3. **Fase 1**: `cpu[0]` sozinha.
      - `upload.map(dev)`, depois `convert_batch(&mapeado.image(), &mut [(&scaler, m.nv12())],
        opts)`, depois `commit(ring.next())` e `submit`. O `force` segue a regra de hoje
        (`take_keyframe || primeiro`, e devolve o pedido se a fila estiver cheia).
      - Estatísticas: `convert.push(t0.elapsed())`;
        `lock_wait.push(rt.lock_wait_us + upload.lock_wait_us)`.
   4. **Fase 2**: `cpu[1..]` juntas. `map` de cada upload, um `convert_batch` com todos, depois
      `commit` e `submit` de cada uma. As estatísticas são as mesmas, com `convert` medido no
      `commit` de cada camada.
   5. `drop(mapeado)` (`Unmap`).

   Em todas as fases: `opts = BatchOptions { threads, on_thread_start:
   Some(prioridade_acima_do_normal) }`. Uma falha de `map`, `convert_batch` ou `commit` numa camada:
   `warn!` e só essa camada pula o quadro.

   Empréstimos: junte `Vec<&mut Camada>` das camadas da CPU devidas e desestruture cada
   `Pipe { conv: Conversor::Cpu(CamadaCpu { scaler, upload }), ring, enc, .. }`. O `scaler` é
   imutável, o `upload` é mutável, e cada `MappedUpload` vive até o seu `commit`.
5. Depois de `processar`: se nenhuma camada tem `Conversor::Cpu`, `leitura = None` (libera 8 MB em
   1080p).

**d) Estatísticas.**
- `LayerStats::converter` vem do tipo do pipe (`None` sem pipe).
- `PipelineStats::cpu_readback = leitura_s.stage()`.
- `PipelineStats::cpu_threads`: `threads` se alguma camada usa a CPU; senão 0.
- Para camadas na CPU, `convert` mede do início da leitura ao `commit` (atraso real até o NV12
  ficar pronto, comparável à soma "captura + conversão + codificação" da bancada); `lock_wait` mede
  a leitura somada à escrita da camada; `convert_gpu` fica vazio.

**e) Bancada** (`examples/bancada.rs`):
- `--conversao auto|gpu|cpu` (padrão `auto`) vira `Args.conversao: ConversionSpec`. O texto de uso
  e os erros seguem as outras flags: valor inválido ou sem valor é erro.
- Usa `VideoPipeline::start_with(.., VideoPipelineOptions { conversion })`.
- Cabeçalho: `conversão pedida: Auto|Gpu|Cpu`.
- Tabela por camada: coluna nova `conversor` (`GPU`, `CPU` ou `-`).
- Linhas novas:
  - `leitura para a CPU média/p95: X / Y ms; threads: N`, quando `cpu_threads > 0`;
  - `CPU do processo: Z% de um núcleo`: `GetProcessTimes` no início e no fim do trecho medido,
    (usuário + núcleo) ÷ tempo de parede × 100, com `// SAFETY:`.
- Testes do `parse_args`: `--conversao cpu`, `gpu` e `auto`; valor inválido; sem valor; padrão
  `Auto` no `defaults_sem_argumentos`.

**f) Testes** (Windows). Pulam com mensagem sem GPU, sem monitor 1 ou sem janela animada.
- `win_video::tests` ganha `iniciar_com(ladder, opts, sem_vp: bool)`. Ele exige só
  `GpuDevice::create_default()` (não exige `has_video`), o monitor 1 e a `JanelaAnimada`, e guarda a
  janela junto do pipeline, como `iniciar`.
  1. `cpu_gera_quadros`: `Cpu`, camada 0 1280×720 a 30 fps.
     - ≥ 15 quadros em 2 s; o primeiro é keyframe com SPS/PPS; todos são 1280×720;
     - `stats().layers[0].converter == Some(ConverterKind::Cpu)`;
     - `cpu_readback.p95_us > 0` e `cpu_threads >= 1`.
  2. `cpu_duas_camadas`: `Cpu`, camadas 0 (1280×720 a 30) e 1 (640×360 a 15), as duas pedidas. Em
     3 s: ≥ 30 quadros da 0 e ≥ 15 da 1; as duas com `Some(Cpu)`.
  3. `cpu_keyframe_pedido_chega`: `Cpu`; `request_keyframe(0)` depois de 1 s; um keyframe chega em
     2 s.
  4. `auto_cai_para_cpu_sem_vp`: `start_sem_vp` com `Auto`: ≥ 15 quadros em 2 s, `Some(Cpu)`.
  5. `gpu_sem_vp_nao_cai`: `start_sem_vp` com `Gpu`: nenhum quadro em 2 s, `converter == None`.
  6. `monitor_1_gera_quadros_na_camada_0` (existente) passa a conferir também
     `converter == Some(Gpu)` quando há VP. Fica mais estrito.
- `tests/cpu_basico.rs` (`#![cfg(windows)]`): **o caminho de uma VM, menos a captura**.
  - `GpuDevice::create_basic()` (pula sem ele);
  - `H264Encoder::new(&basico, 640×360, 30 fps, 1000 kbps, gop 300, EncoderChoice::Software)` (pula
    sem MFT de software);
  - 30 quadros BGRA de cor diferente, cada um: `upload` → `ReadbackStaging` → `CpuScaler` →
    `UploadStaging` → textura de um `TextureRing` NV12 do básico → `submit`;
  - ≥ 20 imagens em 5 s, a primeira keyframe com SPS/PPS.
- Testes portáveis de `escolher_conversor` (5.2 b).
- Os testes atuais passam sem mudança de limite.

**g) Portão da entrega de memória ao codificador de software.** Abre se, na medida
`--conversao cpu --software` (seção 9), a codificação p50 da camada 0 ficar ≤ 10 ms: aí ~1 ms de
cópias evitáveis é ≥ 10%.
- Aberto, vira a rodada seguinte:
  - `zap-encode`: `H264Encoder::submit_memory(nv12: Vec<u8>, capture_us: u64, force_keyframe: bool)
    -> bool`, só no software (no hardware devolve `false` e conta descarte); `Job` com a entrada como
    enum; `sw.rs` pula a staging;
  - `zap-media`: anel de `Vec<u8>` devolvidas pelo codificador por canal.
- Fechado: registrar o número na seção 8.

**h) Verificação da frente.** `cargo test -p zap-media` (5 vezes seguidas sem falha; os testes
novos rodam, sem pular, neste PC), clippy e fmt do crate, `cargo check --workspace --all-targets`
(o `zap-core` compila sem mudança).

## 6. Testes de aceite da rodada

| # | Teste | Esperado |
|---|---|---|
| 1 | `cargo test -p zap-gpu` (Windows; no CI também Linux) | Testes portáveis de 5.1 a; Windows de 5.1 f e c; os antigos passam (o do ímpar trocado conforme 5.1 c) |
| 2 | `cargo run --release -p zap-gpu --example cpu_nv12`, máquina ociosa, mediana de 3 execuções | Com `cpu_threads()` = 4: 1080p ≤ 1,5 ms; 720p ≤ 4,5 ms; 360p ≤ 1,5 ms; 720p + 360p ≤ 5,5 ms; as 3 ≤ 7 ms; NV12 720p → 360p ≤ 1 ms. Com 1 thread: registrar (protótipo: 3,1 / 9,0 / 2,4 ms) |
| 3 | CPU × GPU (5.1 f 1–4) | Sólidas: máx ≤ 1. Gradiente, nítido na NVIDIA e NV12: máx ≤ 2 e ≥ 50 dB por plano. Ímpar: máx ≤ 1 |
| 4 | Adaptador básico (5.1 f 8 e `zap-media --test cpu_basico`) | Passam neste PC, sem pular |
| 5 | `cargo test -p zap-media` | 5 vezes seguidas sem falha; os 6 testes de 5.2 f rodam neste PC |
| 6 | Bancada `--conversao cpu` (NVENC), 5 × 10 s | Mediana: camada 0 ≥ 57 fps, 1 ≥ 28, 2 ≥ 14; p95 captura + conversão + codificação da camada 0 ≤ 12 ms; espera de lock p95 ≤ 1 ms (é a soma de todas as entradas no lock da leitura e da escrita da camada, centenas por quadro); leitura p95 ≤ 2 ms; conversor `CPU` nas 3. CPU do processo: registrar, com meta de no máximo 1 núcleo a mais que no item 7 |
| 7 | Bancada padrão (`auto`, que dá GPU aqui), 5 × 10 s | Sem regressão: camada 0 ≥ 57 fps e p95 somado ≤ 8 ms na mediana (rodada 3: 60 fps e 6,5 ms); conversor `GPU` nas 3 |
| 8 | Bancada `--conversao cpu --software`, 3 × 10 s | Meta: camada 0 ≥ 30 fps com as 3 camadas ligadas. Se não bater, registrar a etapa que domina (esperado: a codificação de software; não bloqueia). Decide o portão 5.2 g |
| 9 | Workspace | `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace --no-fail-fast` sem falha; `cargo check --workspace --all-targets` |

Os itens 6 e 8 são metas, como o item 11 da rodada 3. Se não forem atingidas, o fechamento registra
o número, a etapa que domina e a próxima hipótese.

## 7. Riscos

| Risco | Mitigação |
|---|---|
| VP da AMD ou da Intel com outro filtro: CPU e GPU diferem em conteúdo nítido nesses PCs | Limite estrito só na NVIDIA (medido); nos outros fabricantes o teste imprime. Cores sólidas e gradiente valem para qualquer filtro linear. A imagem continua correta, só não idêntica |
| CPU fraca (VM de 2 núcleos): conversão + codificação de software não dão 1080p60 | Lá, `cpu_threads()` = 1 (sobra núcleo para o codificador). A camada descarta (slot "o mais novo vence" e fila de 2 do codificador) em vez de acumular atraso, e o alocador desce quem assiste pela banda. Itens 6 e 8 medem o custo aqui |
| O `yield_now` na espera da leitura gasta CPU (~0,8 ms por quadro) | Só nos 2 primeiros ms; depois `sleep(1 ms)`. A alternativa (`Map` bloqueante) trava captura e MFTs (2.4) |
| Driver sem textura NV12 (não encontrado; o WARP tem) | Erro claro na camada e nova tentativa em 2 s. A entrega de memória (portão 5.2 g) é a saída se aparecer um caso real |
| `Converter` com BGRA ímpar dá imagem errada em outro driver sem dar erro | Teste de cores com entrada ímpar (5.1 c). Erro de `new` ou 3 falhas seguidas de `convert` no `Auto` levam a camada para a CPU |
| Testes do pipeline com CPU lentos em debug ou sob carga | `opt-level = 3` no `zap-gpu` em dev (5.1 d). Limites com folga de 50% (15 quadros em 2 s a 30 fps) |
| Criar threads por quadro (~0,15 ms por escopo) | No máximo 2 escopos por quadro. O `rayon` só entra se o item 6 mostrar a conversão como gargalo |
| Janela redimensionada no meio: a entrada muda de tamanho | O ramo "entrada mudou" recria o conversor (GPU ou CPU) e a `ReadbackStaging` (por `matches`) |
| Câmera NV12 em memória sobe para textura e desce de novo | 720p30, ~0,3 ms por quadro; fica de fora (seção 1) |

## 8. Números (preencher no fechamento)

Bancada release, `--monitor 1 --preset 1080p60 --segundos 10` com animação, máquina ociosa
(2026-11-05). Mediana por coluna entre as execuções; saídas em `bancada_m2b_{auto,cpu,sw}_N.txt` e
`cpu_nv12_N.txt` no scratchpad. **Fechamento com menos execuções que o plano pedia** (pedido do
chamador): `cpu` 3 (não 5), `auto` 1 (não 5), `cpu --software` 1 (não 3). "p95 somado" = captura +
conversão + codificação da camada 0. Conversão = do início da leitura ao `commit` (CPU) ou do
`convert` (GPU).

| Medida | Camada | conversor | fps | kbps | conversão média/p95 (ms) | leitura média/p95 (ms) | espera lock p95 (ms) | codificação média/p95 (ms) | p95 somado (ms) | CPU do processo |
|---|---|---|---|---|---|---|---|---|---|---|
| `auto`, 1 execução | 0 | GPU | 60,1 | 1397 | 0,06 / 0,09 | — | 0,085 | 3,68 / 7,61 | 7,7 | 15% de um núcleo |
| | 1 | GPU | 30,0 | 757 | 0,03 / 0,10 | — | 0,092 | 1,84 / 5,50 | | |
| | 2 | GPU | 15,0 | 226 | 0,00 / 0,01 | — | 0,000 | 1,48 / 1,89 | | |
| `cpu` (NVENC), mediana de 3 | 0 | CPU | 60,0 | 1472 | 5,15 / 9,56 | 3,35 / 8,46 (4 threads) | 0,155 | 2,08 / 2,29 | **11,9** (5,7 / 11,9 / 14,1) | 68% |
| | 1 | CPU | 30,0 | 766 | 7,95 / 12,43 | | 0,165 | 1,14 / 1,56 | | |
| | 2 | CPU | 15,0 | 230 | 4,41 / 13,13 | | 0,185 | 1,47 / 1,62 | | |
| `cpu --software`, 1 execução | 0 | CPU | 60,1 | 1459 | 5,51 / 9,26 | 3,73 / 8,00 | 0,147 | 5,90 / 7,29 | 17,7 | 215% |
| | 1 | CPU | 30,1 | 633 | 9,57 / 13,23 | | 0,161 | 4,09 / 8,82 | | |
| | 2 | CPU | 15,0 | 183 | 10,60 / 13,70 | | 0,597 | 2,91 / 4,01 | | |

Descartes de captura (mediana): `cpu` 15 (27 / 15 / 14), `auto` 14, `cpu --software` 1; descartes de
codificação 0 em todas. Os 3 itens de `cpu` têm a mesma leitura: a conversão da camada 0 fica em
~5 ms de média (3 a 5,5) porque ela espera a leitura, não porque o laço de pixels seja lento.

Exemplo `cpu_nv12` (mediana das medianas de 3 execuções, ms):

| Caso | 1 thread | `cpu_threads()` = 4 | Meta (item 2) |
|---|---|---|---|
| 1080p | 3,12 | 1,36 | <= 1,5 ok |
| 720p | 5,69 | 2,33 | <= 4,5 ok |
| 360p | 1,71 | 0,75 | <= 1,5 ok |
| 720p + 360p (lote) | 7,39 | 2,98 | <= 5,5 ok |
| as 3 (lote) | 10,57 | 4,18 | <= 7 ok |
| NV12 720p -> 360p | 0,66 | 0,37 | <= 1 ok |

Portão 5.2 g: codificação da camada 0 com `--software` = 5,9 ms de média (p95 7,3 ms; p50 não é
impresso pela bancada, mas a média já é menor que 10) = **portão aberto** (execução única). Vira a
rodada seguinte: `H264Encoder::submit_memory` e anel de `Vec<u8>`.

## 9. Fechamento

Na ordem, com a máquina ociosa (`tasklist | findstr /i "cargo rustc"` vazio) antes de cada medida
de desempenho:
1. `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`;
   `cargo check --workspace --all-targets`; `cargo test --workspace --no-fail-fast` (relatar cada
   falha com a causa).
2. Testes de aceite 1 a 5 da seção 6 (o 5: 5 vezes).
3. `cargo build --release -p zap-gpu --example cpu_nv12` e 3 execuções (saídas `cpu_nv12_N.txt` no
   scratchpad).
4. `cargo build --release -p zap-media --example bancada`. Execuções de
   `target/release/examples/bancada.exe --monitor 1 --preset 1080p60 --segundos 10`, com animação
   (padrão): 5 com `--conversao auto`, 5 com `--conversao cpu` e 3 com `--conversao cpu --software`.
   Saídas `bancada_m2b_{auto,cpu,sw}_N.txt` no scratchpad. Preencher a seção 8 e decidir o portão
   5.2 g.
5. Documentos:
   - `docs/ESTADO.md`: na seção 2 do "O que falta", tirar o item "conversão para NV12 pela CPU" (ou
     dizer o que ficou) e mencionar a correção da janela ímpar; uma "Atualização da rodada 4" curta,
     com os números da seção 8; nos "Próximos passos", tirar o item 2 (conversão), manter o CI e
     acrescentar "conversor em uso no Diagnóstico do app";
   - `docs/planos/m2-midia-windows.md`: na frente `gpu`, uma linha sobre o conversor de CPU de
     reserva e o BGRA ímpar; no "Estado do fechamento", os números da rodada 4;
   - este plano: seção 8 preenchida e uma seção 10 "Estado do fechamento".

   O `docs/arquitetura-v3.md` já foi atualizado com este plano (§2, §3, §5 e §12).
6. `git status`: nenhum arquivo temporário em `crates/`.

## 10. Estado do fechamento (revisor, 2026-11-05)

**Verificação.** `cargo fmt --all --check` limpo. `cargo clippy --workspace --all-targets -- -D
warnings`: uma falha de integração, corrigida no fechamento: `crates/zap-gpu/examples/cpu_nv12.rs`
tinha `fn assinatura` sem uso fora de `#[cfg(test)]` (dead code); movida para dentro do `mod tests`
do exemplo. Depois, limpo. `cargo check --workspace --all-targets` limpo. `cargo test --workspace
--no-fail-fast`: **zero falhas** (saída em `ws_test.txt` no scratchpad; zap-gpu 83 lib + 4
`cpu_windows` + 3 `staging_windows` + 2 `basico_windows` + 10 `windows_gpu`; zap-media 54 lib + 1
`cpu_basico` + 10 sintético; zap-core 389). Testes novos do caminho de CPU (portáveis de `cpu::`,
`cpu_windows`, `staging_windows`, `basico_windows`, os 6 do pipeline e `cpu_basico`), 3 rodadas
seguidas: 3 de 3 verdes, nenhum pulou. `online_aceite_2` e `aceite_camadas_local`: 1 execução cada,
passam (16,3 s e 16,6 s). Não rodado no fechamento: `cargo test -p zap-media` 5 vezes (só 3, nos
testes novos).

**Itens de aceite (seção 6).**

| # | Resultado |
|---|---|
| 1 a 5 | Atendidos neste PC (4 sem pular; 5 com 3 rodadas dos testes novos) |
| 2 | Atendido, todos os casos dentro da meta (tabela da seção 8; 720p em 2,33 ms contra 4,5) |
| 6 (`cpu`, 3 execuções) | **Parcial.** fps 60,0 / 30,0 / 15,0 (meta 57 / 28 / 14): ok. Espera de lock p95 0,155 ms (<= 1): ok. Conversor `CPU` nas 3: ok. p95 somado 11,9 ms (<= 12): ok na mediana, mas 1 de 3 execuções ficou em 14,1 ms. **Leitura p95 8,5 ms (meta <= 2): não atingida** (7,3 / 8,5 / 9,1; média 2,0 a 3,5 ms). CPU do processo 68% contra 15% na GPU: +0,53 núcleo (meta <= +1): ok |
| 7 (`auto`, 1 execução) | Sem regressão: 60,1 fps, p95 somado 7,7 ms (<= 8), conversor `GPU` nas 3 |
| 8 (`cpu --software`, 1 execução) | Meta atingida: camada 0 a 60,1 fps (>= 30) com as 3 camadas. Etapa dominante: p95 somado 17,7 ms, com conversão (9,3) e codificação de software (7,3) em partes parecidas. CPU do processo 215% |
| 9 | Atendido |

**A meta do caminho de CPU foi atingida?** Sim no que importa para o usuário: um PC sem Video
Processor (e a janela de tamanho ímpar) agora transmite a 60/30/15 fps sem descartes de codificação,
a conversão é igual à da GPU dentro de 1 nível e a GPU não regrediu. **Não por completo**: a leitura
da origem para a CPU ficou em p95 ~8,5 ms, contra 0,77 ms (p95 1,26) medidos no protótipo
(seção 2.4) com a GPU ociosa. O p95 somado (11,9 ms) só cabe porque a captura é muito leve.
Hipótese para a próxima rodada: aqui o `CopyResource` espera a fila da GPU, que divide com o NVENC, a
animação e a captura, e a espera é o `copy_wait_us` (leitura média 2 a 3,5 ms). Caminhos: ler o
quadro anterior (duas stagings de leitura em rodízio, com a conversão uma cópia atrás, +1 quadro de
atraso), ou medir quanto do `copy_wait_us` é fila da GPU e quanto é o `yield_now`.

**Pendente.** Entrega de memória ao codificador de software (portão aberto, seção 8); levar
`LayerStats::converter` ao Diagnóstico do app; CI Windows com a cadeia de dispositivos; repetir as
medidas com 5/5/3 execuções se for preciso fechar os números do plano. Nada foi commitado.

## 11. Conferência do orquestrador (2026-11-05)

**Verificação refeita.** fmt, clippy (`-D warnings`), `cargo check --workspace --all-targets` e
`cargo test --workspace --no-fail-fast` (67 alvos, zero falhas); testes novos da CPU 3 vezes
(`cpu::`, `cpu_windows`, `staging_windows`, `basico_windows`, `windows_gpu`, `win_video`,
`cpu_basico`): verdes; `online_aceite_2` e `aceite_camadas_local` passam.

**Achado: os testes do pipeline pulavam em silêncio.** Rodando em paralelo (o padrão do
`cargo test`), só 1 dos 10 testes de `win_video::tests` (os 5 antigos e os 5 novos) rodava: o
Windows só deixa uma duplicação do mesmo monitor por processo, e o segundo `DuplicateOutput` dá
`E_INVALIDARG`, que o teste tratava como "PULANDO" (e passava). Correção: trava `MONITOR_1` nos
testes, solta só depois do pipeline (o `Drop` dele para a captura). Depois: 3 de 3 rodadas com os
10 rodando, nenhum "PULANDO" (~30 s por rodada).

**Limpeza.** Saíram os `#[allow(dead_code)]` do esqueleto ("usado na tarefa X"); a bancada
imprime a meta de 12 ms quando a camada 0 está na CPU (antes dizia 8 em qualquer modo).

**Bancada (2 execuções `cpu`, 1 `auto`, máquina ociosa).** `cpu`: 60/30/15 fps nas duas; p95 somado
26,1 ms (logo depois da compilação, com a captura também lenta, p95 4,55 ms) e 10,6 ms; leitura
p95 12,2 e 6,6 ms. `auto`: 60/30/15 fps, p95 somado 4,4 ms. Juntando as 5 execuções `cpu` (seção 8
e estas): **mediana do p95 somado 11,9 ms e da leitura p95 8,5 ms**, com muita variação entre
execuções. `cpu_nv12` (1 execução, 4 threads): 1,37 / 2,11 / 0,80 / 2,95 / 4,17 / 0,43 ms, igual à
seção 8.

**Hipótese da leitura, refinada.** O `cpu --software` (sem NVENC) também deu leitura p95 8,0 ms,
então o NVENC não é a causa principal. O mais provável é que o `CopyResource` espere a GPU terminar
de produzir o quadro capturado (composição do DWM e cópia da captura). O caminho da GPU também tem
essa espera, mas ela fica escondida na fila da GPU. O protótipo (0,77 ms, seção 2.4) lia uma
textura parada, então a meta de 2 ms provavelmente não vale para a captura real. Próximo passo:
medir com uma query de evento logo antes do `CopyResource`. Ler o quadro anterior (duas stagings
em rodízio) esconderia a espera, mas somaria um quadro inteiro de atraso, o que é pior no total.

## 12. Investigação da leitura (orquestrador, 2026-11-05)

Pergunta: por que a leitura da origem para a CPU tinha p95 de ~8,5 ms, se o protótipo (seção 2.4)
media 0,77 ms (p95 1,26)? Saídas brutas na pasta `r4b/` do scratchpad.

1. **Estado de energia da GPU** (`nvidia-smi` a cada 100 ms durante a bancada). O driver começa em
   P0 (1506 MHz no núcleo, 4608 MHz na memória) e desce para P2, P5 e P8 (607 / 405 MHz) ao longo
   dos 10 s. Isso acontece tanto em `cpu` quanto em `auto`. Com janela curta (3 s, só em P0), a
   leitura média caiu de 6,5 para 2,5 ms, mas o p95 continuou em 7 a 9 ms. O clock explica parte
   da média, não a cauda.
2. **Leitura em partes** (diagnóstico temporário: consulta de evento + `Flush` antes do
   `CopyResource` e outra depois). Espera pelo trabalho anterior da GPU: média 2,04 / p50 0,61 /
   p95 6,80 ms. A cópia: 0,97 / 0,64 / 1,78 ms. Do fim da cópia ao `Map`: ~0,003 ms, então a
   sondagem não pesa.
3. **De onde vem a espera** (diagnóstico temporário na captura DXGI: esperar a cópia da imagem do
   DWM terminar antes de entregar o quadro). A espera na captura deu média 1,9 / p95 5,7 ms, e a
   espera na leitura caiu para média 0,32 / p95 0,82 ms (leitura total p95 2,08 ms). **A espera é
   a GPU terminando o quadro capturado** (composição do DWM e cópias da captura), não o NVENC nem
   a nossa cópia. O caminho da GPU tem a mesma espera, mas dentro da fila da GPU: ela aparece na
   "codificação" (em `auto`, média de 2 a 9 ms e p95 de 4 a 14 ms entre execuções; em `cpu`, a
   codificação fica estável em ~2 ms, porque a entrada já chega pronta).

**Mudança.** A espera passa a ser medida separada, de forma permanente e barata: a
`ReadbackStaging` cria uma consulta de evento (uma por staging), põe `End` + `Flush` antes da cópia,
espera o evento com a mesma regra do `Map` (`yield_now` nos primeiros 2 ms, `sleep(1 ms)` depois) e
devolve `ReadbackTiming::frame_wait_us`. O `Map` volta a girar rápido para pegar o fim da cópia.
`PipelineStats::cpu_frame_wait` leva a medida, e a bancada imprime "espera pelo quadro".
Os diagnósticos temporários foram removidos.

**Meta revista (item 6).** "Leitura p95 ≤ 2 ms" passa a valer para a **cópia** (leitura menos
espera pelo quadro): cumprida (p50 0,64 / p95 1,78 ms). O p95 somado do caminho de CPU deve ser
comparado com o da GPU nas mesmas condições, porque a espera pelo quadro varia com o conteúdo da
tela e com o estado de energia da GPU.

**Medida final** (bancada release, 1080p60, monitor 1; a tela estava mais pesada que na seção 8:
4,4 a 4,8 Mbps na camada 0, contra 1,4):

| Modo | Execuções | p95 somado camada 0 (ms) | Leitura média / p95 (ms) | Espera pelo quadro média / p95 (ms) | Codificação camada 0 média / p95 (ms) | CPU do processo |
|---|---|---|---|---|---|---|
| `auto` (GPU) | 3 | 14,0 / 13,7 / 12,3 | — | — (dentro da codificação) | 8,77 / 13,61; 5,52 / 13,51; 6,70 / 12,14 | 22% |
| `cpu` | 3 | 23,3 / 21,5 / 20,6 | 4,65 / 10,66; 5,27 / 10,74; 5,80 / 10,85 | 3,48 / 9,99; 3,97 / 10,56; 4,64 / 9,46 | 5,14 / 6,98; 4,52 / 6,35; 4,34 / 5,08 | 94 a 108% |

60/30/15 fps em todas. Nas mesmas condições, o caminho de CPU acrescenta ~8 ms ao p95 somado
(mediana 21,5 contra 13,7): a cópia (~1,2 ms de média), a conversão e a subida para o NV12 da
camada. A espera pelo quadro é igual nos dois caminhos e não depende do conversor.

**O que fica.** Baixar a espera pelo quadro não é trabalho do conversor. Ela depende do DWM e do
estado de energia da GPU, que o app não controla sem a NVAPI. Se um dia valer a pena, o caminho é
na captura (por exemplo, uma cópia a menos, de `limpa` para o slot do anel, quando o cursor não
precisa ser desenhado).
