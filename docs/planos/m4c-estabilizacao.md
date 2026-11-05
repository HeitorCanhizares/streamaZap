# M4c + M2: estabilização (rodada 3)

Plano do arquiteto. Continua `docs/planos/m4b-pendencias.md` (rodada 2: sondagem, modo só direta,
correções na GPU real; seções 8 e 9 têm os números e as falhas de partida), `docs/planos/m4-online.md`
(aceites do M4; o 2 é o único aberto) e `docs/planos/m2-midia-windows.md` (mídia no Windows; meta
da bancada). Estado de partida: branch `m4-online`, commit `e3b18c6`. PC de referência: Windows 10,
MSVC, GTX 1060 (NVENC), monitor 1 1920×1080 a 164 Hz (primário), monitor 2 à esquerda.

Contrato e plano mestre já atualizados junto com este plano (as frentes implementam, não reescrevem):
- `docs/api/balanceamento.md` §4 (sondagem em degraus, medida por quem sonda, resultado em três
  casos, carência de rajada própria com keyframes) e §6 (itens 5 e 6);
- `docs/api/midia.md` (prazo do keyframe e dos quadros atrás dele; admissão; estado por conexão
  na ponte);
- `docs/arquitetura-v3.md` §2 (três decisões novas), §5 (captura DXGI), §6.1 itens 4 e 5, §6.3.

## 0. Resumo

| # | Objetivo | Causa achada hoje (seção 2) | Correção |
|---|---|---|---|
| 1 | Aceite 2 passa 5 de 5 | (a) o keyframe da camada 1 (83 KB) num enlace de 4 Mbps estoura o prazo de 150 ms e é resetado junto com os 3 quadros seguintes, e a fila que ele cria vira "sobrecarga" no estimador: A cai para a 2 ~300 ms depois de subir para a 1, a cada troca e a cada keyframe; (b) a rajada de 12000 kbps (1→0) perde ~70 pacotes e reseta 16 quadros de A | Prazo próprio do keyframe e extensão dos quadros atrás dele (`zap-net`); carência de rajada própria no `Bwe` cobrindo sondas e keyframes até a fila escoar (`zap-rate`); sondagem em degraus `1,5 × M` com a medida tirada do relatório (`zap-rate` + `zap-net`); ponte com um `MediaSender` por conexão (`zap-test-server`) |
| 2 | Bancada: p95 captura+conversão+codificação da camada 0 ≤ 8 ms em 1080p60, 60 fps | (a) a duplicação só entrega quadro com a tela mudando; (b) a thread de captura espera no `AcquireNextFrame(100 ms)` **segurando o lock do dispositivo D3D11** que o conversor e os 3 MFTs compartilham: as etapas medem espera pelo lock | Bancada e testes geram movimento sozinhos (`zap-capture::movimento`); captura com `AcquireNextFrame(0)` + `WaitForVBlank`; medição por etapa com consultas de tempo da GPU e espera de lock; dispositivo por codificador só se a medida pedir (etapa B) |
| 3 | `tamanho_medio_nao_chave_software` | Opções do `ICodecAPI` aplicadas **depois** dos tipos de mídia (o MFT pode ignorar) e sem leitura de volta no software | Opções antes dos tipos, leitura de volta completa, controle de taxa testado em ordem; só se nenhum segurar ruído, conteúdo realista + teste de ruído com limite largo documentado |
| 4 | `renovacao_automatica_dispara` determinístico | Tempo pausado do tokio com E/S HTTP de verdade: o relógio avança sozinho enquanto o loopback responde e o timeout de 10 s do `reqwest` dispara no `login` (pânico na linha 474 = `login(..).unwrap()`) | Relógio da renovação injetado; teste com tempo real e sincronização por canal |

## 1. Escopo

Entra:
- `zap-rate`: carência de rajada própria no `Bwe` (sondas e keyframes, fila escoada, descarte pelo
  instante visto); `DelayDetector::fila_us`; `ProbePlanner` em degraus com `on_result`;
  `ProbeMeter::finish`; `ProbeBurst::total_kbps`.
- `zap-net`: prazo do keyframe e extensão dos quadros atrás dele, admissão sem keyframes em voo;
  cabeçalho de keyframe avisa o estimador; `GapTracker` devolve o instante visto;
  `Prober::on_report`; relógio injetado na renovação do token e testes com tempo pausado
  auditados.
- `zap-test-server`: um `MediaSender` por conexão de espectador (vídeo, áudio e sondas);
  relatório do espectador alimenta `Prober::on_report`.
- `zap-core`: relatório do espectador alimenta `Prober::on_report` no publicador; aceite 2 5 de 5;
  fechamento do M4 (`core-online-22` e `-27`).
- `zap-gpu`: `GpuTimer` (consultas de tempo), lock do dispositivo com medida de espera, cache das
  visões do `Converter`.
- `zap-capture`: `AcquireNextFrame(0)` + espera fora do lock; teste de contenção; `movimento`
  (janela animada).
- `zap-encode`: `ICodecAPI` antes dos tipos, leitura de volta completa, controle de taxa do MFT de
  software; teste de "não segura quadro".
- `zap-media`: bancada com `--animar` (padrão), aquecimento, aviso de tela parada, colunas novas;
  testes `win_video` com movimento ou pulo claro; medição final e números neste plano e no M2.

Fica de fora:
- Servidor de produção (repositório privado): segue `balanceamento.md` §4 e §6 e `midia.md`.
- Ritmo próprio do vídeo no envio (pacing de keyframe em pedaços): reserva, só se o aceite 2 ainda
  falhar depois das correções (seção 7).
- Mudança de histerese no alocador (`allocator/hysteresis.rs`): **não muda**. Motivo: a queda de A
  vem de sinais do estimador causados por rajadas do próprio sistema; mascará-los na origem
  (carência) é preciso, e atrasar a descida para todo mundo atrasaria a reação a congestionamento
  real. O §3 do contrato fica igual.
- Dispositivo por codificador com textura compartilhada (etapa B da seção 5.8): só se a medida da
  etapa A passar no portão.
- Conversão NV12 pela CPU, cadeia de dispositivos no CI, câmera (continua como está; a medida de
  espera de lock vale também para ela e vira pendência se aparecer).

Regras para todas as frentes: uma frente por crate (trava por crate); não afrouxar testes de
aceite (corrige-se o produto); `CoreOptions::default()` continua com `default_server: None` e os
testes só falam com o `zap-test-server`; sem commit, push ou troca de branch; todo comando de cargo
começa com `export PATH="$HOME/.cargo/bin:/c/Program Files/CMake/bin:$PATH";` e o `--target
x86_64-pc-windows-gnu` vira `cargo check --workspace --all-targets`; caminhos absolutos (raiz
`C:/Users/heito/Desktop/Projetos/streamazap`). Desempenho só vale com movimento na tela e máquina
ociosa (`tasklist | findstr /i "cargo rustc"` vazio): a medição final fica para o fechamento.
Saídas de medição vão para o scratchpad
`C:/Users/heito/AppData/Local/Temp/claude/a126e729ff4629a0/acd536d0-6c64-4aca-8e82-23354094a821/scratchpad`.

## 2. Diagnósticos (medidos em 2026-11-04 neste PC)

### 2.1 Aceite 2: o que derruba A

Teste de diagnóstico temporário (cópia do aceite 2 que lê a ponte a cada 50 ms: banda, estado,
camada atual/pendente de cada espectador, `quinn::ConnectionStats` da conexão ponte→A, contadores
do proxy, log `debug` de `zap_net::media_tx` e `zap_test_server`). Ficou no scratchpad como
`diag_aceite2.rs` (receita em 5.4 e); as saídas são `diag_a2_1.txt` e `diag_a2_2.txt`. Já foi
apagado de `crates/`.

Execução 1 (o aceite teria falhado com "camadas oscilaram: A=Some(2) B=Some(0) C=Some(2)"):

| t (ms desde o `watch`) | A (visto pela ponte) | Proxy (enviados, descartados) |
|---|---|---|
| 1240–1500 | banda 1176 → 3508 (rajada 2→1, mediu ~3700) | (99, 0) → (170, 0) |
| 4550 | troca para a camada 1 (keyframe) | (340, 0) |
| 4601–4859 | descartes no proxy; **Overuse**, banda 1132 | (357, 10) → (459, 14) |
| 4963 | volta para a 2 | |
| 9913 → 10325 | sobe para a 1 → Overuse (banda 990) → volta para a 2 | (830, 14) → (946, 37) |
| 10427 | app vê A=1, B=0, C=2: o aceite começa os 3 s de estabilidade | |
| 10891 | app vê A=2: **falha** | |
| 15026 → 15489, 20340 → 20751 | o mesmo ciclo a cada subida | +14, +10/+9 |

Log da ponte: a cada subida de A, `quadro de vídeo atrasado, stream resetado seq=0,1,2,3` (o
keyframe da camada 1 e os 3 quadros seguintes). **A rajada 1→0 de A nunca chegou a sair**: A não
ficava na camada 1 tempo bastante.

Execução 2 (com `ConnectionStats`): a subida para a 1 em 4444 ms leva o `cwnd` de 13,6 KB a 66 KB
(o keyframe não é limitado pela aplicação: partida lenta do Cubic), RTT de 8 para 87 ms, 10 pacotes
perdidos, 4 eventos de congestionamento; Overuse com banda 2600, que por sorte ficou acima do limiar
de descida (2639 × 0,9 ≥ 2375 só 517 ms depois). A **sobreviveu**, chegou ao alvo em 10,0 s e o
aceite teria passado. Depois:
- 5269: B sobe para a 1 e pede keyframe da camada 1, que A também recebe: +8 perdas, resets 26–27;
- 6459: rajada 1→0 de A (sonda ≈ 9500 kbps): perdas 18 → 29, eventos 9 → 13, **resets 61–76 e
  88–90**; A sobreviveu porque a banda tinha subido sozinha (fase inicial, 1,5×/s) até ~3300;
- 16899: segunda rajada 1→0 (10 s depois, banda de A já em 5143 por crescimento sem prova):
  descartes no proxy 29 → 100, perdas 46 → 100, **53 eventos de congestionamento**, banda 2402 →
  Overuse → A na 2;
- 25438, 30643, 35911: cada nova subida para a 1 → keyframe → Overuse → volta para a 2.

Conclusões:
1. **Keyframe da troca de camada** (o mecanismo dominante): 83 KB (8 × 10,4 KB, `zap-media::logic::
   synthetic_frame_size`; um codificador real com VBV de 250 ms chega perto disso) levam 167 ms a
   4 Mbps. O prazo de `max(150 ms, 2 × RTT)` reseta o keyframe e os quadros atrás dele: a camada 1
   **nunca fica decodificável** para A (o aceite não vê, porque conta pacotes e `camada`, mas um
   usuário veria a imagem parada). A fila que sobra (perdas no proxy, retransmissão, ~0,5 s para
   esvaziar com 1,5 Mbps de folga) dispara a "fila persistente" (`trendline.rs`: mínimo de 300 ms
   30 ms acima da base) depois que o filtro de rajada (200 ms) expira; o corte vai para
   `0,85 × vazão` e, com a vazão limitada a ~2500 pela própria camada, sempre abaixo do limiar de
   descida (2639). O espectador cai para a 2.
2. **Rajada acima do gargalo** (o mecanismo descrito no pedido): confirmado na execução 2. Além do
   estimador, a rajada derruba o **vídeo**: os pacotes dos streams de vídeo são descartados junto
   com as sondas no gargalo e o Cubic corta a janela.
3. A carência de 300 ms depois da última sonda não basta: a fila pode durar mais que isso, e os
   descartes do emissor são registrados quando o buraco de `seq` **vence** (visto + 300 ms,
   `media_rx::GAP_TIMEOUT`), já fora da carência.
4. A ponte cria um `MediaSender` por quadro e por espectador (`zap-test-server/src/media.rs::
   sender_for` dentro de `forward_video`): o `backlog` da admissão é sempre zero ali, e qualquer
   estado por conexão (como o do prazo novo) se perde.
5. Teto da sondagem relativo à estimativa não serve: a estimativa sobe sozinha até 2 × a vazão
   (regra de fonte limitada) sem nada que a prove (A chegou a 5143 com 2500 de vídeo).

### 2.2 Mídia na GPU

Números de partida (m4b-pendencias.md §8, release, timer de 1 ms, NVIDIA H.264 Encoder MFT):

| Situação | Camada 0 fps | conversão média/p95 | codificação média/p95 | p95 somado |
|---|---|---|---|---|
| Tela parada | 2,9 | 46,48 / 200,79 | 123,11 / 502,28 | 703 ms |
| Janela animada a ~60 Hz | 25,9 | 12,36 / 30,28 | 21,01 / 41,48 | 71,8 ms |
| Codificador sozinho (`zap-encode/tests/latencia.rs`, 1280×720 a 30 fps, sem captura) | — | — | p95 ≤ 12 ms (passa) | — |

- `zap-capture/src/dxgi.rs`: `ESPERA_MS = 100`; a thread de captura chama `AcquireNextFrame(100)`
  no **mesmo** `GpuDevice` (clone) que o conversor e os 3 MFTs (`win_video.rs::spawn`), com
  `SetMultithreadProtected(TRUE)` (`zap-gpu/src/device.rs`). O lock do dispositivo fica preso
  durante toda a espera do `AcquireNextFrame` (relato e correção do Sunshine/Apollo, commit "Fix
  starvation of encoder thread when not receiving new captured frames"). Bate com os números: tela
  parada → a espera dura até 100 ms (conversão p95 ≈ 200 ms, codificação, que trava o lock várias
  vezes por quadro, ≈ 500 ms); tela mudando a 60 Hz → espera de até 16 ms (conversão 12/30,
  codificação 21/41); codificador sem captura → p95 ≤ 12 ms. GPU a 6% e 607 MHz: ela estava
  parada esperando a CPU.
- `Converter::convert` (`zap-gpu/src/converter.rs`) cria `VideoProcessorInputView` e `OutputView`
  a cada quadro, sob o mesmo lock.
- Os descartes (captura 134 = slot sobrescrito; codificação 149 = fila de 2 cheia) são
  consequência: a thread de vídeo e os MFTs ficam parados no lock.
- Tela parada: a duplicação só entrega quadro quando a tela muda (~0,5 quadro/s); a bancada mede
  espera e `win_video::tests::desligar_camada_para_de_emitir` (exige quadros em 1 s) falha às
  vezes.
- Hipóteses que ficam para a medida confirmar ou descartar (não há dado que as sustente agora):
  relógio baixo da GPU (aquecimento antes de medir resolve na bancada), 3 sessões NVENC no mesmo
  dispositivo (contenção que sobra depois da etapa A → etapa B).

### 2.3 Codificador de software

`tamanho_medio_nao_chave_software`: média 42199 B contra 8333..24999 (4000 kbps, 30 fps,
1280×720, blocos 8×8 de cor aleatória nova a cada quadro); o NVENC passa. `sw.rs::open` aplica
`codec_api::apply_settings` **depois** de `SetOutputType`/`SetInputType` e não lê nada de volta
(`hw.rs` lê só modo e bitrate). A documentação do H.264 Encoder da Microsoft pede várias opções
(modo de controle de taxa, baixa latência, B-frames) antes do tipo de saída; depois disso o MFT
pode aceitar o `SetValue` e ignorar. Não se sabe ainda em que modo ele rodou.

### 2.4 `renovacao_automatica_dispara`

`#[tokio::test(start_paused = true)]` com servidor `axum` em loopback de verdade. Com o tempo
pausado o tokio avança o relógio sozinho quando nenhuma tarefa está pronta; esperando a resposta
HTTP (E/S real), o timeout de 10 s do `reqwest` (`client.rs:48`) vence em tempo virtual e o
`login(..).unwrap()` (linha 474) entra em pânico. Sozinho, o loopback responde antes; no
`cargo test --workspace` (máquina carregada), não. O mesmo vale para a espera da renovação e para
`tarefa_termina_ao_descartar_cliente`. O código de produção (`renovacao_automatica`, `renew_delay`)
está certo; o defeito é o teste misturar relógio virtual com E/S real.

## 3. Decisões

| Decisão | Motivo |
|---|---|
| Keyframe com prazo `max(600 ms, 2 × RTT)`; quadro comum conta o prazo a partir do fim do último keyframe em voo da mesma conexão; admissão sem os bytes desses keyframes | 2.1 (1). Perder o keyframe é pior que atrasá-lo: sem ele nada decodifica até o próximo. Custa até ~0,5 s de atraso a mais logo depois de um keyframe num enlace justo; o jitter buffer absorve |
| Carência de rajada própria no `Bwe` (sondas e keyframes), fechando quando a fila escoa, no máximo 1 s | 2.1 (1) e (3). A fila do keyframe dura ~0,5 s num enlace com 1,6× de folga e ~1 s no limite de subida (1,28×); congestionamento de verdade não fica escondido mais que 1 s |
| Descarte do emissor contado pelo instante em que o buraco foi **visto** | 2.1 (3): o buraco vence 300 ms depois, fora de qualquer janela |
| Sondagem em degraus: teto `1,5 × M` (sem saturar) / `1,25 × M` (saturou); `M` medido por quem sonda com o relatório do espectador; resultado em sucesso / degrau (próxima em 1 s) / falha | 2.1 (2) e (5). `1,5 × M` por 200 ms cabe numa fila de 100 ms de um gargalo `≥ M` (conta em `balanceamento.md` §4). Medir pelo relatório não muda mensagem nenhuma (`sondas` e `chegadas` já vão nele) |
| Ponte com um `MediaSender` por conexão de espectador | 2.1 (4): prazo novo e admissão precisam de estado por conexão |
| Sem mudança na histerese do alocador | Seção 1 ("Fica de fora") |
| Sem ritmo próprio de vídeo (pacing) nesta rodada | O prazo novo + a carência cobrem as perdas do keyframe; pacing exige escrever o quadro em pedaços com timer e entra só se 5 de 5 não vier (seção 7) |
| Captura DXGI: `AcquireNextFrame(0)` e espera em `IDXGIOutput::WaitForVBlank` (sem o lock); reserva de 2 ms de `sleep` se `WaitForVBlank` falhar | 2.2. Mudança mínima e com causa conhecida; acorda uma vez por atualização do monitor (164/s aqui, custo de CPU desprezível) |
| `zap_capture::movimento::JanelaAnimada` (thread própria, só bancada e testes) | Bancada e testes precisam de tela mudando; sem dependência nova (só features do `windows`), uma thread enquanto a janela existe |
| Medida por etapa com `D3D11_QUERY_TIMESTAMP(_DISJOINT)` e espera de lock (`ID3D11Multithread::Enter` explícito) | Separar trabalho da GPU de espera na CPU; sem isso a etapa B seria chute |
| Dispositivo por codificador só se o portão da etapa B abrir | Texturas compartilhadas e mutex chaveado custam código e sincronização; só se a medida mostrar contenção |
| `ICodecAPI` antes dos tipos de mídia, com leitura de volta de tudo | 2.3; vale para hardware e software |
| Renovação do token com relógio injetado (`RenewClock`) | 2.4. Teste em tempo real, sincronizado por canal; sem `start_paused` com E/S real |

## 4. Frentes

| Id | Crate | Depende de | Resumo |
|---|---|---|---|
| `rate` | `zap-rate` | — | Carência de rajada própria; `fila_us`; planejador em degraus (`on_result`, `total_kbps`); `ProbeMeter::finish`; testes com `sim::Link` |
| `net` | `zap-net` | `rate` (só a parte A; a parte B começa já) | A: prazo do keyframe e extensão, admissão, cabeçalho de keyframe no estimador, instante visto do buraco, `Prober::on_report`. B: renovação com relógio injetado e auditoria dos testes com tempo pausado |
| `servidor-teste` | `zap-test-server` | `net`, `rate` | `MediaSender` por conexão; relatório → `Prober::on_report`; testes da ponte |
| `core` | `zap-core` | `net`, `rate`, `servidor-teste` | Publicador: relatório → `Prober::on_report`; aceite 2 5 de 5; demais aceites; fechamento do M4 |
| `gpu` | `zap-gpu` | — | `GpuTimer`; `GpuDevice::lock` com espera medida; cache de visões no `Converter`; tempos do `Converter` |
| `captura` | `zap-capture` | — | `AcquireNextFrame(0)` + `WaitForVBlank`; teste de contenção do lock; `movimento::JanelaAnimada` |
| `encode` | `zap-encode` | — | `ICodecAPI` antes dos tipos e leitura de volta; controle de taxa do software; teste "não segura quadro" |
| `media` | `zap-media` | `gpu`, `captura`, `encode` | Bancada (`--animar`, aquecimento, aviso, colunas novas); estatísticas novas; testes `win_video` com movimento; medição final; portão da etapa B |

Caminho crítico do aceite 2: `rate` → `net` (parte A) → `servidor-teste` → `core`. Caminho da
bancada: `gpu` e `captura` → `media`. `encode`, `net` parte B, `gpu` e `captura` começam já.
Se o portão da etapa B abrir (5.8), ela vira três frentes novas em sequência curta: `gpu` (texturas
compartilhadas) → `encode` (dispositivo próprio) → `media` (ligação), com a API da seção 5.8.

## 5. Frentes em detalhe

### 5.1 `rate` (`crates/zap-rate`)

Sem tokio, sem I/O; tempo em `Us` como parâmetro.

`trendline.rs`:
```rust
impl DelayDetector {
    /// Fila atual estimada (µs): atraso de ida do último grupo fechado menos o menor atraso dos
    /// últimos 10 s (`atraso_min`). 0.0 antes de dois grupos. Nunca negativo.
    pub fn fila_us(&self) -> f64;
}
```
(Guardar o atraso do último grupo e a base em campos; o resto do detector não muda.)

`bwe.rs` (troca `CARENCIA_SONDA_US` e `ultima_sonda` por uma carência só):
```rust
/// Mínimo da carência depois da última sonda (keyframe não tem mínimo).
const CARENCIA_SONDA_MIN_US: Us = 300_000;
/// Máximo da carência depois do último evento de rajada própria.
const CARENCIA_MAX_US: Us = 1_000_000;
/// Fila (atraso acima da base) abaixo da qual a rajada já escoou.
const FILA_ESCOADA_US: f64 = 30_000.0;

impl Bwe {
    /// Sonda chegou: `on_packet` + evento de rajada (sonda) em `arrival_us`.
    pub fn on_probe_packet(&mut self, send_us: Us, arrival_us: Us, bytes: u32);
    /// Cabeçalho de um keyframe chegou (o quadro ainda vai chegar): evento de rajada (keyframe).
    pub fn on_keyframe_start(&mut self, arrival_us: Us);
    /// Keyframe terminou de chegar: `on_packet` + evento de rajada (keyframe) em `arrival_us`.
    pub fn on_keyframe_packet(&mut self, send_us: Us, arrival_us: Us, bytes: u32);
    /// Quadro descartado pelo emissor; `seen_us` = quando o buraco de `seq` foi visto (não quando
    /// venceu). Dentro da carência (avaliada em `seen_us`) é ignorado; fora dela entra na janela de
    /// descartes com o instante `seen_us`.
    pub fn on_frame_dropped(&mut self, seen_us: Us);
    /// Fim de um grupo de sondas (igual a hoje) + evento de rajada (sonda) em `now`.
    pub fn on_probe_cluster(&mut self, now: Us, kbps: u32);
}
```
Regra (`fn em_carencia(&self, t: Us) -> bool`), com `ultima_sonda` e `ultimo_keyframe` (instante do
último evento de cada tipo) e `ultimo = max(os dois)`:
- sem evento: `false`;
- `t − ultimo > 1 s` (saturando: `t < ultimo` conta como 0): `false`;
- `t − ultima_sonda ≤ 300 ms`: `true`;
- senão: `detector.fila_us() > 30 ms`.

Dentro da carência (em `update(now)` e em `state()`, como hoje com `em_carencia(now_chegada)`):
o ramo de descarte e o de sobrecarga (inclusive a fila persistente) não mexem em `alvo`,
`vazao_na_sobrecarga`, `ultima_sobrecarga` nem `fase_inicial`; `state()` devolve `Normal` no lugar
de `Overuse`. O resto do `Bwe` não muda.

`probe.rs` e `probe/planner.rs`:
```rust
/// Teto do degrau sobre a última medida que não saturou.
pub const FATOR_DEGRAU: f64 = 1.5;
/// Teto depois de uma medida que saturou.
pub const FATOR_APOS_SATURAR: f64 = 1.25;
/// Medida abaixo desta fração do alvo da rajada = saturou.
pub const FATOR_SATURADO: f64 = 0.85;
/// Espera entre um degrau e o seguinte (a partir do fim da rajada).
pub const DEGRAU_US: Us = 1_000_000;
/// Validade da medida do caminho.
pub const VALIDADE_MEDIDA_US: Us = 60_000_000;

pub struct ProbeBurst {
    pub cluster: u16,
    /// Taxa só das sondas (o vídeo já enviado não entra).
    pub kbps: u32,
    /// Taxa total que deve chegar durante a rajada (`alvo(e)` do contrato): `kbps + enviado`.
    pub total_kbps: u32,
    pub duration_us: Us,
}

impl ProbePlanner {
    /// Resultado medido de um grupo (por quem sonda, com o relatório do espectador). Grupo de
    /// `cluster` que não é de uma rajada recente deste planejador (últimas 8) é ignorado.
    /// Guarda `M = r.kbps`, `saturou = M < 0,85 × total_kbps` da rajada e o instante `now`.
    pub fn on_result(&mut self, now: Us, r: &ProbeResult);
    /// Última medida válida (até 60 s): (kbps, saturou).
    pub fn last_measure(&self, now: Us) -> Option<(u32, bool)>;
}
```
Mudanças no `poll` (o resto das regras de `m4b-pendencias.md` §5.1 continua):
- Alvo: `alvo_camada = need.burst_total_kbps()`; `teto = last_measure(now)` → `1,5 × M` ou
  `1,25 × M` (saturou), sem medida = sem teto; `alvo = min(alvo_camada, teto)` arredondado para
  cima; `sonda = alvo − enviado`; `sonda < 100` → como hoje (não sonda, `ultimo_fim = now`). A
  rajada guarda `(cluster, total_kbps = alvo, limitada = alvo < alvo_camada)`.
- Avaliação (o antigo passo 4), na ordem: `bandwidth_kbps ≥ required` → sucesso (`intervalo =
  5 s`); medida desta rajada já chegou, **não** saturou e `limitada` → **degrau**: a próxima
  rajada fica elegível em `fim + DEGRAU_US` (sem mexer em `intervalo`); medida desta rajada chegou
  e saturou → falha na hora (`intervalo = min(2 × intervalo, 60 s)`); `now ≥ fim + 1 s` → falha;
  senão espera.
- Mudar `need` volta `intervalo` a 5 s e `elegivel_em = now + 1 s` (como hoje); **não** apaga a
  medida.

`probe/meter.rs`:
```rust
impl ProbeMeter {
    /// Fecha o grupo aberto agora (quem sonda chama quando chega um relatório sem sondas).
    pub fn finish(&mut self) -> Option<ProbeResult>;
}
```

Testes (todos em `zap-rate`):
- `fila_us_sobe_e_desce` (detector: atraso +80 ms por 10 grupos → `fila_us ≥ 70 ms`; volta → `≤ 5 ms`).
- `keyframe_no_enlace_justo_nao_derruba` (`sim::Link` 4000 kbps, atraso base 20 ms, fila 150 000 B
  para não descartar): estimativa inicial 3600 (um `on_probe_cluster(0, 3790)`); vídeo de 2500 kbps
  a 30 fps em pacotes de 1200 B (quadro comum 10 417 B, keyframe 83 333 B em t = 0,5 s, 10,5 s,
  20,5 s, todos os pacotes do quadro enviados no mesmo instante); o primeiro pacote de cada
  keyframe chama `on_keyframe_start`, o último `on_keyframe_packet`; `update` a cada 50 ms por
  30 s → `state()` nunca `Overuse` e `estimate_kbps() ≥ 2639` o tempo todo.
- `congestionamento_real_ainda_derruba`: o mesmo, e em t = 12 s a capacidade cai para 2000 kbps →
  `estimate_kbps() < 2639` em até 2,5 s (a carência de um keyframe dura no máximo 1 s).
- `sonda_em_degrau_nao_derruba`: mesmo enlace com fila de 50 000 B (100 ms), estimativa 3600,
  rajada de 200 ms com 5625 kbps no total (3125 de sondas + 2500 de vídeo) → nenhum pacote perdido
  no `Link`, `state()` nunca `Overuse`, estimativa ≥ 2639 até 3 s depois.
- `descarte_visto_na_carencia_ignorado`: `on_frame_dropped(seen)` com `seen` dentro da carência,
  chamado 300 ms depois (fora dela) → não conta; com `seen` fora → conta.
- `carencia_fecha_em_1s_com_fila`: fila parada em 80 ms depois de uma sonda → `em_carencia` true
  até 1 s e false depois.
- `bwe_carencia_ignora_overuse_e_descarte` (existe): a parte "depois da carência" passa a usar um
  instante ≥ última sonda + 1 s (a fila do teste continua alta, então a carência agora vai até
  1 s); continua provando que descarte volta a contar. Nenhum outro teste existente muda.
- Planejador: `degrau_sobe_3750_5625_8437_12000` (passo de 100 ms, enviado 400 kbps e depois
  2500: necessidade 2500 desde 0 → rajada em ~1,0 s com `total_kbps` 3750; `on_result` 100 ms
  depois do fim com `kbps = total`; banda 3562 → sucesso; necessidade muda para 8000 em 4,5 s →
  rajadas em ~6,2 s, ~7,4 s, ~8,6 s com `total_kbps` 5625, 8438, 12000 (±1), cada uma seguida de
  `on_result` igual ao total e banda `0,95 ×` o total; depois da última, banda 11400 → sucesso e
  `interval_us() == 5 s`);
  `saturou_falha_e_teto_1_25` (medida 4000 contra alvo 5625 → falha, intervalo 10 s, próxima rajada
  com `total_kbps` 5000); `sem_medida_vai_direto_ao_alvo` (3750 na primeira);
  `medida_expira_em_60s`; `medida_de_cluster_estranho_ignorada`; `need_muda_mantem_medida`.
- Os testes atuais do planejador continuam passando sem mudança (sem `on_result` o comportamento
  é o de hoje).

Verificação: `cargo test -p zap-rate && cargo clippy -p zap-rate --all-targets -- -D warnings &&
cargo fmt -p zap-rate --check`.

### 5.2 `net` (`crates/zap-net`)

**Parte A (depende de `rate`).**

`media_tx.rs` / `media_tx/video.rs`:
```rust
/// Piso do prazo de um keyframe (midia.md "Prazo").
pub const PRAZO_KEYFRAME_MINIMO: Duration = Duration::from_millis(600);

impl MediaSender {
    /// `max(150 ms, 2 × RTT)` (quadro comum; igual a hoje).
    pub fn deadline(&self) -> Duration;
    /// `max(600 ms, 2 × RTT)`.
    pub fn keyframe_deadline(&self) -> Duration;
    /// Como hoje, mais: keyframe usa `max(deadline, keyframe_deadline())`; quadro comum conta
    /// `deadline` a partir do mais tarde entre a entrada dele e o fim (ACK completo ou reset) de
    /// todo keyframe desta conexão que entrou antes dele (se algum ainda está em voo quando o prazo
    /// venceria, espera ele terminar e conta de novo). Admissão: `fila` à frente sem os bytes de
    /// keyframes em voo.
    pub async fn send_video(&self, header: VideoHeader, payload: Bytes, deadline: Duration) -> Result<bool, NetError>;
}
```
Estado novo no `MediaSender` (privado): keyframes em voo `{id → (entrada, bytes)}`, os fins dos
últimos keyframes `(entrada, fim)` (até 8, mais novos que 5 s) e um `tokio::sync::Notify`
acordado a cada fim. Toda a lógica de "quando vence" fica numa função pura testável:
```rust
/// Instante em que o prazo de um quadro comum que entrou em `entrada` vence, dado o que se sabe
/// dos keyframes; `None` = ainda depende de um keyframe em voo que entrou antes dele.
pub(crate) fn vence_em(entrada: Instant, prazo: Duration, em_voo: &[Instant], fins: &[(Instant, Instant)]) -> Option<Instant>;
```

`media_rx.rs` / `media_rx/gap.rs` / `media_rx/frame.rs` / `report.rs`:
- `GapTracker::expire` devolve `Vec<(u16, u8, u32, u64)>` (o último é o instante em que o buraco
  foi visto); `ReportBuilder::on_lost(track, layer, seq, seen_us)` chama
  `bwe.on_frame_dropped(seen_us)`.
- `read_frame` passa a aceitar um aviso de cabeçalho:
  `pub async fn read_frame_with(recv, clock, on_header: impl FnOnce(&VideoHeader, u64)) -> Result<(VideoHeader, Bytes, u64), NetError>`
  (`read_frame` vira `read_frame_with(.., |_, _| {})`). `trata_stream`, quando o cabeçalho tem
  `is_keyframe()`, faz `reports.lock().on_keyframe_start(arrival_header_us)`.
- `ReportBuilder::on_video`: keyframe → `bwe.on_keyframe_packet(..)` no lugar de `on_packet`;
  `on_keyframe_start(arrival_us)` novo repassa a `Bwe::on_keyframe_start`.

`probe.rs`:
```rust
impl Prober {
    /// Relatório do espectador (relógio dele nos instantes): junta `report.probes` e
    /// `report.arrivals` por `chegada_us`, alimenta o `ProbeMeter` desta conexão
    /// (`on_probe`/`on_media`), fecha o grupo com `poll(maior chegada do relatório)` e, se o
    /// relatório não tem sondas e há grupo aberto, com `finish()`. Cada resultado vai para
    /// `ProbePlanner::on_result(now_us, &r)`.
    pub fn on_report(&self, now_us: u64, report: &zap_proto::Report);
}
```
`run_burst` não muda (usa `burst.kbps`).

Testes da parte A:
- `vence_em_*` (puros): sem keyframe → `entrada + prazo`; keyframe em voo que entrou antes →
  `None`; keyframe que terminou depois da entrada → `fim + prazo`; keyframe que entrou depois → não
  conta.
- `tests/prazo_keyframe.rs` (QUIC no loopback através do proxy de `tests/support`, 4000 kbps,
  fila de 100 ms, guarda o timer de 1 ms): keyframe de 83 333 B seguido de 15 quadros de 10 417 B a
  cada 33 ms → todos `Ok(true)` (nenhum reset), em 3 execuções; com o mesmo cenário e um quadro
  comum sozinho num caminho parado (proxy a 50 kbps), o reset sai entre 150 e 250 ms.
- `admissao_ignora_keyframe_em_voo` (unitário com `backlog` montado).
- `report.rs`: `keyframe_abre_carencia` (cabeçalho de keyframe + atraso crescente por 400 ms →
  `state` nunca `Overuse`); `perda_usa_instante_visto`.
- `tests/probe.rs` ganha `on_report_mede_o_grupo`: o relatório do receptor alimenta `on_report` do
  emissor e o planejador recebe `M` dentro de ±10% da taxa enviada.
- `impaired` (`rede_ruim_3mbps_fonte_6mbps` e os demais) continua passando 3 vezes seguidas, sem
  mudar faixas.

**Parte B (sem dependência; começa já).**

`online/refresh.rs` e `online/client.rs`:
```rust
/// Relógio da renovação do token: hora de parede e espera. Produção: `SystemTime` + `tokio::time::sleep`.
pub(crate) trait RenewClock: Send + Sync + 'static {
    fn now_unix(&self) -> u64;
    fn sleep(&self, d: Duration) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>;
}
pub(crate) struct SystemClock;
impl RenewClock for SystemClock { /* agora_unix() e tokio::time::sleep */ }

impl ServerClient {
    /// Como `login`, com o relógio da renovação dado (testes).
    pub(crate) async fn login_with_clock(base_url: &str, req: LoginRequest, clock: Arc<dyn RenewClock>) -> Result<Self, ApiError>;
}
```
`renovacao_automatica(fraco, clock)` usa `clock.now_unix()` e `clock.sleep(espera)`; a regra
(`renew_delay`, backoff 30 s → 300 s, 23 h sem data) não muda. `login` passa `SystemClock`.

Testes (sem `start_paused`, tempo real): relógio de teste cujo `sleep(d)` manda `d` por um
`mpsc` e espera um `oneshot` liberado pelo teste.
- `renovacao_automatica_dispara`: `login_with_clock` → o teste recebe `d1` (com timeout real de
  5 s) e confere `9 s ≤ d1 ≤ 10 s` e `n == 0`; libera; recebe `d2` (timeout 5 s): nesse ponto a
  renovação terminou, então `n == 1`, `token() == "t1"` e `9 s ≤ d2 ≤ 10 s`. Verifica o mesmo de
  antes (dispara no tempo certo, troca o token, uma vez só) e mais (o próximo agendamento).
- `tarefa_termina_ao_descartar_cliente`: recebe `d1`, solta o cliente, libera → o canal do relógio
  fecha (a tarefa terminou) em até 5 s e `n == 0`.
- Auditoria: todo teste do crate com `start_paused`/`time::pause()` que faz E/S de rede real
  (`handshake/server.rs:339`, `online/ws.rs:267` e `:278`) passa para tempo real ou relógio
  injetado do mesmo jeito; os que não fazem E/S ficam como estão. Relatar a lista no fechamento.

Aceite da parte B: `cargo test -p zap-net --lib online::client` 20 vezes seguidas sem falha
**com carga** (outro `cargo test -p zap-rate` rodando em paralelo) e o binário da biblioteca 20
vezes: `BIN=$(ls -t target/debug/deps/zap_net-*.exe | head -1); for i in $(seq 20); do $BIN online:: >/dev/null 2>&1 || echo falhou $i; done`.

Verificação da frente: `cargo test -p zap-net` (3 vezes o `--test impaired`), clippy e fmt do crate.

### 5.3 `servidor-teste` (`crates/zap-test-server`)

- `state.rs::ParticipantData` ganha `pub media_tx: Option<Arc<zap_net::media_tx::MediaSender>>`.
  `bridge.rs::registrar` cria **um** `Arc<MediaSender>` por conexão (prévia inclusive) e o mesmo
  `Arc` vai para o `Prober` (quando houver); a limpeza da conexão solta os dois.
- `Hub::plan_video` / `plan_audio` devolvem `Vec<Arc<MediaSender>>` no lugar de
  `Vec<quinn::Connection>`; `media::forward_video` e `forward_audio` usam esses senders
  (`sender_for` fica só para os testes ou sai).
- `bridge.rs::relatorio_do_espectador`: depois de soltar o lock do hub, se o participante tem
  `prober`, `prober.on_report(hub.clock.now_us(), r)`.
- Testes: unitário `plan_video_reusa_o_sender_da_conexao` (dois quadros seguidos → mesmo
  `Arc::ptr_eq`); `tests/bridge.rs::sondas_levam_espectador_a_camada_0` continua ≤ 20 s (com os
  degraus a previsão é ~12 s) e ganha a conferência de que houve ≥ 2 rajadas com `total_kbps`
  crescente (`Prober` expõe a contagem só em teste, ou o teste lê o log).

Verificação: `cargo test -p zap-test-server` (3 vezes o `tests/bridge.rs`), clippy e fmt do crate.

### 5.4 `core` (`crates/zap-core`)

**a) Publicador mede as sondas.** `publish/media_serve.rs`, no `ControlMsg::Report(r)` do
espectador (linha ~61): `prober.on_report(sender.clock.now_us(), &r)` antes do que já é feito.
Conferir que cada conexão de espectador usa um único `Arc<MediaSender>` para vídeo, áudio e sondas
(hoje sim) e que o envio do publicador para a ponte (`send_loop.rs`) usa o mesmo sender por
conexão (o prazo novo depende disso).

**b) Testes.** Nenhum teste de aceite muda. Novo unitário: `relatorio_do_espectador_alimenta_o_prober`.

**c) Aceite 2: 5 de 5.** `cargo test -p zap-core --test online_aceite_2 -- --nocapture` 5 vezes
seguidas, com a máquina sem outro cargo rodando, e mais 2 vezes com `cargo build -p zap-media`
rodando em paralelo (carga). Previsão (relógio desde o `watch`): rajada 2→1 de A e B em ~1 s (3750,
sem medida); A e B na 1 em ~4,5 s; rajadas 1→0 em ~6,2 s (5625): B mede ~5600 (degrau), A mede
~4000 (saturou, falha, próxima em ~16,4 s com 5000); B em ~7,4 s (8437) e ~8,6 s (12000), sucesso;
B na 0 em ~12 s. Keyframes de A (troca de B para a 1, periódico a cada 10 s) não derrubam A.

**d) Se falhar.** Rodar a receita de diagnóstico: copiar o scratchpad `diag_aceite2.rs` para
`crates/zap-core/tests/zz_diag_a2_tmp.rs`, `DIAG_LOG="zap_net::media_tx=debug,zap_net::probe=debug,zap_test_server=debug" cargo test -p zap-core --test zz_diag_a2_tmp -- --nocapture > <scratchpad>/diag_a2_N.txt`,
e **apagar o arquivo depois**. Ler: (1) resets em `seq` 0..3 depois de cada troca → o prazo do
keyframe não está valendo na ponte (5.2/5.3); (2) `est=Overuse` de A até 1 s depois de uma troca
ou rajada → a carência não está fechando certo (5.1); (3) descartes no proxy > 0 numa rajada 1→0
depois da primeira → o teto não está valendo (5.1/5.2 `on_report`); (4) `lost` do QUIC subindo a
cada keyframe e A caindo mesmo sem Overuse → abrir a reserva de pacing (seção 7). Não afrouxar o
teste.

**e) Outros aceites.** Aceites 1, 3, 4 (3 funções), prévia, reconexão e `aceite_camadas_local`
3 vezes seguidas cada.

**f) Fechamento do M4** (`core-online-27`, depois de todas as frentes): ver seção 9.

### 5.5 `gpu` (`crates/zap-gpu`)

`timing.rs` (novo, `pub mod timing;` e reexport de `GpuTimer` em `lib.rs`; `stub.rs` ganha a
versão vazia):
```rust
/// Tempo de GPU de um trecho de comandos no contexto imediato, por `D3D11_QUERY_TIMESTAMP_DISJOINT`
/// + 2 × `D3D11_QUERY_TIMESTAMP`, sem bloquear: anel de 8 conjuntos, resultados lidos com
/// `GetData(.., D3D11_ASYNC_GETDATA_DONOTFLUSH)` alguns quadros depois.
pub struct GpuTimer { /* anel de (disjunta, início, fim, em_uso) */ }
impl GpuTimer {
    pub fn new(dev: &GpuDevice) -> Result<Self, GpuError>;
    /// Abre uma medição (`Begin` da disjunta + `End` do início). `None` se o anel está cheio.
    pub fn begin(&mut self, dev: &GpuDevice) -> Option<usize>;
    pub fn end(&mut self, dev: &GpuDevice, slot: usize);
    /// Medições prontas em µs (descarta as marcadas `Disjoint`); nunca bloqueia.
    pub fn collect(&mut self, dev: &GpuDevice) -> Vec<f64>;
}
```

`device.rs`:
```rust
/// Lock explícito do dispositivo (`ID3D11Multithread::Enter`/`Leave`; reentrante, então chamadas
/// D3D11 dentro dele não travam). Serve para medir quanto se espera pelo lock.
pub struct DeviceLock<'a> { /* &'a ID3D11Multithread */ }
impl Drop for DeviceLock<'_> { fn drop(&mut self); }
impl GpuDevice {
    /// Pega o lock e devolve quanto esperou por ele.
    pub fn lock(&self) -> (DeviceLock<'_>, std::time::Duration);
}
```

`converter.rs`:
```rust
#[derive(Debug, Clone, Copy, Default)]
pub struct ConvertTiming {
    /// Espera pelo lock do dispositivo (µs).
    pub lock_wait_us: f64,
    /// CPU dentro do lock: visões + `VideoProcessorBlt` (µs).
    pub cpu_us: f64,
}
impl Converter {
    /// Liga a medição de GPU do `VideoProcessorBlt` (cria um `GpuTimer`).
    pub fn enable_gpu_timing(&mut self, dev: &GpuDevice) -> Result<(), GpuError>;
    /// Tempos da última `convert` (CPU e lock).
    pub fn last_timing(&self) -> ConvertTiming;
    /// Tempos de GPU (µs) prontos desde a última chamada.
    pub fn take_gpu_us(&mut self, dev: &GpuDevice) -> Vec<f64>;
}
```
`convert` faz: `dev.lock()` (mede a espera) → visões do cache → Blt (entre `begin`/`end` do timer
se ligado) → solta. Cache de visões: até 8 de entrada (chave = ponteiro cru do `ID3D11Texture2D`,
guardando um clone COM para o ponteiro não ser reaproveitado) e até 8 de saída; cheio → esvazia e
recomeça. Motivo: tirar duas criações de objeto do driver por quadro de dentro do lock.

Testes: `gpu_timer_mede_blt` (1920×1080 → 1280×720, 20 conversões, depois de `Flush` e 50 ms
`collect` devolve ≥ 10 amostras entre 1 µs e 20 ms); `lock_sem_concorrencia_espera_pouco`
(100 × `lock()` numa thread só: p95 da espera ≤ 100 µs); `cache_de_visoes_nao_cresce` (convert
alternando 3 origens e 4 destinos 100 vezes: o cache fica ≤ 8 + 8); os testes atuais passam.

Verificação: `cargo test -p zap-gpu`, clippy e fmt do crate.

### 5.6 `captura` (`crates/zap-capture`)

**a) Espera fora do lock.** `dxgi_dup.rs`: `Duplication` guarda o `IDXGIOutput` achado em
`achar_saida` e ganha `pub(crate) fn wait_vblank(&self) -> bool` (`WaitForVBlank`; `false` em
erro). `dxgi.rs`: `ESPERA_MS` some; o laço faz `acquire(0)`; em `Timeout` chama `wait_vblank()`
e, se `false`, `std::thread::sleep(2 ms)`; o resto (reentrega por `StaleTimer`, `Lost`, `Frame`)
fica igual. O comentário do arquivo explica o lock (link para a decisão em `arquitetura-v3.md` §2).

**b) Teste de contenção** (`tests/lock_dispositivo.rs`, pula sem GPU/monitor):
`captura_nao_segura_o_dispositivo` — `capture_monitor` do monitor 1 num `GpuDevice` com a tela
parada (sem janela animada), e numa thread ao lado, no mesmo dispositivo, 200 iterações de
`CopyResource` 64×64 + `End(evento)` + `GetData` em laço (com `dev.lock()` do `zap-gpu` medindo a
espera, se a frente `gpu` já entregou; senão o tempo de parede de cada iteração): **p95 ≤ 5 ms**.
Antes da correção a previsão é p95 ≥ 50 ms; rodar uma vez antes de corrigir e anotar o número no
relato (prova a causa).

**c) Movimento** (`src/movimento.rs`, `pub mod movimento` só no Windows; `stub.rs` sem nada):
```rust
/// Janela pequena (320×200), sempre no topo, sem foco e fora da barra de tarefas, no canto
/// inferior direito de um monitor (x + largura − 340, y + altura − 240), que muda de cor a cada
/// ~8 ms: dá movimento para a duplicação de tela na bancada e nos testes. Fecha no `Drop`.
pub struct JanelaAnimada { /* thread "zap-anim", HWND, AtomicBool, contador */ }
impl JanelaAnimada {
    /// Abre e espera a primeira pintura (até 2 s).
    pub fn abrir(monitor: &MonitorInfo) -> Result<Self, CaptureError>;
    /// Pinturas feitas até agora.
    pub fn quadros(&self) -> u64;
}
impl Drop for JanelaAnimada { /* PostMessageW(WM_CLOSE) + join; sem pânico se a janela já fechou */ }
```
Thread `zap-anim`: `RegisterClassW` (uma vez por processo), `CreateWindowExW(WS_EX_TOPMOST |
WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE, .., WS_POPUP | WS_VISIBLE)`, laço `PeekMessageW` +
`InvalidateRect`/`UpdateWindow` + `std::thread::sleep(8 ms)` (o `sleep` da `std` usa timer de alta
resolução), `WM_PAINT` com `FillRect` numa cor que gira por matiz a cada pintura. Features do
`windows` que faltarem (`Win32_UI_WindowsAndMessaging`, `Win32_Graphics_Gdi`,
`Win32_System_LibraryLoader`) entram no `Cargo.toml` do crate (sem crate novo). Uma thread a mais
só enquanto a janela existe.

Teste: `janela_animada_gera_quadros` (com a janela aberta no monitor 1, `capture_monitor` entrega
≥ 30 quadros com `new_content` em 1 s; pula sem monitor).

Aceite: os testes do crate passam; o binário da biblioteca 30 vezes seguidas sem queda (como no
M4b).

### 5.7 `encode` (`crates/zap-encode`)

**a) Opções antes dos tipos.** `hw.rs::open` e `sw.rs::open`: `codec_api::apply_settings` antes
de `SetOutputType`; depois de `SetInputType`, reaplica só as opções que o MFT recusou antes (alguns
MFTs só aceitam com tipo definido). O mesmo em `hw.rs::reabrir`.

**b) Leitura de volta completa.**
```rust
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct CodecReadback {
    pub rate_control: Option<u32>,
    pub mean_bitrate: Option<u32>,
    pub buffer_size: Option<u32>,
    pub max_bitrate: Option<u32>,
    pub low_latency: Option<bool>,
    pub b_pictures: Option<u32>,
    pub gop: Option<u32>,
    pub quality_vs_speed: Option<u32>,
    pub min_qp: Option<u32>,
    pub max_qp: Option<u32>,
}
pub(crate) fn readback(api: &ICodecAPI) -> CodecReadback;   // log_readback passa a logar isto
```
Chamado em `hw.rs` e `sw.rs` depois de abrir (`tracing::info!`).

**c) Controle de taxa do MFT de software.** Rodar `tamanho_medio_nao_chave_software` com cada
configuração abaixo, nesta ordem, e anotar média dos não-chave e `CodecReadback` numa tabela no
relato (e na seção 8 deste plano). A primeira que der 8333..24999 vira a do software em
`apply_settings` (por `EncoderInfo::hardware == false`):
1. só (a) (opções antes dos tipos: CBR, `MeanBitRate = kbps × 1000`, `BufferSize = kbps × 1000 / 4`,
   `LowLatency`, 0 B-frames);
2. (1) + `CODECAPI_AVEncCommonQualityVsSpeed = 0` + `CODECAPI_AVEncVideoMaxQP = 51` +
   `CODECAPI_AVEncVideoMinQP = 0`;
3. (2) com `BufferSize = kbps × 1000 / 8` (125 ms);
4. (2) com `eAVEncCommonRateControlMode_PeakConstrainedVBR`, `MaxBitRate = kbps × 1000`,
   `MeanBitRate = 0,9 × kbps × 1000`.

Se nenhuma servir (o MFT não segura ruído puro: anotar a menor média obtida e o `max_qp` lido):
- `tamanho_medio_nao_chave_software` passa a usar `support::frame_realista` (novo: gradiente
  horizontal que anda 4 px por quadro + 40 linhas de "texto" — retângulos de 2×12 px em padrão
  pseudoaleatório fixo — rolando 2 px por quadro), com a **mesma** faixa ±50%;
- novo `ruido_software_dentro_do_limite_documentado`: ruído, `media ≤ 1,2 × menor média medida`,
  com o número e o motivo num comentário do teste e na seção 8 deste plano;
- o teste de hardware com ruído (`tamanho_medio_nao_chave`) não muda.

**d) Testes novos.**
- `software_aceita_controle_de_taxa`: abre o software e confere `readback().rate_control ==
  Some(o modo escolhido)` e `mean_bitrate == Some(4_000_000)` (ou o valor do item escolhido).
- `tests/latencia.rs::saida_sai_antes_do_proximo_quadro` (pula sem hardware): 20 quadros de ruído
  1280×720 com 100 ms entre eles; cada saída chega ≤ 40 ms depois do seu `submit` (um MFT que segura
  o quadro até o próximo entregaria ~100 ms depois).

Aceite: `cargo test -p zap-encode --test windows` e `--test latencia` 3 vezes seguidas; `cargo test
-p zap-encode`; clippy e fmt.

### 5.8 `media` (`crates/zap-media`)

**a) Estatísticas.** `types.rs`:
```rust
pub struct LayerStats {
    /* como hoje + */
    /// Tempo de GPU do `VideoProcessorBlt` (consulta de tempo).
    pub convert_gpu: StageStats,
    /// Espera pelo lock do dispositivo antes da conversão.
    pub lock_wait: StageStats,
}
pub struct VideoStats {
    /* como hoje + */
    /// Quadros com imagem nova entregues pela captura no último segundo.
    pub capture_new_fps: f32,
}
```
`win_video.rs`: `criar_pipe` liga `conv.enable_gpu_timing(dev)` (falha só loga); `processar`
guarda `last_timing()` e `take_gpu_us()` nas amostras; `executar` conta quadros com
`new_content`. A política de descarte (slot "o mais novo vence", fila de 2 do codificador) não
muda; os números dela entram na tabela da bancada.

**b) Bancada** (`examples/bancada.rs`):
- `--animar` (padrão) / `--sem-animar`: abre `zap_capture::movimento::JanelaAnimada` no monitor
  medido antes do pipeline e fecha no fim (também em erro: guarda local);
- `--aquecimento S` (padrão 2): roda S segundos antes de contar quadros (a GPU sai do estado de
  economia; as amostras por etapa são janelas de 120, então o que sai é do trecho medido);
- cabeçalho: `animação: sim/não`; colunas novas `conv. GPU média/p95` e `espera lock p95`; linha
  `quadros novos da captura: X/s`;
- aviso quando `capture_new_fps < 30` com animação ou `< 5` sem: `AVISO: tela quase parada (X
  quadros novos/s): os tempos medem espera, não trabalho`;
- testes do `parse_args` para as flags novas.

**c) Testes `win_video`.** `iniciar()` abre `JanelaAnimada` no monitor 1 e a guarda junto com o
pipeline; se não abrir, pula com `eprintln!("sem janela animada ({e}): teste ignorado")`. Os 4
testes passam a ter movimento. Aceite: `cargo test -p zap-media` 10 vezes seguidas sem falha nem
queda, e 3 vezes com `cargo build -p zap-net` rodando ao lado.

**d) Medida da etapa A** (depois de `gpu`, `captura` e `encode`): `cargo build --release -p
zap-media --example bancada`, máquina ociosa, 3 execuções de `target/release/examples/bancada.exe
--monitor 1 --preset 1080p60 --segundos 10` com `nvidia-smi --query-gpu=clocks.gr,clocks.video,utilization.gpu,utilization.encoder --format=csv -lms 500`
ao lado (saídas no scratchpad). Anotar a mediana na seção 8.

**e) Portão da etapa B.** Abre se, na mediana da etapa A, `espera lock p95 > 1 ms` **ou**
`codificação p95 da camada 0 > 6 ms` com `conv. GPU p95 ≤ 2 ms`. Fechado: a meta se decide na
etapa A (se ainda não atingida, escrever qual etapa domina e o que a bancada mostra). Aberto:
- `gpu`: `Texture::new_shared(dev, w, h, fmt) -> Result<Texture, GpuError>`
  (`D3D11_RESOURCE_MISC_SHARED_NTHANDLE | D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX`),
  `Texture::open_shared(dev: &GpuDevice, t: &Texture) -> Result<Texture, GpuError>` e
  `KeyedMutexGuard` (`AcquireSync(chave, 100 ms)`/`ReleaseSync`), `GpuDevice::create_for_adapter`
  já existe para o dispositivo novo;
- `encode`: `H264Encoder::new_dedicated(adapter_luid, cfg, out)` cria o próprio `GpuDevice` e
  aceita `submit` de textura compartilhada (abre uma vez por textura do anel, guarda em cache;
  `AcquireSync(1)` antes do `ProcessInput`, `ReleaseSync(0)` quando o MFT devolve a amostra);
- `media`: o anel NV12 de cada camada vira de texturas compartilhadas; o conversor faz
  `AcquireSync(0)`/`ReleaseSync(1)`.
  Motivo: cada MFT trava só o próprio dispositivo. Custo: um dispositivo por camada ligada e a
  sincronização por textura. Medir de novo como em (d).

**f) Medição final** (fechamento, seção 9): 5 execuções release de 10 s, máquina ociosa, animação
ligada; mediana na seção 8 e em `m2-midia-windows.md` ("Estado do fechamento").

## 6. Testes de aceite da rodada

| # | Teste | Esperado |
|---|---|---|
| 1 | `zap-core --test online_aceite_2` | 5 de 5 sem carga e 2 de 2 com carga; A=1, B=0, C=2 e P com {0,1,2} em ≤ 30 s, estável 3 s (teste sem mudança) |
| 2 | `zap-core` aceites 1, 3, 4, prévia, reconexão, `aceite_camadas_local` | 3 vezes seguidas cada |
| 3 | `zap-rate` | Testes da seção 5.1; os antigos passam (um ajustado como descrito) |
| 4 | `zap-net --test prazo_keyframe` (novo) | Keyframe de 83 KB + 15 quadros num enlace de 4 Mbps/100 ms: nenhum reset, 3 vezes |
| 5 | `zap-net --test impaired` e `--test probe` | Passam 3 vezes seguidas, faixas sem mudança |
| 6 | `zap-net --lib online::client` | 20 vezes sem falha com carga |
| 7 | `zap-test-server --test bridge` | `sondas_levam_espectador_a_camada_0` ≤ 20 s com ≥ 2 degraus; antigos passam |
| 8 | `zap-capture --test lock_dispositivo` | p95 da espera ≤ 5 ms com a tela parada |
| 9 | `zap-encode --test windows` e `--test latencia` | Todos passam 3 vezes; software com 8333..24999 B (ruído ou, pela regra de 5.7 c, conteúdo realista + ruído no limite documentado) |
| 10 | `zap-media` | 10 vezes sem falha; testes `win_video` com movimento |
| 11 | Bancada release, animada, 5 × 10 s | Mediana: camada 0 ≥ 57 fps, p95 somado ≤ 8 ms (meta); descartes ≤ 1% dos quadros; nenhuma queda |
| 12 | Workspace | `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` sem falha, `cargo check --workspace --all-targets` |

O item 11 é meta, não bloqueio: se não for atingida depois da etapa B (ou com o portão fechado),
o fechamento registra o número, a etapa que domina e a próxima hipótese.

## 7. Riscos

| Risco | Mitigação |
|---|---|
| O keyframe ainda perde pacotes no gargalo (QUIC em partida lenta manda ~66 KB de uma vez numa fila de 50 KB) e a retransmissão leva o keyframe além de 600 ms | Diagnóstico 5.4 d mostra; reserva: ritmo do keyframe no `MediaSender` (escrever em pedaços de 16 KB a `2 × kbps(camada)` com o timer de 1 ms), mudança só em `zap-net` e uma linha em `midia.md` |
| Carência esconde congestionamento real que começa junto de um keyframe | No máximo 1 s por evento; keyframes da mesma camada no máximo 1 por segundo; teste `congestionamento_real_ainda_derruba` |
| Muitos keyframes (espectador ruim pedindo) deixam o estimador cego boa parte do tempo | A carência de keyframe não tem mínimo: fecha assim que a fila escoa; `keyframe.pedir` limitado a 1/s por camada (ponte) |
| Degraus deixam a subida para a camada 0 mais lenta (3 rajadas) | 1 s entre degraus: ~3,6 s a mais; aceite 2 e `aceite_camadas_local` têm folga (previsão ~12 s contra 30 s e 20 s) |
| Relatório sem `chegadas` (outro app, versão futura) deixa a medida errada | A medida também precisa de ≥ 5 sondas e ≥ 100 ms; sem medida o planejador cai no comportamento de hoje (sem teto) |
| `AcquireNextFrame(0)` + `WaitForVBlank` perde quadro ou acorda demais em monitor de alta taxa | `WaitForVBlank` acorda uma vez por atualização (164/s aqui); o teste da janela animada exige ≥ 30 quadros novos/s |
| `WaitForVBlank` também segura algum lock | O teste 5.6 b mede; reserva: dispositivo próprio para a captura com textura compartilhada (mesma API da etapa B) |
| Etapa B (dispositivo por codificador) com driver que não abre textura compartilhada NV12 | Testar `open_shared` em NV12 antes de ligar; sem suporte, fica o dispositivo único e a meta é registrada como não atingida |
| Mais de 3 sessões NVENC (tela + câmera, 5 camadas) em driver antigo | Fora desta rodada; a abertura já cai para o software se o MFT recusar (`encoder.rs::worker`) |
| MFT de software não segura ruído | Regra de 5.7 c: conteúdo realista com a mesma faixa + ruído com limite medido e documentado |
| Testes de tempo da GPU variam com o relógio da GPU | Aquecimento de 2 s na bancada; os testes de `zap-gpu` só conferem faixas largas |

## 8. Números

### 8.1 Bancada (1080p60, monitor 1, release, timer de 1 ms, NVIDIA H.264 Encoder MFT)

Antes (m4b-pendencias.md §8, janela animada a ~60 Hz): camada 0 25,9 fps 1083 kbps, conversão
12,36 / 30,28 ms, codificação 21,01 / 41,48 ms; camada 1 22,8 fps; camada 2 14,5 fps; descartes
captura 134, codificação 149; p95 somado 71,8 ms.

| Medida | Camada | fps | kbps | conversão CPU média/p95 | conv. GPU média/p95 | espera lock p95 | codificação média/p95 | p95 somado | descartes cap./cod. |
|---|---|---|---|---|---|---|---|---|---|
| Etapa A (mediana de 3) | 0 | 48,8 | 6469 | 0,05 / 0,07 | 0,00 / 0,00 | 0,059 ms | 11,51 / 25,26 | 25,3 ms | 7 / 0 |
| | 1 | 30,0 | 2492 | 0,01 / 0,01 | 0,00 / 0,00 | 0,000 ms | 12,30 / 27,74 | - | - |
| | 2 | 15,0 | 388 | 0,00 / 0,01 | 0,00 / 0,00 | 0,000 ms | 12,91 / 27,57 | - | - |
| Etapa B, se houver | - | não houve: `open_shared`/`new_dedicated` não existem no código | | | | | | | |
| Final (mediana de 5) | 0 | 48,5 | 2993 | 0,05 / 0,07 | 0,00 / 0,00 | 0,058 ms | 12,03 / 23,47 | 23,6 ms | 8 / 0 |
| | 1 | 30,0 | 1554 | 0,00 / 0,01 | 0,00 / 0,00 | 0,000 ms | 12,32 / 23,31 | - | - |
| | 2 | 15,0 | 353 | 0,00 / 0,01 | 0,00 / 0,00 | 0,000 ms | 12,52 / 23,68 | - | - |

Final: 5 execuções, máquina ociosa, `bancada.exe --monitor 1 --preset 1080p60 --segundos 10` (animação interna), nvidia-smi ao lado. Meta NÃO atingida (48,5 fps < 57; p95 somado 23,6 ms > 8; descartes de captura ~8 em ~480 quadros, 1,7% > 1%, de codificação 0). Etapa dominante: codificação (p95 ~23 ms em todas as camadas). Medidas brutas: scratchpad `bancada_final_1..5.txt` e `nvsmi_final_1..5.csv`.

Portão da etapa B: ABERTO. Na mediana de 3 execuções, a espera pelo lock p95 é 0,059 ms (<= 1 ms, não dispara), mas a codificação p95 da camada 0 é 25,26 ms (> 6 ms) com conv. GPU p95 de 0,00 ms (<= 2 ms): o tempo está no codificador, não na conversão nem no lock.
A meta (p95 somado <= 8 ms, camada 0 >= 57 fps) não foi atingida na etapa A: 25,3 ms e 48,8 fps. A codificação domina (média ~11,5 ms nas três camadas, quase igual mesmo em 640x360, o que sugere custo do MFT/driver compartilhado entre as sessões e não de resolução); captura (0,01 ms) e conversão (< 0,1 ms) são desprezíveis. Nenhum aviso de tela parada nas 3 execuções; descartes de captura 7/7/27, de codificação 0.
Frentes novas para o orquestrador criar, nesta ordem: (1) zap-gpu: texturas compartilhadas NV12 por codificador (`open_shared`); (2) zap-encode: `new_dedicated` (dispositivo próprio por codificador); (3) zap-media: ligar o dispositivo dedicado por camada.
Medidas brutas: scratchpad `bancada_A_1..3.txt` e `nvsmi_A_1..3.csv`.

### 8.2 MFT de software (5.7 c)

Números registrados pela frente encode (ver também encode-04).

Ruído 8x8, 1280x720, 4000 kbps, 30 fps; faixa de aceite 8333..24999 bytes. Média dos não-chave (B)
a partir do quadro 10.

| Configuração | média não-chave (B), ruído | média não-chave (B), realista | `CodecReadback` |
|---|---|---|---|
| 1 | 42199 | 18209 | rate_control 0, mean_bitrate 4000000, buffer_size 1000000, max_bitrate 0, max_qp 51, gop 300, B 0, low_latency sim |
| 2 | 42199 | 18209 | rate_control 0, mean_bitrate 4000000, buffer_size 1000000, max_bitrate 0, max_qp 51, gop 300, B 0, low_latency sim |
| 3 | 49652 | 15540 | rate_control 0, mean_bitrate 4000000, buffer_size 500000, max_bitrate 0, max_qp 51, gop 300, B 0, low_latency sim |
| 4 | 48279 | 14911 | rate_control 1, mean_bitrate 3600000, buffer_size 1000000, max_bitrate 4000000, max_qp 51, gop 300, B 0, low_latency sim |

### 8.3 Codificador de software

Nenhum dos 4 perfis fica na faixa com ruído puro (todos ~2,5x acima do orçamento de 16666 B/quadro),
mesmo com CBR, buffer de 1 s e max_qp lido = 51. O MFT de software não segura ruído puro no
orçamento. Decisão: `PERFIL_SW_ESCOLHIDO` fica em Sw1 (menor média com ruído, 42199 B). O teste
`tamanho_medio_nao_chave_software` passou a usar conteúdo realista (`frame_realista`), mesma faixa
±50% (Sw1: 18209 B, dentro da faixa), e `ruido_software_dentro_do_limite_documentado` guarda o
ruído em no máximo 1,2 x 42199 B como limite de regressão. O teste de hardware
`tamanho_medio_nao_chave` não mudou (ruído, 22577 B).

## 9. Fechamento

Na ordem, com a máquina ociosa (`tasklist | findstr /i "cargo rustc"` vazio antes de cada medida
de desempenho):
1. `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`;
   `cargo check --workspace --all-targets`; `cargo test --workspace --no-fail-fast` (relatar cada
   falha com causa).
2. Aceites da seção 6 itens 1 e 2 (frente `core`); itens 4–10 já rodados pelas frentes, repetir
   uma vez cada.
3. Bancada final (5.8 f), seção 8 preenchida.
4. Documentos: `docs/planos/m4-tarefas-pendentes.json` (`core-online-22` e `-27` com `"estado":
   "concluida"` e uma `"nota"` de uma linha, se o aceite 2 deu 5 de 5); `m4-online.md` e
   `m2-midia-windows.md` ("Estado do fechamento"); `docs/ESTADO.md` ("Atualização do M4": o que
   fechou e o que ficou; "O que o servidor de produção precisa mudar": sondagem em degraus com
   medida pelo relatório, prazo do keyframe e extensão, um estado de envio por conexão).
5. Conferir que nenhum arquivo de diagnóstico temporário ficou em `crates/` (`git status`).

## 10. Estado do fechamento (rodada 3, revisor, 2026-11-04)

Tarefas não executadas pelo orquestrador (dependência falhou em `net-03`): `net-13`,
`servidor-teste-04`, `core-02`, `core-03`, `core-05`. `net-03` (teste `prazo_keyframe`) falha por
defeito de produto em `zap-net` (ver abaixo); o plano precisa decidir entre a admissão com RTT de
base, o ritmo do keyframe (risco da seção 7) ou rever o cenário.

Portão: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` e
`cargo check --workspace --all-targets` passam. `npm run check` passa; `npx vitest run`: 83
arquivos, 666 testes passam.

`cargo test --workspace --no-fail-fast`: 3 alvos falharam.
1. `zap-net --test prazo_keyframe keyframe_de_83kb_com_quadros_atras_nao_reseta`: descartados
   `[(0,[5,8,9]),(1,[5,8,9]),(2,[5,8,9])]` (determinístico). Causa: perda no gargalo na partida
   lenta do QUIC reseta o quadro 5 pelo prazo contado do fim do keyframe, e a admissão (srtt já
   inflado pela fila, somado a `fila/taxa`) descarta os quadros 8 e 9. Os outros 6 testes do arquivo passam.
2. `zap-core --test online_aceite_2`: falha com `camadas oscilaram: A=Some(2) B=Some(0) C=Some(2)`
   (A volta à camada 2). Consequência de 1: as frentes `net-13`, `servidor-teste-04` e `core-02/03/05`
   (ponte com um `MediaSender` por conexão, sondagem em degraus, uso no aceite) não rodaram.
3. `zap-core --lib publish::alloc_loop::tests::tick_com_espectador_pede_camada_e_pipeline_liga`:
   falhou uma vez no workspace inteiro ("camada 0 ou 1 deveria estar ligada"), 0 de 5 falhas
   isolado: instável sob carga (o teste espera só 1 s para o pipeline ligar uma camada).

Repetições (`cargo test -p zap-core --test ...`): `online_aceite_2` **falhou 2 de 5** (mais a
falha do workspace: 3 de 6 no total); `online_aceite_1`, `online_aceite_3`, `online_aceite_4`,
`online_previa`, `online_reconexao` e `aceite_camadas_local` falharam 0 de 3 cada.

Bancada release, 5 execuções, máquina ociosa, `--monitor 1 --preset 1080p60 --segundos 10`
(animação interna, aquecimento 2 s). Medianas por camada (brutos no scratchpad `bancada_r3_1..5.txt`):

| Camada | fps | kbps | conversão CPU média/p95 (ms) | conv. GPU | espera lock p95 (ms) | codificação média/p95 (ms) |
|---|---|---|---|---|---|---|
| 0 (1080p) | 60,0 | 2214 | 0,08 / 0,18 | 0,00 / 0,00 | 0,173 | 3,69 / 6,23 |
| 1 (720p) | 30,0 | 1506 | 0,02 / 0,11 | 0,00 / 0,00 | 0,098 | 4,22 / 7,10 |
| 2 (360p) | 15,0 | 323 | 0,03 / 0,10 | 0,00 / 0,00 | 0,091 | 4,36 / 7,23 |

p95 somado da camada 0 (captura+conversão+codificação): mediana 6,5 ms (execuções 6,5 / 6,3 / 6,9
/ 2,2 / 9,1). **Meta de 8 ms atingida na mediana** (4 de 5 execuções; a 5ª deu 9,1 ms) e camada 0 em 60 fps
(>= 57). Descartes de captura 60 / 19 / 27 / 20 / 32 (mediana 27), de codificação 0; sem referência
exata de quadros esperados, o limite de 1% não foi confirmado. Etapa dominante: codificação (3,7 ms
média, p95 6,2 ms), com conversão e lock desprezíveis. Portão da etapa B (dispositivo por codificador)
fechado nesta medida. Os números "Final" da seção 8.1 (48,5 fps, 23,6 ms) são de uma medida
anterior de outra frente e ficam superados por esta tabela.

## 11. Rodada 3b: correções do orquestrador (2026-11-04)

Depois do fechamento acima, o orquestrador fez direto (trabalho pequeno, sem a hierarquia) o que
`net-03` e o aceite 2 pediam, e rodou ele mesmo as verificações de `net-13`, `servidor-teste-04` e
`core-02/03/05`.

**a) `prazo_keyframe` (`zap-net/src/media_tx/video.rs`).** As duas causas do relatório de `net-03`:
- admissão: a espera estimada passou a ser `(fila + quadro) / (cwnd / RTT) + RTT de base`. O RTT de
  base é o menor RTT suavizado visto na conexão (`MediaSender::rtt_base_us`). O RTT atual já contém
  a fila, que `fila / taxa` também conta: somá-lo contava a mesma espera duas vezes e descartava os
  quadros 8 e 9;
- prazo: o piso de `2 × RTT` usa o RTT **atual** na hora de decidir o reset, não o da entrada na
  fila. Os quadros 4 e 5, que esperavam a retransmissão de pacotes perdidos no gargalo, eram
  resetados por 1 a 4 ms com o RTT velho (~20 ms). Com o RTT de agora (~110 ms) o prazo deles cobre
  a retransmissão. Escrito em `docs/api/midia.md` ("Prazo"). O teste ficou como `net-03` o deixou:
  3 de 3.

**b) Aceite 2: o que ainda derrubava A.** O diagnóstico da receita 5.4 d, com o estado do `Bwe` de A
impresso a cada relatório, mostrou duas causas. As saídas estão no scratchpad: `diag_a2_v3_1.txt` e
`diag_a2_v3_2.txt`.
1. **Sobrecarga que sobra do keyframe.** No keyframe periódico (10 s) e em cada nova subida para a
   1, o detector de A entra em sobrecarga dentro da carência, por fila persistente e fila de 58 a
   92 ms. A fila escoa: com menos de 30 ms a carência fecha. Mas a tendência do detector ainda diz
   `Overuse` por mais um ou dois relatórios. Esse resto cortava a estimativa para
   `0,85 × vazão ≈ 3100`, e o `estado = sobrecarga` fazia a ponte descer A na hora (histerese:
   sobrecarga desce sem esperar). Correção (`zap-rate/src/bwe.rs`, `ignora_overuse`): sobrecarga
   que **começou** dentro da carência continua sendo da rajada até 1 s do último evento e 1 s do
   início dela. O contrato está em `docs/api/balanceamento.md` §4. O limite de 1 s mantém o risco
   da seção 7: sobrecarga real que começa junto com a rajada ainda aparece, no máximo 1 s depois.
   Testes: `overuse_nascido_na_carencia_ignorado_ate_1s`, `overuse_nascido_fora_da_carencia_conta`
   e `overuse_longo_conta_mesmo_nascido_na_carencia`.
2. **Volta para uma camada antiga virava centenas de descartes.** Quando A voltava para a camada 2
   depois de ~20 s na 1, o `GapTracker` via o salto de `seq` da camada 2 (~300) como 300 quadros
   descartados. O `Bwe` entrava em "descartando" e a estimativa ia para 358 kbps. Correção
   (`zap-net/src/media_rx/gap.rs`): quadro numa camada diferente da última recebida na trilha não
   abre buracos, e a contagem daquela camada recomeça. Documentado em `docs/api/midia.md`
   (`perdidos`) e em `balanceamento.md` §4. Testes: `volta_para_camada_antiga_nao_abre_buraco` e
   `quadros_da_camada_velha_no_meio_da_troca_nao_abrem_buraco`.

**c) Testes instáveis.**
- `alloc_loop::tick_com_espectador_pede_camada_e_pipeline_liga`: a espera passou de 1 s para 5 s,
  porque ligar a camada abre o codificador numa thread.
- `support::proxy::tests::set_rate_kbps_none_remove_limite` (`zap-net`): a rajada de 200 pacotes UDP
  transbordava o buffer do socket do sistema quando outros testes disputavam a CPU. Ela agora sai em
  blocos de 10, com 1 ms entre eles (~100 Mbps).

**d) Desvio aceito do `gpu-05`.** O `VideoProcessorBlt` roda no motor de vídeo da NVIDIA, e as
consultas de tempo da fila 3D marcam 0 µs. Por isso o piso do `gpu_timer_mede_blt` ficou em 0. A
"conversão GPU" da bancada sai 0,00 nesse driver. Quem mostra a conversão de fato é a coluna de CPU
(0,08 ms), junto com a espera de lock.

**e) Verificação final (orquestrador, máquina sem outro cargo).**
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` e
  `cargo test --workspace --no-fail-fast` passam: 1380 testes, 0 falhas, 2 ignorados.
- Aceite 2: 5 de 5 sem carga e 2 de 2 com `cargo build -p zap-media --release` em paralelo. Antes
  destas correções falhava 4 de 4.
- Aceites 1, 3, 4, prévia, reconexão e `aceite_camadas_local`: 3 de 3 cada.
- `prazo_keyframe`, `impaired` e `probe`: 3 de 3 cada. `online::` do `zap-net`: 20 de 20.
- `zap-test-server --test bridge`: 3 de 3. `zap-encode --test latencia`: 2 de 2, isolado.
- `core-online-22` e `-27` foram fechadas em `m4-tarefas-pendentes.json`.
- A bancada não foi medida de novo: estas correções não tocam `zap-gpu`, `zap-capture`,
  `zap-encode` nem `zap-media`.
