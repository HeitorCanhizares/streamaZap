# M4b + M2: pendências da rodada 1 do M4 e da primeira rodada numa GPU real

Plano do arquiteto. Continua `docs/planos/m4-online.md` (rodada 1: núcleo Online, Tauri e telas;
aceites 1, 3, prévia e reconexão passam) e `docs/planos/m2-midia-windows.md` (mídia no Windows,
que até agora só tinha rodado em CI sem GPU de vídeo). Estado de partida: branch `m4-online`,
commit `1d043aa`. PC de referência: Windows 10, MSVC, GTX 1060 (NVENC).

Contrato já atualizado junto com este plano (as frentes implementam, não reescrevem):
`docs/api/balanceamento.md` §4 e §6 (sondagem), `docs/api/midia.md` (sonda), `docs/arquitetura-v3.md`
§2 (timers e COM no Windows), §6.1 item 5 e §7.2, `docs/planos/m4-online.md` (rota direta / `use_relay`).

## 0. Resumo

Três objetivos e o que os diagnósticos de hoje (seção 2) acrescentaram:

1. **Sondagem de verdade** (fecha o aceite 2, `core-online-22`): estimador aceita o resultado de um
   grupo de sondas, a ponte do servidor de teste e o publicador (Local/direto) sondam.
2. **Modo só direta** (fecha o cenário C do aceite 4, `core-online-24`, e a `core-online-27`): sem
   ponte o `watch` fica pendente e vira assinatura quando a rota direta conecta. `Drop` do `Core`
   seguro dentro de um runtime.
3. **M2 na GPU real**: (a) o `STATUS_ACCESS_VIOLATION` do `zap-capture --lib` é o MTA do COM
   morrendo com uma fábrica WinRT em uso; (b) `tamanho_medio_nao_chave` mede conteúdo sem
   informação (cor sólida); a bancada **também** cai com violação de acesso em release (ponteiro do
   `GpuDevice` pendurado) e mostra que os timers do Windows (15,6 ms) estragam fps, latência e o
   teste `rede_ruim_3mbps_fonte_6mbps`.

## 1. Escopo

Entra:
- `zap-rate`: medidor de grupo de sondas, planejador de rajadas, ritmo por crédito; `Bwe` com
  resultado de sonda e carência.
- `zap-net`: relatório alimenta o medidor; `Prober` (envio das rajadas); `MediaSender::sent_bytes`.
- `zap-test-server`: a ponte sonda cada espectador abaixo do teto.
- `zap-core`: publicador sonda (Local/direto); modo só direta; `Drop` seguro; resolução de timer
  enquanto há sala; limpeza dos testes de aceite (prints DIAG, junção do cenário C); fechamento.
- `zap-sys`: `platform` (timer de 1 ms, MTA preso).
- `zap-gpu`: MTA preso no `ComGuard`; `GpuDevice: Clone`.
- `zap-capture`: captura com dono do dispositivo (sem ponteiro cru); MTA preso nas entradas WinRT.
- `zap-encode`: conteúdo de teste com informação; laço de eventos do MFT de hardware sem espera
  por timer.
- `zap-media`: bancada com timer de 1 ms; números registrados aqui e no plano do M2.

Fica de fora:
- Servidor de produção (repositório privado): segue `docs/api/balanceamento.md` §4. Até ele sondar,
  quem assiste pela ponte de produção continua preso na camada 2.
- Prévia sem ponte (`rest.md`: não há prévia sem ponte), troca de `use_relay` com a sala aberta
  (vale na próxima entrada), interface nova para tile "aguardando conexão direta".
- Contenção do contexto D3D11 entre conversor e MFT (se a bancada mostrar que é o gargalo depois
  das correções, vira pendência registrada na seção 8, não tarefa desta rodada).
- Conversão NV12 pela CPU e a cadeia de dispositivos no CI (continuam no `ESTADO.md`).

Regras que valem para todas as frentes: nada de mudar a lógica dos testes de aceite (corrige-se o
produto); `CoreOptions::default()` continua com `default_server: None` e os testes só falam com o
`zap-test-server`; sem commit, push ou troca de branch; todo comando de cargo começa com
`export PATH="$HOME/.cargo/bin:/c/Program Files/CMake/bin:$PATH";` e o `--target
x86_64-pc-windows-gnu` vira `cargo check --workspace --all-targets`.

## 2. Diagnósticos (medidos em 2026-11-04 neste PC)

### 2.1 Aceite 2: ninguém sai da camada 2

`crates/zap-rate/src/bwe.rs`, estado `Normal`: o alvo só cresce se `vazao >= FRACAO_APP_LIMITED
(0,5) × alvo`. Recebendo a camada 2 (400 kbps, keyframe 8× a cada 10 s), a estimativa para em
~2× o pico da janela de 500 ms (~1450 kbps). A camada 1 pede `banda ≥ 1,15 × 2500 / 0,9 = 3195`;
a 0, `≥ 10223`. Ninguém manda sondas: `grep send_probe` só acha os testes do `zap-net`; o
`zap-test-server/tests/bridge.rs` passa porque monta `banda_kbps` à mão. O mesmo vale para o modo
Local (o publicador também não sonda): lá também ninguém passa da camada 2 com tela parada; os
aceites Local atuais não conferem isso.

### 2.2 `zap-net --test impaired rede_ruim_3mbps_fonte_6mbps`

Saída do teste: `enviados=647 descartados_pub=221 recebidos=434 perdidos_rx=213 atraso_max_ms=157
banda_media_kbps=2102 (n=99)`, proxy `forwarded_bytes=5687113, dropped_packets=9`.
- 647 quadros em 20 s com `interval(16,667 ms)` = **31 ms por tique**: os timers do tokio estão em
  passos de 15,6 ms (resolução padrão do Windows; 2 passos = 31,25 ms → 640 quadros). A fonte que
  devia ter 6 Mbps teve 3,2 Mbps.
- O proxy só descartou 9 pacotes e passou ~2,2 Mbps: o enlace de 3 Mbps **não** encheu. Quem jogou
  fora 34% dos quadros foi o emissor (prazo/admissão), com ACKs e RTT inflados pelos mesmos passos
  de 15,6 ms (ACK atrasado do quinn, soltura do proxy).
- O estimador acertou: com ≥ 3 descartes/s vai para 0,95 × o que chega (0,95 × 2,2 ≈ 2,1 Mbps).

Conclusão: **não é o estimador**. É a resolução de timer do processo. No Linux os timers têm ~1 ms
e o teste passava. O app sofre o mesmo (ver 2.4), então a correção é de produto (pedir 1 ms
enquanto há sala) e o teste passa a rodar nas mesmas condições do app.

### 2.3 `zap-capture --lib`: violação de acesso só com os testes juntos

- Biblioteca inteira: 15 de 25 execuções morrem (código 139 = `STATUS_ACCESS_VIOLATION`).
- Bisseção por filtro (12–15 execuções cada): cada grupo sozinho, 0 falhas; `camera wgc`, 9/12;
  `list_cameras wgc_supported camera_mode`, 13/15; tirando `wgc_supported`, 0/15; tirando
  `list_cameras`, 0/15. (Os testes puros só mudam o momento em que as threads começam.)
- Mecanismo: `list_cameras` faz `CoInitializeEx(MTA)` + `MFStartup` e depois `MFShutdown` +
  `CoUninitialize` (`camera_enum::MfGuard`). `wgc_supported` chama
  `GraphicsCaptureSession::IsSupported()` numa thread **sem** COM: o `windows-rs` só faz
  `CoIncrementMTAUsage` se `RoGetActivationFactory` devolver `CO_E_NOTINITIALIZED`
  (`windows-core-0.62.2/src/imp/factory_cache.rs`). Com o MTA vivo por causa da outra thread, a
  fábrica é obtida e **guardada em cache estático**; quando a thread do `MfGuard` desinicializa, o
  MTA morre e a fábrica (e a DLL) somem debaixo de quem está usando.
- É bug de produto: o app lista câmeras, janelas e apps de som e abre captura de tela e de janela ao
  mesmo tempo, em threads do tokio sem COM. Correção: prender o MTA pelo processo inteiro.

### 2.4 Bancada (`cargo run -p zap-media --example bancada -- --monitor 1 --preset 1080p60`)

Codificador escolhido: `NVIDIA H.264 Encoder MFT`. Máquina possivelmente carregada por builds de
outros agentes (números de tempo pouco confiáveis; o travamento não depende disso).

| Perfil | Camada | fps | kbps | conversão média/p95 (ms) | codificação média/p95 (ms) |
|---|---|---|---|---|---|
| debug, 10 s | 0 (1920×1080) | 32,8 | 2007 | 9,72 / 29,84 | 17,70 / 41,50 |
| | 1 (1280×720) | 26,2 | 1624 | 1,04 / 5,66 | 17,10 / 37,22 |
| | 2 (640×360) | 14,1 | 369 | 1,64 / 11,83 | 15,53 / 35,82 |
| release, 5 s (1ª) | 0 | 17,6 | 2477 | 10,38 / 36,08 | 31,23 / 61,52 |
| | 1 | 18,8 | 1744 | 2,31 / 17,51 | 27,86 / 59,21 |
| | 2 | 13,4 | 385 | 3,49 / 27,69 | 26,34 / 61,73 |

Debug: captura 0,45/0,11 ms, descartados captura 90 / codificação 150, p95 somado da camada 0
71,4 ms (meta ≤ 8). Release: p95 somado 97,6 ms; outra execução de 5 s deu 6 fps e 207 ms.
**Duas de quatro execuções em release morreram com `0xc0000005`** (uma de 10 s, outra de 5 s).

- **Travamento**: `zap-media/src/win_video.rs::spawn` cria `dev`, chama
  `zap_capture::capture_monitor(&dev, ..)` e depois **move** `dev` para a tupla e para a thread
  `zap-video`. As três capturas (`dxgi.rs:49`, `wgc.rs:71/181`, `camera.rs:213`) guardam
  `dev as *const GpuDevice`: o ponteiro aponta para a pilha de `spawn`, que já acabou. Em debug a
  pilha às vezes não é reaproveitada a tempo; em release é. Afeta monitor, janela e câmera no app.
- **Latência de codificação ≈ um passo de timer**: o laço do MFT de hardware (`hw.rs::run`) consulta
  `GetEvent(NO_WAIT)` e espera o próximo quadro com `recv_timeout(até 5 ms)`; um `HaveOutput` que
  chega durante essa espera só é visto quando ela acaba, e no Windows essa espera tem 15,6 ms.
  Média de 15,5–17,7 ms em debug com NVENC (que codifica 1080p em poucos ms).
- **fps abaixo do pedido** (32,8 na camada 0, que pede 60): as esperas por `Condvar`/canal com
  prazo da thread de vídeo têm o mesmo passo de 15,6 ms.
- kbps abaixo do nominal com tela parada é esperado (CBR não enche quadro sem informação).

### 2.5 `zap-encode --test windows tamanho_medio_nao_chave`

`média 270,76 fora de ±50% de 16666`, com `EncoderChoice::Auto` = NVENC neste PC (a nota do M4
dizia "codificador de software": errado). O quadro de teste (`tests/windows.rs::frame`) é uma
**cor sólida** que muda a cada quadro: o resíduo de um quadro P é um DC por macrobloco, ~270 B.
Nenhum controle de taxa gasta 16 KB nisso, e não deve (quem precisa de banda medida tem as sondas).
O erro é a expectativa sobre aquele conteúdo; falta provar a taxa em conteúdo com informação
(seção 5.6). Os outros 5 testes do arquivo passam.

### 2.6 Modo só direta (`use_relay = false`)

(i) `subscribe/pool.rs::ConnPool::get_or_connect` → `online_conn` devolve `Err("ponte
indisponível")`; `Subscription::start` falha; `engine_watch::watch` faz `push_error` + `remove_tile`
e o `assinar` nem vai pelo WebSocket (então o servidor nunca manda `p2p.candidatos`).
(ii) `online/direct.rs::on_candidates` (`Client`) exige a trilha em `shared.subs`.
(iii) `tentar_direta` exige `room.bridge` (usa `bridge.router` e `bridge.direct`); a queda da direta
reassina por `bridge.subscribe`.

### 2.7 `Drop` do `Core`

`engine.rs`: `Drop` chama `shutdown`, que faz `runtime.block_on`. Num teste que entra em pânico
antes do `desligar`, o `Core` cai dentro de uma tarefa do tokio: "Cannot start a runtime from within
a runtime" durante o desenrolar = aborto (`STATUS_STACK_BUFFER_OVERRUN`) que derruba o binário
inteiro de testes.

## 3. Decisões

| Decisão | Motivo |
|---|---|
| Sondas pela ponte (decisão do dono) e pelo publicador no Local/direto | Único jeito de medir banda acima de ~2× a vazão de uma camada barata (2.1) |
| Lógica da sondagem pura em `zap-rate::probe` (planejador, ritmo, medidor); I/O em `zap-net::probe` | Testável com tempo virtual; o servidor de teste e o `zap-core` usam o mesmo código, e o de produção tem uma referência |
| Alvo da rajada `1,5 × (kbps(c) + reserva)`, app adota `0,95 ×` a taxa do grupo | Contas em `balanceamento.md` §4 (precisa de ≥ 1,35×; sobra ~10%) |
| Carência de 300 ms depois da última sonda para sobrecarga e descarte | A fila que a rajada cria não pode derrubar quem está estável (o espectador de 4 Mbps sondado para a camada 0) |
| Intervalo entre rajadas 5 s, dobra na falha até 60 s | Limita o custo para quem não aguenta a camada seguinte |
| `zap-sys::platform`: `timeBeginPeriod(1)` enquanto há sala ativa | 2.2 e 2.4. Custo: mais despertares do sistema; por isso só com sala ativa, não o tempo todo |
| MTA preso (`CoIncrementMTAUsage` uma vez, cookie nunca solto) em `zap-sys::platform` (app) e em `zap_gpu::com` (bibliotecas e seus testes) | 2.3. Duas cópias de ~10 linhas porque `zap-gpu` não pode depender de `zap-sys` (camadas) |
| `GpuDevice: Clone` e cada captura com o próprio clone | 2.4. Clonar = `AddRef` dos COM; tira três `unsafe impl Send` de ponteiro cru |
| `zap-sys` como dev-dependency de `zap-net`, `zap-media` e dependência normal de `zap-test-server` | Os testes e a bancada precisam medir nas condições do app (timer de 1 ms). Não cria ciclo: `zap-sys` não depende de crate interno |
| Thread de eventos por codificador de hardware (`GetEvent` bloqueante) | Latência de saída sem depender do passo do timer; até 5 codificadores = até 5 threads a mais, ociosas bloqueadas no MFT |
| Modo só direta: `watch` pendente guardado na `OnlineRoom`, assinatura nasce na conexão direta | Interface (`ConfigConexao.tsx`) e `rest.md` (`ponte: false`) já prometem; sem `PubConn` falsa da ponte |

## 4. Frentes

| Id | Crate | Depende de | Resumo |
|---|---|---|---|
| `rate` | `zap-rate` | — | `probe.rs` (ProbeMeter, ProbePlanner, ProbePacer, ProbeNeed); `Bwe::on_probe_packet`/`on_probe_cluster` e carência; constantes do alocador públicas |
| `sys` | `zap-sys` | — | `platform`: `TimerResolution` (1 ms), `pin_com_mta`, `init` |
| `gpu` | `zap-gpu` | — | `com::pin_mta` chamado pelo `ComGuard`; `GpuDevice: Clone` |
| `net` | `zap-net` | `rate`, `sys` | `ReportBuilder` com `ProbeMeter`; `probe::Prober`; `MediaSender::sent_bytes`/`probe_size`; testes com timer de 1 ms; `impaired` passando |
| `captura` | `zap-capture` | `gpu` | DXGI/WGC/câmera com clone do dispositivo; MTA preso nas entradas WinRT/COM; testes de stress |
| `encode` | `zap-encode` | — | Conteúdo de teste com informação; laço de eventos sem timer; taxa provada |
| `servidor-teste` | `zap-test-server` | `net`, `rate`, `sys` | Ponte sonda cada espectador (`probe.rs`, `ticker.rs`); `Handle` com timer de 1 ms |
| `media` | `zap-media` | `captura`, `encode`, `sys` | Bancada com timer de 1 ms; stress dos testes Windows; números nas seções 8 e no M2 |
| `core` | `zap-core` | `rate`, `sys`, `net`, `servidor-teste`, `media` | Timer com sala; `Drop` seguro; só direta; publicador sonda; testes; fechamento do M4 |

Caminho crítico do aceite 2: `rate` → `net` → `servidor-teste` → `core`. As frentes `sys`, `gpu` e
`encode` começam já; `captura` anda em paralelo com `net`. Dentro de `core`, o `Drop` seguro e o
modo só direta (5.9 b, c) não dependem da sondagem e podem começar logo; a sondagem do publicador
(5.9 d) espera `net`, e o aceite 2 espera `servidor-teste`.

## 5. Frentes em detalhe

### 5.1 `rate` (`crates/zap-rate`)

Sem tokio, sem I/O; tempo em `Us` como parâmetro.

`allocator.rs` (sem mudar comportamento; `fill.rs` e `hysteresis.rs` passam a usar as constantes):
```rust
/// disponível = banda × FOLGA_BANDA − reserva (balanceamento.md §3).
pub const FOLGA_BANDA: f64 = 0.9;
/// A camada c sobe com disponível ≥ MARGEM_SUBIDA × kbps(c).
pub const MARGEM_SUBIDA: f64 = 1.15;
```

`probe.rs` (novo, `pub mod probe;` em `lib.rs`):
```rust
use crate::Us;

pub const DURACAO_RAJADA_US: Us = 200_000;
/// Espera depois que a necessidade aparece (ou muda) antes da primeira rajada.
pub const PRIMEIRA_SONDA_US: Us = 1_000_000;
pub const INTERVALO_MIN_US: Us = 5_000_000;
pub const INTERVALO_MAX_US: Us = 60_000_000;
/// Prazo para o relatório mostrar o resultado depois do fim da rajada.
pub const JANELA_RESULTADO_US: Us = 1_000_000;
pub const FATOR_ALVO: f64 = 1.5;
pub const SONDA_MIN_KBPS: u32 = 100;
pub const TAMANHO_SONDA: usize = 1200;
/// Janela da taxa enviada (`enviado(e)`); o planejador só sonda com ≥ 500 ms de amostras.
pub const JANELA_ENVIADO_US: Us = 1_000_000;
/// Medidor: o grupo fecha 50 ms depois da última sonda; vale com ≥ 5 sondas e ≥ 100 ms.
pub const FIM_GRUPO_US: Us = 50_000;
pub const MIN_SONDAS: u32 = 5;
pub const MIN_DURACAO_US: Us = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeNeed {
    /// `kbps` nominal da camada que se quer alcançar (`c` em balanceamento.md §4).
    pub next_kbps: u32,
    /// O mesmo `reserved_kbps` que o alocador usa para esse espectador (hoje 0 no app e no servidor de teste).
    pub reserve_kbps: u32,
}
impl ProbeNeed {
    /// `ceil((MARGEM_SUBIDA × next + reserva) / FOLGA_BANDA)`; 2500/0 → 3195; 8000/0 → 10223.
    pub fn required_kbps(&self) -> u32;
    /// `ceil(FATOR_ALVO × (next + reserva))`; 2500/0 → 3750; 8000/0 → 12000.
    pub fn burst_total_kbps(&self) -> u32;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeBurst {
    pub cluster: u16,
    /// Taxa só das sondas (o vídeo já enviado não entra).
    pub kbps: u32,
    pub duration_us: Us,
}

/// Decide quando e quanto sondar um espectador (balanceamento.md §4, "Quando", "Quanto",
/// "Resultado"). Um por conexão.
#[derive(Debug, Clone, Default)]
pub struct ProbePlanner { /* need, elegivel_em, intervalo, ultimo_fim, em_rajada_ate,
                             avaliando: Option<(fim, required)>, cluster, enviados: VecDeque<(Us, u64)> */ }
impl ProbePlanner {
    pub fn new() -> Self;
    /// Chamar a cada ~100 ms. `need`: `None` = nada a testar. `bandwidth_kbps`: último
    /// `relatorio.banda_kbps` (0 = ainda sem relatório). `sent_bytes_total`: contador acumulado de
    /// bytes enviados a esse espectador (o planejador mede a taxa do último 1 s).
    pub fn poll(&mut self, now: Us, need: Option<ProbeNeed>, bandwidth_kbps: u32, sent_bytes_total: u64) -> Option<ProbeBurst>;
    pub fn interval_us(&self) -> Us;
}

/// Ritmo por crédito de uma rajada: independe do espaçamento real dos tiques.
#[derive(Debug, Clone)]
pub struct ProbePacer { /* inicio, fim, bytes_por_us, tamanho, enviados */ }
impl ProbePacer {
    pub fn new(start: Us, burst: &ProbeBurst, packet_bytes: usize) -> Self;
    /// Datagramas a mandar agora: `⌊kbps × (min(now, fim) − início) / (8 × tamanho)⌋ − já enviados`.
    pub fn due(&mut self, now: Us) -> u32;
    pub fn finished(&self, now: Us) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProbeResult { pub cluster: u16, pub kbps: u32, pub probes: u32, pub span_us: Us }

/// Mede a taxa de chegada de cada grupo de sondas (lado de quem recebe).
#[derive(Debug, Clone, Default)]
pub struct ProbeMeter { /* grupo aberto: cluster, primeira, ultima, n, bytes; bytes_pendentes */ }
impl ProbeMeter {
    pub fn new() -> Self;
    /// Sonda chegou. `cluster` diferente do grupo aberto fecha o anterior e devolve o resultado dele.
    pub fn on_probe(&mut self, cluster: u16, arrival_us: Us, bytes: u32) -> Option<ProbeResult>;
    /// Vídeo/áudio chegou: conta no grupo aberto (só entra no total quando chega uma sonda depois dele).
    pub fn on_media(&mut self, arrival_us: Us, bytes: u32);
    /// Fecha o grupo se a última sonda foi há ≥ `FIM_GRUPO_US`.
    pub fn poll(&mut self, now: Us) -> Option<ProbeResult>;
}
```

Regras do `ProbePlanner::poll` (nesta ordem):
1. Guarda `(now, sent_bytes_total)`; descarta amostras com mais de 1 s (fica uma mais velha para a
   diferença). `enviado_kbps = Δbytes × 8000 / Δt_us`.
2. `need` diferente da última vista (inclusive `None ↔ Some`): `elegivel_em = now + 1 s`,
   `intervalo = 5 s`, esquece `avaliando`.
3. Rajada em andamento (`now < em_rajada_ate`): `None`. Acabou: `ultimo_fim = em_rajada_ate`,
   `avaliando = Some((fim, need.required_kbps()))`.
4. Avaliando: `bandwidth_kbps ≥ required` → sucesso, `intervalo = 5 s`; `now ≥ fim + 1 s` → falha,
   `intervalo = min(2 × intervalo, 60 s)`; senão `None`.
5. Sonda se: `need` é `Some`, `bandwidth_kbps > 0`, `now ≥ elegivel_em`,
   `now ≥ ultimo_fim + intervalo`, janela de envio com ≥ 500 ms. `sonda = burst_total − enviado`;
   `sonda < 100` → não sonda e faz `ultimo_fim = now` (tenta de novo depois do intervalo).
6. Sondando: `cluster = cluster.wrapping_add(1)` (o primeiro é 1), `em_rajada_ate = now + 200 ms`,
   devolve `ProbeBurst { cluster, kbps: sonda, duration_us: 200_000 }`.

`ProbeMeter`: taxa = `bytes × 8000 / (ultima − primeira)` kbps, em que `bytes` soma as sondas depois
da primeira e o vídeo/áudio que chegou entre a primeira e a última sonda (o que chega depois da
última não conta). Grupo com < 5 sondas ou < 100 ms: descartado (`None`).

`bwe.rs`:
```rust
/// Carência depois da última sonda: sobrecarga e descarte do emissor não reduzem nem aparecem.
const CARENCIA_SONDA_US: Us = 300_000;
/// Fração da taxa medida no grupo que vira estimativa.
const FATOR_SONDA: f64 = 0.95;

impl Bwe {
    /// Sonda chegou: igual a `on_packet` (detector e vazão) e marca a janela de sondagem.
    pub fn on_probe_packet(&mut self, send_us: Us, arrival_us: Us, bytes: u32);
    /// Fim de um grupo (`ProbeMeter`). Com perda ≤ PERDA_ALTA: `alvo = max(alvo, 0,95 × kbps)`,
    /// limitado a `max_kbps`; `estimate_kbps()` já devolve o novo valor.
    pub fn on_probe_cluster(&mut self, now: Us, kbps: u32);
}
```
- `update(now)` com `now − ultima_sonda ≤ 300 ms`: o ramo de sobrecarga e o de descarte não mexem
  no alvo (nem em `vazao_na_sobrecarga`/`fase_inicial`); `on_frame_dropped(t)` nessa janela é
  ignorado; `state()` devolve `Normal` no lugar de `Overuse` (o relatório e o envio imediato de
  `sobrecarga` usam `state()`).
- O resto do `Bwe` não muda: `nao_sobe_app_limited` e os demais testes continuam valendo.

Testes (todos em `zap-rate`, unitários ou em `tests/`):
- `probe_need_numeros`: 2500 → 3195 e 3750; 8000 → 10223 e 12000.
- `planner_primeira_rajada_1s`: necessidade e banda desde 0 → rajada em 1,0 s (±100 ms), `kbps =
  3750 − enviado`.
- `planner_intervalo_dobra_na_falha`: banda nunca alcança → rajadas em ~1,0, 11,2, 31,4 e 71,6 s
  (intervalo 10, 20, 40 s depois de cada falha) e daí a cada 60 s (±100 ms, passo de 100 ms).
- `planner_sucesso_mantem_5s`: sucesso na rajada de 1,0 s e a mesma necessidade continua → próxima
  em ~6,2 s.
- `planner_sucesso_volta_a_5s`; `planner_need_muda_reinicia`; `planner_sem_banda_ou_sem_need_nao_sonda`;
  `planner_sonda_pequena_nao_sai` (enviado já ≥ alvo − 100).
- `pacer_total_independe_do_tique`: 9500 kbps × 200 ms com tiques de 1 ms e de 15,625 ms → 198 ou
  197 datagramas de 1200 B; 0 depois do fim.
- `meter_mede_rajada`: sondas de 1200 B a 3,35 Mbps + vídeo 400 kbps por 200 ms → 3750 kbps ± 5%.
- `meter_chegada_em_degraus_de_15ms`: mesmas chegadas arredondadas para múltiplos de 15,625 ms →
  ± 10%.
- `meter_descarta_grupo_curto` (4 sondas; 80 ms); `meter_fecha_por_cluster_novo`; `meter_fecha_por_silencio`.
- `bwe_sonda_sobe_para_camada_1`: vídeo 400 kbps por 5 s (estimativa ≤ 1600), grupo de 200 ms a
  3,5 Mbps no total → `estimate_kbps() ≥ 3195` até 150 ms depois; continua ≥ 3195 por mais 3 s só
  com os 400 kbps.
- `bwe_sonda_acima_da_capacidade_nao_derruba` (com `sim::Link` de 4 Mbps e fila de 100 ms): vídeo
  2500 kbps estável (estimativa E0), rajada que faz chegar 12 Mbps: `state()` nunca `Overuse` da
  primeira sonda até 300 ms depois da última; 1 s depois, `estimate ≥ min(E0, 2639)` (limiar de
  descida da camada 1 = 0,95 × 2500 / 0,9) e `≤ max(E0, 0,95 × 4400)`.
- `bwe_sonda_com_perda_alta_ignorada`: perda 0,3 → `on_probe_cluster` não sobe.

Verificação: `cargo test -p zap-rate && cargo clippy -p zap-rate --all-targets -- -D warnings &&
cargo fmt -p zap-rate --check`.

### 5.2 `sys` (`crates/zap-sys`)

`src/platform.rs` (novo; Windows com `windows` feature `Win32_Media` no `Cargo.toml` do crate):
```rust
/// Resolução de timer de 1 ms enquanto viver (Windows: `timeBeginPeriod(1)` / `timeEndPeriod(1)`,
/// contados pelo sistema; vários guardas no mesmo processo podem coexistir). Outros sistemas: nada.
pub struct TimerResolution { active: bool }
impl TimerResolution {
    pub fn high() -> Self;
    pub fn is_active(&self) -> bool;
}
impl Drop for TimerResolution { fn drop(&mut self); }

/// Mantém o MTA do COM vivo até o fim do processo (`CoIncrementMTAUsage` uma vez, `OnceLock`;
/// o cookie nunca é solto). Idempotente e barato depois da primeira vez. Outros sistemas: nada.
pub fn pin_com_mta();
```
Testes: `timer_alta_resolucao_encurta_esperas` (Windows: com o guarda, 50 ×
`std::sync::mpsc::Receiver::recv_timeout(2 ms)` num canal vazio têm média ≤ 4 ms);
`guardas_aninhados` (dois guardas, solta em qualquer ordem, `is_active` = true no Windows);
`pin_com_mta_idempotente` (chamar 3× de threads diferentes). No Linux os testes só conferem que não
há pânico.

### 5.3 `gpu` (`crates/zap-gpu`)

- `com.rs`: `pub fn pin_mta()` (mesma regra de `zap_sys::platform::pin_com_mta`); `ComGuard::new()`
  chama `pin_mta()` **antes** do `CoInitializeEx`. Reexporta em `lib.rs` (`zap_gpu::pin_mta`);
  `stub.rs` ganha a versão vazia.
- `device.rs`: `#[derive(Clone)]` em `GpuDevice` (e `AdapterInfo` se faltar); `stub.rs` idem.
  Comentário: clone = mesmo dispositivo e mesmo contexto imediato (`AddRef`), já protegido para
  multithread.
- Testes: `clone_compartilha_dispositivo` (o `as_raw()` do `ID3D11Device` é o mesmo; soltar o
  original e usar o clone para criar uma textura funciona); `pin_mta_idempotente`.

### 5.4 `net` (`crates/zap-net`)

`report.rs`:
- `ReportBuilder` ganha `probe_meter: ProbeMeter` (usado só quando `bwe` é `Some`).
- `on_video`/`on_audio`: também `probe_meter.on_media(arrival_us, bytes)`.
- `on_probe`: guarda em `probes` (como hoje); `bwe.on_probe_packet(send, arrival, bytes)` (com o
  mesmo desembrulhador de tempo); se `probe_meter.on_probe(..)` devolver resultado,
  `bwe.on_probe_cluster(arrival, r.kbps)`.
- `take(now)`: `on_loss_fraction`, depois `probe_meter.poll(now)` → `on_probe_cluster`, depois
  `update(now)`.

`media_tx.rs`:
```rust
impl MediaSender {
    /// Bytes UDP enviados na conexão até agora (`quinn::Connection::stats().udp_tx.bytes`).
    pub fn sent_bytes(&self) -> u64;
    /// `min(TAMANHO_SONDA, max_datagram_size())`; `None` sem suporte a datagrama.
    pub fn probe_size(&self) -> Option<usize>;
}
```

`probe.rs` (novo, `pub mod probe;`):
```rust
/// Sondagem de um espectador (uma por conexão). As trilhas informam a necessidade; quem chama
/// `poll` a cada ~100 ms e dispara `run_burst` numa tarefa quando ele devolve uma rajada.
pub struct Prober { /* sender: Arc<MediaSender>, planner: Mutex<ProbePlanner>, needs: Mutex<HashMap<u16, ProbeNeed>> */ }
impl Prober {
    pub fn new(sender: Arc<MediaSender>) -> Arc<Self>;
    /// Necessidade da trilha `track` (`None` = nada a testar nela). Vale a de menor `next_kbps`.
    pub fn set_need(&self, track: u16, need: Option<ProbeNeed>);
    /// Esquece a trilha (assinatura cancelada).
    pub fn clear_track(&self, track: u16);
    /// `ProbePlanner::poll(now_us, menor necessidade, bandwidth_kbps, sender.sent_bytes())`.
    pub fn poll(&self, now_us: u64, bandwidth_kbps: u32) -> Option<ProbeBurst>;
    /// Manda a rajada: `ProbePacer` com o relógio do `MediaSender`, tique `interval(5 ms)`
    /// (`MissedTickBehavior::Skip`), `send_probe(cluster, seq, probe_size)`; `seq` de 0 em diante.
    /// Erro de datagrama encerra a rajada. Devolve quantos datagramas saíram.
    pub async fn run_burst(&self, burst: ProbeBurst) -> Result<u32, NetError>;
}
```

Timer nos testes: `zap-sys` entra em `[dev-dependencies]`; `tests/support/mod.rs` ganha
`pub fn timer() -> zap_sys::platform::TimerResolution` e **todo** teste de integração de rede
(`impaired`, `media_loopback`, `handshake`, `online_e2e`, `proxy_selftest`) guarda um no começo.
Isso não muda a lógica dos testes: só põe o processo na mesma resolução que o app usa com sala.

Testes novos:
- `report.rs`: `sonda_sobe_banda` (vídeo 400 kbps por 2 s + rajada de 200 ms com 3,35 Mbps de
  sondas → `take().bandwidth_kbps ≥ 3195`); `sonda_nao_vira_sobrecarga` (atraso crescente durante
  as sondas → `state` nunca `Overuse` no relatório).
- `tests/probe.rs` (QUIC no loopback): emissor manda vídeo sintético de 400 kbps; `Prober` com
  `set_need(1, next 2500)`; receptor com `spawn_receiver` + `ReportBuilder(true)`; a rajada sai
  ~1 s depois e o relatório seguinte traz `banda_kbps ≥ 3195`; `run_burst` devolve entre 60 e 80
  datagramas (~3300 kbps × 200 ms / `probe_size`, que com MTU inicial de 1200 fica abaixo de 1200 B).

Aceite da frente: `cargo test -p zap-net --test impaired -- --nocapture` passa 3 vezes seguidas
(banda média em 2200..3600) e a saída mostra `enviados ≥ 1150`. Se com o timer de 1 ms ainda
falhar: anote `enviados`, `descartados_pub`, `forwarded_bytes/20 s`; se o proxy passar ≥ 2,7 Mbps
e a banda ficar abaixo de 2200, é o estimador (abra tarefa na frente `rate` com essa saída); se o
proxy passar < 2,5 Mbps com `dropped_packets` baixo, é a admissão em `media_tx/video.rs` (corrija
aqui). Não afrouxe a faixa do teste.

### 5.5 `captura` (`crates/zap-capture`)

- `dxgi.rs`, `wgc.rs`, `camera.rs`: em vez de `SendPtr/SendWrap(dev as *const GpuDevice)`, cada
  captura faz `let dev = dev.clone();` e move o clone para a thread (e, no WGC, para o handler de
  `FrameArrived`, dentro de um `Arc`). Some o contrato "o `dev` precisa continuar vivo até o
  handle cair" da documentação das três funções; somem os `unsafe impl Send` de ponteiro.
- Entradas que tocam WinRT/COM sem `ComGuard` chamam `zap_gpu::pin_mta()` primeiro:
  `wgc_supported`, `list_windows`, `list_cameras`, `native_modes`, `capture_window`,
  `capture_camera`.
- Se o stress abaixo ainda falhar depois disso: `MfGuard` passa a fazer `MFStartup` uma vez por
  processo (`OnceLock`) e nunca `MFShutdown` (mesma classe de bug: plataforma desligada com outra
  thread usando). Só com evidência.

Testes novos:
- `com_concorrente_com_winrt_e_mf`: 4 threads × 25 × `list_cameras()` e 4 threads × 25 ×
  `wgc_supported()` ao mesmo tempo, sem pânico nem queda.
- `capture_monitor_sobrevive_a_mover_e_soltar_o_dispositivo`: inicia a captura, move o `GpuDevice`
  para um `Box` e o solta; durante 1 s mexendo o mouse chega ≥ 1 quadro; `drop(handle)` termina.
- O mesmo para câmera (pula sem câmera) e janela (pula sem janela visível).

Aceite da frente (stress, sem filtro): o binário de teste da biblioteca roda 30 vezes seguidas sem
nenhuma saída diferente de 0 —
`BIN=$(ls -t target/debug/deps/zap_capture-*.exe | head -1); for i in $(seq 30); do $BIN >/dev/null 2>&1 || echo falhou $i; done`
(antes: 15 de 25 caíam). Serializar testes não vale: o bug é do produto.

### 5.6 `encode` (`crates/zap-encode`)

Tarefa 1, teste (`tests/windows.rs`):
- `frame()` passa a gerar conteúdo com informação: ruído determinístico (xorshift com semente =
  índice do quadro) em blocos de 8×8, deslocado a cada quadro, nos 1280×720. Mesma função para
  todos os testes do arquivo.
- `tamanho_medio_nao_chave` fica com a mesma asserção (média dos não-chave a partir do quadro 10
  dentro de ±50% de `expected_frame_bytes(4000, 30)` = 16666), agora sobre conteúdo que de fato
  pede banda. Roda com `Auto` (NVENC aqui) e ganha um irmão `tamanho_medio_nao_chave_software`
  (`EncoderChoice::Software`, mesma faixa).
- Novo `conteudo_estatico_nao_passa_do_orcamento`: cor sólida fixa → média dos não-chave
  ≤ 1,5 × 16666 (CBR não pode estourar; pode sobrar).
- Se com ruído a média sair da faixa, o problema é o controle de taxa: aplique
  `codec_api::apply_settings` **antes** de `SetOutputType`/`SetInputType` em `hw.rs`, leia de volta
  com `ICodecAPI::GetValue` (`AVEncCommonRateControlMode`, `AVEncCommonMeanBitRate`) e registre no
  log; repita. Não mude a faixa.

Tarefa 2, laço de eventos (`hw.rs`): uma thread `zap-encode-ev` por codificador de hardware faz
`GetEvent(0)` (bloqueante) e manda `(tipo, status)` por `crossbeam_channel::bounded(64)`; o laço
principal usa `crossbeam_channel::select!` entre eventos e quadros (`io.rx`, só quando
`need_input > 0`), com `default(100 ms)` só para olhar `stop`. Para encerrar: `IMFShutdown::Shutdown`
no MFT (o `GetEvent` volta `MF_E_SHUTDOWN` e a thread sai) e `join`. MFT sem `IMFShutdown`: a thread
usa `GetEvent(MF_EVENT_FLAG_NO_WAIT)` + `std::thread::sleep(1 ms)` (o `sleep` da `std` usa timer de
alta resolução) e olha um `AtomicBool`. Teste novo `latencia_hardware` (pula sem hardware): 60
quadros de ruído a 30 fps, `p95(encode_us) ≤ 12 ms`; anote média e p95 no relato.

Aceite: `cargo test -p zap-encode --test windows` passa 3 vezes seguidas;
`cargo test -p zap-encode` passa.

### 5.7 `servidor-teste` (`crates/zap-test-server`)

- `src/probe.rs` (novo):
```rust
/// Necessidade de sondagem do espectador `num` numa trilha (balanceamento.md §4): `current > teto`
/// (`relay::max_layer_for`) e sem `pending` → camada existente logo acima de `current`;
/// `reserve_kbps = 0` (o alocador do servidor de teste não reserva áudio).
pub fn need_for(track: &TrackData, num: u64) -> Option<ProbeNeed>;
/// Banda do último relatório do espectador nessa sala (de qualquer `ViewerEntry` dele; 0 se nenhum).
pub fn bandwidth_of(room: &RoomData, num: u64) -> u32;
```
- `state.rs::ParticipantData` ganha `pub prober: Option<Arc<zap_net::probe::Prober>>`. `bridge.rs::registrar`
  cria um `Prober::new(Arc::new(MediaSender::new(conn.clone(), hub.clock.clone())))` para conexões
  que não são prévia (substitui o anterior); a limpeza da conexão o tira.
- `ticker.rs::tick_all`: depois do alocador, sob o lock, para cada participante com `prober`:
  `set_need(trilha.number, need_for(..))` para cada trilha de vídeo da sala dele (`None` nas que ele
  não assiste) e `poll(now_us, bandwidth_of(..))`; as rajadas devolvidas saem **fora** do lock com
  `tokio::spawn(async move { let _ = prober.run_burst(b).await; })`.
- `lib.rs`: o `Handle` guarda um `zap_sys::platform::TimerResolution` (dependência normal de
  `zap-sys`); o cabeçalho passa a dizer que a ponte sonda.
- Testes: unitários de `need_for` (teto, pendente, camera sem camada 1, trilha sem o espectador);
  `tests/bridge.rs` ganha `sondas_levam_espectador_a_camada_0`: publicador sintético (camadas 0/1/2
  nos tamanhos nominais), espectador com `ReportBuilder::new(true)` + `spawn_reporter` de verdade
  (sem `banda_kbps` à mão), sem limite de enlace → `camada.atual` 0 em ≤ 20 s. Os testes existentes
  continuam passando (o espectador de teste deve ignorar datagramas `0x03`).

Verificação: `cargo test -p zap-test-server` (3 vezes o `tests/bridge.rs`), clippy e fmt do crate.

### 5.8 `media` (`crates/zap-media`)

- `examples/bancada.rs`: guarda `zap_sys::platform::TimerResolution::high()` durante a medição
  (`zap-sys` em `[dev-dependencies]`) e imprime `timer 1 ms: sim/não` no cabeçalho.
- `win_video.rs`: só ajustar comentários (a captura agora tem o próprio clone do dispositivo).
- Stress: `cargo test -p zap-media` 10 vezes seguidas sem queda.
- Bancada: `cargo build --release -p zap-media --example bancada` e 5 execuções de
  `target/release/examples/bancada.exe --monitor 1 --preset 1080p60 --segundos 10` **com a máquina
  ociosa** (nenhum `cargo`/`rustc` rodando: `tasklist | findstr /i "cargo rustc"` vazio). Nenhuma
  pode cair. Registre a mediana das 5 na seção 8 deste plano e em `m2-midia-windows.md` ("Estado do
  fechamento"), com o p95 somado da camada 0 contra a meta de 8 ms. Se a meta não for atingida,
  escreva qual etapa domina (conversão ou codificação) e o que a bancada mostra; não é bloqueio.

### 5.9 `core` (`crates/zap-core`)

**a) Timer com sala.** `Shared` ganha `timer: Mutex<Option<zap_sys::platform::TimerResolution>>` e
`fn set_active_room(&self, room: Option<Arc<ActiveRoom>>) -> Option<Arc<ActiveRoom>>` (troca o
`room` e pega/solta o timer: `Some` → guarda um se não houver; `None` → solta). Todos os pontos que
hoje escrevem `shared.room` (`engine_room.rs` 35, 123, 149, 227; `online/entry.rs` 52, 57) passam
por ela. `Core::start_with` chama `zap_sys::platform::pin_com_mta()`.

**b) `Drop` seguro.** `shutdown()` sempre roda o desligamento numa thread nova
`zap-core-shutdown` (que não tem contexto de runtime): ela faz o `block_on` de hoje e o
`shutdown_timeout(2 s)`. Quem chama espera o fim por um canal; se `std::thread::panicking()`, espera
no máximo 5 s e segue (a thread termina sozinha). Funciona igual de thread comum, de
`spawn_blocking` e de dentro de uma tarefa do tokio. Testes: `drop_dentro_de_tarefa_tokio`
(`#[tokio::test]` cria e solta um `Core` sem `spawn_blocking`) e `panico_com_core_vivo_nao_aborta`
(`tokio::spawn(async { let _c = core(); panic!() }).await` → `JoinError::is_panic()`, processo segue).

**c) Modo só direta.** `ponte_ativa(room, shared)` (hoje privada em `online/reconnect.rs`) vira
`OnlineRoom::bridge_enabled(&self, shared: &Shared) -> bool` (`use_relay && media.bridge.is_some()`).
```rust
// subscribe/mod.rs
pub struct Subscription { /* + */ pub(crate) spec: Mutex<WatchSpec> /* tamanho/teto atuais */ }
impl Subscription {
    /// Como `start`, mas na conexão dada (rota direta sem ponte), sem passar pelo `ConnPool`.
    pub(crate) async fn start_on(shared: &Arc<Shared>, spec: WatchSpec, conn: Arc<PubConn>) -> Result<Self, CoreError>;
    /// Pedido atual (o `update` grava tamanho/teto).
    pub(crate) fn spec(&self) -> WatchSpec;
    /// Para sem mandar `cancelar` (a conexão já caiu): aborta o tique, tira do `Router` e do playout.
    pub(crate) fn stop_local(&self, shared: &Arc<Shared>);
}
// online/direct.rs
pub struct DirectState { /* como hoje + */
    /// Só direta: tiles esperando a rota direta.
    pub(crate) pending: Mutex<HashMap<TileId, WatchSpec>>,
    /// Só direta: conexão direta por trilha (com ponte elas ficam em `bridge.direct`).
    pub(crate) conns: Mutex<HashMap<String, Arc<PubConn>>>,
}
/// Sem ponte: guarda o pedido e manda `assinar` pelo WebSocket (é o que faz o servidor mandar
/// `p2p.candidatos`). Mesmo tile com a mesma trilha: troca tamanho/teto e reenvia.
pub(crate) async fn watch_pending(room: &Arc<OnlineRoom>, spec: WatchSpec) -> Result<(), CoreError>;
/// Tira o tile de `pending` (e manda `cancelar` pelo WebSocket). `false` se não estava lá.
pub(crate) async fn unwatch_pending(room: &Arc<OnlineRoom>, tile: TileId) -> bool;
/// A trilha está assistida (em `shared.subs` ou em `pending`)?
pub(crate) fn is_watched(room: &OnlineRoom, shared: &Shared, track_id: &str) -> bool;
```
Comportamento:
- `engine_watch::watch`: sala Online e `!bridge_enabled` → `watch_pending` (geração do tile como
  hoje: um `unwatch` ou `watch` novo no meio vence); não mexe no `UiState` (o tile aparece quando a
  assinatura nascer, como hoje com vídeo). `unwatch` também chama `unwatch_pending`.
- `on_candidates` (`Client`): usa `is_watched` no lugar de "está em `shared.subs`".
- `tentar_direta` com ponte: como hoje. Sem ponte: `connect_direct(.., Arc::new(Router::new()))`;
  para cada `WatchSpec` pendente da trilha, `Subscription::start_on(.., direta)`; tira de `pending` e
  põe em `shared.subs` só se o tile ainda estava pendente (senão `stop`). Nenhuma assinatura nasceu
  → fecha a direta e `falhou`. Senão: `direct.conns[trilha] = direta`, `routes[trilha] = Direct`,
  `ok_sent`, WebSocket `cancelar` + `midia.direta_ok` (igual ao modo com ponte).
- Queda da direta sem ponte: se `conns[trilha]` ainda é ela, sai de `conns` e de `routes`; cada
  assinatura em `shared.subs` com essa trilha e `Arc::ptr_eq(sub.conn, direta)` sai de `subs`, faz
  `stop_local`, sai do `UiState` (`remove_tile` + `touch`) e volta para `pending` com `sub.spec()`;
  depois `assinar` de novo pelo WebSocket (se `!room.left`).
- `direta_falhou` sem ponte: além da mensagem ao servidor, um `push_error` por trilha: "Não deu para
  conectar direto em {nome}. Ligue “Usar o servidor como ponte” em Configurações para assistir pelo
  servidor." O tile segue pendente; novos `p2p.candidatos` tentam de novo.
- `reconnect::volta_ws`: reenvia `assinar` também para `pending`. Sair da sala: fecha
  `direct.conns` e limpa `pending`.

**d) Publicador sonda (Local e direto).**
- `publish/viewer.rs`: `ViewerLink` ganha `pub prober: Option<Arc<Prober>>` (novo parâmetro de
  `ViewerLink::new`). `media_serve.rs::serve_viewer` cria **um** `Prober::new(sender.clone())` por
  conexão e passa a todos os links de vídeo dela; links de áudio e de ponte recebem `None`. Ao
  cancelar a trilha: `prober.clear_track(track)`.
- `publish/alloc_loop.rs`:
```rust
/// `current > ceiling()` e sem `pending` → a camada da escada logo acima de `current`
/// (`reserve_kbps = 0`, como o `input(id, 0)` do alocador). Link de ponte → `None`.
pub(crate) fn probe_need(state: &ViewerState, ladder: &[LayerSpec]) -> Option<ProbeNeed>;
```
  Em `run_alloc_tick`, depois das decisões, para cada link que não é de ponte e tem `prober`:
  `set_need(shared.track, probe_need(..))`, `poll(now, state.bandwidth_kbps)` e, se vier rajada,
  `rt.spawn(async move { let _ = p.run_burst(b).await; })`.

**e) Testes.**
- `online_aceite_1.rs` e `online_aceite_2.rs`: só tirar os `println!("DIAG ..")` (no 2, o bloco
  inteiro de diagnóstico dentro do `if !ok`, inclusive o laço que lê `srv.hub`). Nada mais muda.
- `online_aceite_4.rs`: tirar `#[ignore]` e juntar `sem_use_relay_direta_com_candidatos` em
  `sem_use_relay_so_direta` (3 s sem vídeo sem candidatos; depois `injetar_candidatos_reais`,
  ≥ 10 pacotes em 10 s e `online_route == Some(Route::Direct)`); o arquivo fica com as 3 funções da
  spec da `core-online-24`.
- Novo `tests/aceite_camadas_local.rs`: sala Local no loopback, P publica 1080p60 sintético, V1
  assiste `Main`, V2 `Thumbnail`; em ≤ 20 s V1 na camada 0 e V2 na 2, com ≥ 10 pacotes cada, e as
  camadas não mudam por 3 s. (Prova a sondagem do publicador.)
- Unitários: `probe_need` (teto, pendente, ponte), `watch_pending`/`unwatch_pending`,
  `is_watched`, `set_active_room` pega e solta o timer.

**f) Fechamento (`core-online-27`).** Na ordem: `cargo fmt --all --check`; `cargo clippy --workspace
--all-targets -- -D warnings`; `cargo test --workspace`; `cargo check --workspace --all-targets`; cada
teste de aceite Online (1, 2, 3, 4, prévia, reconexão) e `aceite_camadas_local` 3 vezes seguidas
(relate os instáveis). Depois: em `docs/planos/m4-tarefas-pendentes.json`, as tarefas
`core-online-22`, `-24` e `-27` ganham `"estado": "concluida"` e uma `"nota"` de uma linha;
`m4-online.md` ("Estado do fechamento") e `docs/ESTADO.md` (tabela e "Atualização do M4") passam a
dizer o que fechou e o que ficou (servidor de produção precisa sondar; seção 7).

## 6. Testes de aceite do M4b

| # | Teste | Esperado |
|---|---|---|
| 1 | `zap-core --test online_aceite_2` | A=1, B=0, C=2 e P com camadas {0,1,2} em ≤ 30 s; estável 3 s. Previsão (relógio desde o `watch`): rajada para B e A em ~1 s; B e A na camada 1 em ~4,5 s; rajada de B para a 0 em ~6,2 s; B na 0 em ~10 s; A sondado para a 0 em ~6,2 s e ~16,4 s, falha e fica na 1 |
| 2 | `zap-core --test online_aceite_4` (3 funções) | A e B como hoje; C: 0 pacotes em 3 s sem candidatos, ≥ 10 em 10 s com candidatos, rota `Direct` |
| 3 | `zap-core --test aceite_camadas_local` (novo) | V1 na 0 e V2 na 2 em ≤ 20 s; estável 3 s |
| 4 | `zap-core` aceites 1, 3, prévia, reconexão | Continuam passando (3 vezes seguidas) |
| 5 | `zap-net --test impaired` | `rede_ruim_3mbps_fonte_6mbps` com banda média em 2200..3600, `enviados ≥ 1150`, 3 vezes seguidas |
| 6 | `zap-net --test probe` (novo) | `banda_kbps ≥ 3195` no relatório depois de uma rajada para a camada 1 |
| 7 | `zap-test-server --test bridge` | `sondas_levam_espectador_a_camada_0` em ≤ 20 s; os antigos passam |
| 8 | `zap-rate` | Testes da seção 5.1 |
| 9 | `zap-capture --lib` | 30 execuções seguidas sem queda; testes novos de concorrência e de dispositivo movido |
| 10 | `zap-encode --test windows` | Todos passam 3 vezes; média dos não-chave com ruído em 8333..24999 B (hardware e software); estático ≤ 24999 |
| 11 | Bancada release | 5 execuções sem queda; números na seção 8 |
| 12 | Workspace | fmt, clippy `-D warnings`, `cargo test --workspace`, `cargo check --workspace --all-targets` |

## 7. Riscos

| Risco | Mitigação |
|---|---|
| Rajada acima da capacidade derruba quem está estável (A no aceite 2: 12 Mbps num enlace de 4) | Carência de 300 ms no `Bwe`; o grupo só sobe a estimativa (`max`); intervalo dobra até 60 s. Teste `bwe_sonda_acima_da_capacidade_nao_derruba` |
| Erro de medida deixa a estimativa abaixo do limiar (3195 para a camada 1) | Alvo 1,5× (precisa de 1,35×); testes do medidor com chegadas em degraus de 15,6 ms |
| `enviado(e)` conta cabeçalho do QUIC e a sonda sai um pouco menor | ~3–8% dentro da margem de 10%; se o aceite 2 ficar no limite, subir `FATOR_ALVO` para 1,6 é a única mudança permitida (e vai junto no contrato) |
| Servidor de produção ainda não sonda | Fica registrado em `ESTADO.md`; o contrato (§4) e a referência em código bastam para implementar |
| `timeBeginPeriod(1)` gasta energia | Só com sala ativa; solto ao sair |
| Algum fabricante de MFT sem `IMFShutdown` | Plano B na própria thread de eventos (seção 5.6) |
| O crash do `zap-capture` tem uma segunda causa além do MTA | O stress de 30 execuções decide; plano B do `MFStartup` único (seção 5.5) |
| Modo só direta mexe no caminho do `watch` usado por todos os modos | O caminho novo só entra com sala Online e `!bridge_enabled`; os aceites Local e Online com ponte rodam 3 vezes no fechamento |
| Testes do `zap-core` instáveis com 4 `Core`s e proxy no mesmo processo | Timer de 1 ms no processo (via sala ativa) deixa proxy e quinn precisos; relatar instáveis no fechamento |

## 8. Bancada: números

Antes (2026-11-04, ver 2.4): camada 0 a 17,6–32,8 fps, codificação média 17,7–31,2 ms, p95 somado
71–207 ms, 2 quedas em 4 execuções release.

Depois (fechamento, 2026-11-04, uma execução release de 10 s, `--monitor 1 --preset 1080p60`, timer de
1 ms ativo, NVIDIA H.264 Encoder MFT): **meta NÃO atingida**. A bancada mostra a GPU praticamente
parada (nvidia-smi: 6 % de uso, 607 MHz) e etapas de dezenas a centenas de ms.

| Camada | WxH | fps | kbps | conversão média/p95 | codificação média/p95 |
|---|---|---|---|---|---|
| 0 | 1920×1080 | 2,9 | 208 | 46,48 / 200,79 | 123,11 / 502,28 |
| 1 | 1280×720 | 2,4 | 112 | 39,72 / 187,04 | 111,38 / 403,04 |
| 2 | 640×360 | 1,9 | 30 | 59,57 / 501,05 | 105,74 / 602,40 |

Captura média/p95: 0,76 / 0,03 ms; descartados: captura 8, codificação 0; p95 somado da camada 0:
703,1 ms (meta ≤ 8 ms); quedas: 0 de 1 (sem violação de acesso; 10 de 10 execuções de
`cargo test -p zap-media` sem queda). As execuções da frente `media` (5 de 10 s) deram 0,5 fps e
p95 somado de 2217 a 2625 ms.

**Essas medições não valem: a área de trabalho estava parada.** A duplicação de tela (DXGI) só
entrega quadro quando a tela muda; com a tela parada chegam ~0,5 quadro/s, e o codificador de
hardware segura cada quadro até o próximo chegar, então "conversão" e "codificação" medem a espera
pelo quadro seguinte (por isso a GPU estava a 6 %). Repetido com uma janela de 320×200 mudando de
cor a ~60 Hz no canto do monitor 1 (mesmo comando, release, timer de 1 ms, NVIDIA H.264 Encoder MFT):

| Camada | WxH | fps | kbps | conversão média/p95 | codificação média/p95 |
|---|---|---|---|---|---|
| 0 | 1920×1080 | 25,9 | 1083 | 12,36 / 30,28 | 21,01 / 41,48 |
| 1 | 1280×720 | 22,8 | 952 | 8,38 / 30,01 | 22,71 / 48,55 |
| 2 | 640×360 | 14,5 | 250 | 7,75 / 30,43 | 21,56 / 48,46 |

Captura média/p95: 0,19 / 0,03 ms; descartados: captura 134, codificação 149; p95 somado da camada
0: 71,8 ms (meta ≤ 8 ms). Bate com o "Antes" (71–207 ms): esta rodada não piorou nada, e a meta
continua **não atingida**. Pendências: (1) a bancada precisa gerar movimento na tela sozinha (ou
avisar quando a tela está parada), senão mede espera; o mesmo vale para
`win_video::tests::desligar_camada_para_de_emitir`, instável pela mesma causa; (2) investigar por que
conversão (~12 ms) e codificação (~21 ms) estão uma ordem de grandeza acima do esperado numa GTX 1060
(contenção do contexto D3D11 entre conversor e MFT, relógio baixo da GPU, três sessões NVENC) e os
quadros descartados.

## 9. Estado do fechamento (2026-11-04)

| # | Teste (seção 6) | Resultado |
|---|---|---|
| 1 | `online_aceite_2` | **FALHA** (1 de 4 execuções passou): A fica na 2 ou as camadas oscilam |
| 2 | `online_aceite_4` | ok, 3 vezes |
| 3 | `aceite_camadas_local` | ok, 3 vezes |
| 4 | aceites 1, 3, prévia, reconexão | ok, 3 vezes |
| 5 | `zap-net --test impaired` | ok no `cargo test --workspace` (6 testes) |
| 6 | `zap-net --test probe` | ok (6) |
| 7 | `zap-test-server --test bridge` | ok (3) |
| 8 | `zap-rate` | ok (107 + 5 de integração) |
| 9 | `zap-capture` | ok (33 + 10 de integração; 30 execuções seguidas não repetidas aqui) |
| 10 | `zap-encode --test windows` | **FALHA** `tamanho_medio_nao_chave_software`: 42199 B contra 8333..24999 (hardware passa) |
| 11 | Bancada release | meta não atingida (seção 8) |
| 12 | Workspace | fmt, clippy `-D warnings` e `check` ok; `cargo test --workspace` com falhas abaixo |

Falhas de `cargo test --workspace --no-fail-fast` na última rodada (4):
1. `zap-core --lib engine::tests::set_active_room_pega_e_solta_timer`: o teste supunha que
   `create_room` não pegava o timer; desde 5.9 a) ele pega. **Corrigido** (teste ajustado).
2. `zap-core --test online_aceite_2`: ver acima; bug de produto (sondagem), não do teste.
3. `zap-encode --test windows tamanho_medio_nao_chave_software`: o MFT de software (Microsoft
   H.264) não segura 4000 kbps com ruído e gasta ~2,5× o nominal. A expectativa de ±50% vale para o
   hardware; o de software precisa de outro limite ou de um controle de taxa melhor.
4. `zap-media --lib win_video::tests::desligar_camada_para_de_emitir`: instável (passou em 4 de 4
   rodadas isoladas). A duplicação de tela só entrega quadros quando a tela muda e o teste exige
   quadros em 1 s; com a máquina ocupada, 0 quadros.
