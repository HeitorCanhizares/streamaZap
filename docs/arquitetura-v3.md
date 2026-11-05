# StreamaZap v3: arquitetura em Rust

> Plano mestre (nível **arquiteto**). Os planos de cada marco (`docs/planos/`) e as tarefas
> pequenas saem daqui. Quando uma decisão daqui mudar, este arquivo é atualizado primeiro.

## 1. Objetivo

Reescrever o StreamaZap para ter o desempenho e a qualidade de um app de streaming de verdade
(nível Parsec/Moonlight no caminho de mídia), mantendo o que o app já faz bem:

- compartilhar **tela, janela e agora câmera**, com **som por aplicativo**;
- várias pessoas compartilhando ao mesmo tempo na mesma sala;
- modo **Local** (Radmin VPN / rede de casa, P2P) e o novo modo **Online** (servidor streamaZap,
  sala com código `ZAP-XXXX`, servidor como ponte quando a conexão direta falha);
- **qualidade individual por espectador**, limitada pela qualidade escolhida por quem transmite;
- vídeo liso sem configurar nada (suavização e bitrate automáticos);
- atualização automática segura.

Metas mensuráveis (PC médio com GPU dedicada, 1080p60, rede local):

| Métrica | Hoje (Python) | Meta v3 |
|---|---|---|
| Captura → pacote na rede (host) | ~15–30 ms, CPU alta | ≤ 8 ms, CPU < 5% |
| Atraso tela-a-tela na LAN | ~120–250 ms | ≤ 60 ms (suavização auto mínima) |
| Cópias de quadro pela RAM no host | 3 (8,3 MB cada) | 0 (tudo na GPU) |
| Decodificação no espectador | CPU (software) | GPU (hardware) |
| Troca de bitrate | reabre o codificador (keyframe) | sem keyframe, em ≤ 1 quadro |
| Espectadores com redes diferentes | todos recebem o mesmo bitrate | cada um recebe a camada que aguenta |

## 2. Decisões (e por quê)

| Tema | Decisão | Motivo | Alternativas descartadas |
|---|---|---|---|
| Linguagem do núcleo | **Rust** | Multithread sem GIL, sem GC (sem pausas no áudio), acesso direto às APIs do Windows (`windows` crate), binário pequeno | C++ (sem segurança de memória), C# (GC, runtime) |
| Interface | **Tauri 2 + React + TypeScript (Vite)** | A nova interface já é HTML; Tauri usa o WebView2 do Windows (instalador ~10 MB contra ~150 MB do PyInstaller) e tem updater assinado pronto | PySide6 (exige Python), egui/Slint (reescrever o design) |
| Captura de tela/janela | **Windows.Graphics.Capture (WGC)**, com DXGI Desktop Duplication de reserva | WGC captura janela atrás de outras **na GPU** (hoje é `PrintWindow` na CPU), entrega textura D3D11 | GDI/PrintWindow |
| Conversão de cor/escala | **D3D11 Video Processor** (BGRA → NV12 + escala por camada) | Roda no bloco de vídeo da GPU, sem shader próprio, sem cópia para RAM | swscale na CPU (hoje) |
| Codificação de vídeo | **Media Foundation** (MFT de hardware NVIDIA/AMD/Intel; MFT de software da Microsoft de reserva), H.264 | Recebe textura D3D11 direto (zero cópia), muda bitrate **sem reabrir**, força keyframe, não precisa de DLL do FFmpeg | FFmpeg/PyAV (cópias, DLLs grandes), SDKs NVENC/AMF/QSV separados (3x o código) |
| Decodificação e exibição | **WebCodecs no WebView2** (`VideoDecoder` com hardware) + `<canvas>` | Decodificação por hardware e desenho na GPU, já dentro da janela do app; o Rust entrega só o H.264 comprimido (poucos Mbps) no instante certo | Decodificar no Rust e copiar quadros para o WebView (cópias de 8 MB/quadro) |
| Transporte | **QUIC (`quinn`)**, ALPN `zap/4` | Sem bloqueio em fila do TCP; criptografia nativa; streams independentes por quadro com prioridade e descarte; datagramas para áudio; um só porto UDP | TCP (hoje), WebRTC completo (ICE/DTLS/SDP pesados para P2P sobre VPN) |
| Áudio | WASAPI *process loopback* (por app) + **Opus** 48 kHz 20 ms (FEC ligado) + saída WASAPI via `cpal` | Mesmo recurso de hoje, sem GIL na thread de áudio | — |
| Câmera | **Media Foundation Source Reader** → NV12 na GPU → mesmo codificador | Mesmo pipeline da tela; a câmera vira só mais uma trilha | `getUserMedia` no WebView (no Windows 10 a câmera é exclusiva de um processo) |
| Atualização | **tauri-plugin-updater** com assinatura ed25519 + `latest.json` nas Releases | Confere assinatura (hoje só SHA-256 do mesmo lugar), instalação por usuário sem UAC, nunca reinicia no meio de uma sala | Updater próprio |
| Timers no Windows | **Resolução de 1 ms (`timeBeginPeriod(1)`) enquanto houver sala ativa** (`zap-sys::platform`) | Sem isso as esperas do tokio/quinn, `Condvar`/`recv_timeout` andam em passos de 15,6 ms: um `interval` de 16,7 ms vira 31 ms (fonte a 32 fps), ACK atrasado infla o RTT e o emissor descarta quadros que a rede carregaria (medido no M4b) | Deixar o padrão do sistema; laços com espera ativa |
| COM no Windows | **MTA preso pelo processo inteiro** (`CoIncrementMTAUsage` uma vez) | Se a última thread que fez `CoInitializeEx` sai, o MTA morre junto com objetos (fábricas WinRT em cache) que outras threads ainda usam: violação de acesso (medido no M4b em `zap-capture`) | `CoInitialize`/`CoUninitialize` por thread sem mais nada |

Protocolo novo: **versão 4** (incompatível com o app Python, que é o 3). A tela "Diagnóstico" e
a descoberta mostram a versão de cada um, como hoje.

## 3. Mapa do repositório

```
Cargo.toml                 workspace
crates/
  zap-proto/   tipos e formatos do protocolo (sem I/O, sem plataforma)
  zap-rate/    algoritmos de qualidade/latência (sem I/O, determinísticos, simuláveis)
  zap-net/     QUIC (quinn), descoberta LAN, cliente do servidor Online
  zap-gpu/     dispositivo D3D11, texturas, conversor BGRA/NV12 → NV12 (Video Processor)
  zap-capture/ monitor (DXGI), janela (WGC), câmera (MF Source Reader), listas
  zap-encode/  H.264 por Media Foundation (MFT de hardware e de software)
  zap-audio/   Opus, loopback por app (WASAPI), saída com mixer lock-free
  zap-media/   pipelines de vídeo/áudio (threads, camadas) + backend sintético (testes)
  zap-sys/     integração com o Windows: firewall, interfaces de rede, Radmin/VPN
  zap-core/    orquestração: sala, publicação, assinatura, threads, estado para a interface
app/
  src-tauri/   comandos/eventos Tauri, configurações, updater (depende de zap-core)
  src/         interface React (porte do design em HTML)
docs/
  arquitetura-v3.md   este arquivo
  api/                contrato da API do servidor e do protocolo
  planos/             plano detalhado de cada marco + lista de tarefas
streamazap/            app Python atual (fica até a v3 atingir paridade, marco M6)
```

### Camadas e dependências permitidas

```
zap-proto  ←  zap-rate  ←  zap-net  ←  zap-core  ←  app/src-tauri
                                          ↑    ↑
                                    zap-sys    zap-media ← zap-capture ← zap-gpu
                                                         ← zap-encode  ← zap-gpu
                                                         ← zap-audio
```

- `zap-proto` e `zap-rate` **não** dependem de tokio, quinn nem `windows`. Todo algoritmo recebe o
  tempo como parâmetro (`Us`, microssegundos), para poder ser testado com relógio simulado.
- Código específico do Windows fica atrás de `#[cfg(windows)]`, só em `zap-gpu`, `zap-capture`,
  `zap-encode`, `zap-audio` e `zap-sys`. Todos compilam em Linux (stub), e `zap-media` tem um
  backend sintético, para os testes de ponta a ponta rodarem no CI Linux.
- `unsafe` só nesses crates do Windows, sempre com um comentário `// SAFETY:`.
- Por que tantos crates de mídia: isolam o `unsafe`, compilam mais rápido e deixam a hierarquia de
  agentes trabalhar em paralelo (nunca há duas tarefas no mesmo crate ao mesmo tempo).
- Monitor inteiro é capturado por DXGI Desktop Duplication (sem a borda amarela que o WGC desenha
  no Windows 10); janela, por WGC.

## 4. Modelo de dados

- **Sala** (`Room`): id, nome, dono, modo (`local` | `online`), acesso (`aberta` | `senha`),
  código `ZAP-XXXX` (online), participantes, trilhas publicadas.
- **Participante**: id, nome, cor, papel (`host` | `membro`), estado (assistindo quem, latência).
- **Trilha** (`Track`): mídia publicada por um participante. Tipos: `screen`, `window`, `camera`,
  `audio`. Cada participante pode publicar ao mesmo tempo uma trilha de tela ou janela, uma de
  câmera e uma de áudio.
- **Camada** (`Layer`): versão codificada de uma trilha de vídeo (simulcast). Até 3 por trilha.
- **Assinatura** (`Subscription`): espectador × trilha, com a camada máxima desejada pelo espectador
  (`auto` ou fixa) e o tamanho em que ele exibe (miniatura, principal, tela cheia).

### Escada de camadas (simulcast)

A camada **L0 é sempre a qualidade escolhida por quem transmite**. As outras saem dela:

| Trilha | L0 (escolhida) | L1 | L2 (prévia/miniatura) |
|---|---|---|---|
| Tela 1080p60 (8 Mbps) | 1080p60 · 8 Mbps | 720p30 · 2,5 Mbps | 360p15 · 0,4 Mbps |
| Tela 1080p30 (5 Mbps) | 1080p30 · 5 Mbps | 720p30 · 2,5 Mbps | 360p15 · 0,4 Mbps |
| Tela 720p60 (4 Mbps) | 720p60 · 4 Mbps | 540p30 · 1,2 Mbps | 360p15 · 0,4 Mbps |
| Tela 720p30 (2,5 Mbps) | 720p30 · 2,5 Mbps | 540p30 · 1,2 Mbps | 360p15 · 0,4 Mbps |
| Câmera | 720p30 · 1,5 Mbps | — | 360p15 · 0,3 Mbps |

- Uma camada só é codificada se alguém a assina (sem espectador = GPU parada, como hoje).
- Dentro de uma camada o bitrate do codificador varia entre 50% e 100% do nominal, sem trocar de
  camada (ajuste fino).
- Ninguém recebe mais que L0: essa é a "qualidade individual limitada pela escolha de quem
  transmite".

## 5. Pipeline e threads

### Quem transmite (por trilha de vídeo)

```
[thread captura]  WGC FrameArrived (pool free-threaded) ──textura BGRA──┐
                                                                         ▼
[thread vídeo da trilha]  dona do contexto D3D11 imediato
     ├─ VideoProcessor BGRA→NV12 L0 ──► MFT encoder L0 ─┐   (cada MFT de hardware roda
     ├─ VideoProcessor BGRA→NV12 L1 ──► MFT encoder L1 ─┤    assíncrono no motor de vídeo
     └─ VideoProcessor BGRA→NV12 L2 ──► MFT encoder L2 ─┤    da GPU; os eventos chegam
                                                         │    nas threads do MF)
                                                         ▼
                              canal (crossbeam, limitado) de EncodedFrame
                                                         ▼
[runtime tokio multi-thread]  Publisher: para cada espectador, envia a camada escolhida
     ├─ conexão QUIC A: 1 stream unidirecional por quadro (prioridade, descarte por prazo)
     ├─ conexão QUIC B: ...
     └─ Allocator (zap-rate) a cada 100 ms: decide camada/bitrate de cada um
```

Áudio: `[thread áudio, MMCSS "Pro Audio"]` WASAPI event-driven → mixa os apps escolhidos →
Opus 20 ms → canal → datagrama QUIC para cada espectador.

### Quem assiste (por trilha assinada)

```
[tokio] recebe quadros/datagramas ─► JitterBuffer (zap-rate) ─► agenda de exibição
[thread playout, alta prioridade]  no instante certo:
     ├─ vídeo: entrega o H.264 (Annex B) ao WebView por tauri::ipc::Channel (bytes crus)
     │        └─ JS: VideoDecoder (hardware, optimizeForLatency) → canvas
     └─ áudio: decodifica Opus → anel lock-free (rtrb) por trilha
[thread saída de áudio, MMCSS]  callback do cpal mixa os anéis com o volume de cada tile
```

Regras de concorrência:
- Nenhum lock no callback de áudio (só `rtrb` SPSC e atômicos).
- Canais entre threads são **limitados**; quando enchem, descarta o mais velho (vídeo) ou pede
  keyframe. Atraso escondido em fila é o inimigo.
- Um `D3D11Device` por processo com `SetMultithreadProtected(true)`; o contexto imediato só é usado
  pela thread de vídeo da trilha.
- Estado para a interface: `zap-core` publica *snapshots* imutáveis (`Arc`) no máximo 4x/s;
  a interface nunca segura lock do núcleo.

## 6. Algoritmos de qualidade e latência (`zap-rate`)

Todos são estruturas puras: `fn on_xxx(&mut self, now: Instant, ...)` e `fn poll(&mut self, now)`.
Cada um tem testes com o **simulador de enlace** (`zap-rate::sim`): banda, atraso, jitter, perda e
mudanças de banda ao longo do tempo, com relógio virtual.

### 6.1 Estimador de banda por enlace (`Bwe`)

Um por conexão (espectador, ou o servidor no modo Online). Entradas:
- envio de cada quadro (instante, tamanho, número);
- **relatório do receptor** a cada 50 ms: instante de chegada (relógio dele) do último byte de cada
  quadro, quadros perdidos/descartados e nível do jitter buffer;
- estatísticas do quinn: RTT suavizado, perdas e eventos de congestionamento.

Combinação (inspirada no GCC do WebRTC):
1. **Atraso**: agrupa quadros em rajadas de 5 ms, calcula a variação de atraso entre grupos,
   filtro de tendência (regressão linear em 20 grupos) e limiar adaptativo → `sobrecarga`,
   `normal` ou `ociosa`.
2. **Controle AIMD**: sobrecarga → alvo = 0,85 × vazão entregue medida; normal → sobe 8%/s longe
   da última sobrecarga, sobe aditivo (~1 pacote por RTT) perto dela; ociosa → mantém.
3. **Perda**: > 10% → reduz `(1 − 0,5·perda)`; < 2% → libera subida.
4. Estimativa = mínimo dos dois, limitada a `[150 kbps, 1,5 × soma das L0]`.

Ajustes decididos no fechamento do M1 (provados no simulador e no teste de rede ruim):
- **Fase inicial**: até a primeira sobrecarga ou perda ≥ 2%, sobe **50%/s** (como o *slow start*);
  acha 8 Mbps partindo de 1 Mbps em ~6 s. A "vizinhança da última sobrecarga" (subida aditiva,
  `max(96 kbps, 3%)`/s) é esquecida 5 s depois da última sobrecarga.
- **Sobrecarga do GCC só com 2 grupos seguidos** acima do limiar (o primeiro conta meio intervalo)
  e a tendência ainda subindo.
- **Filtro de rajada**: grupo com mais de 4× os bytes médios (keyframe) e os grupos logo atrás
  dele ficam fora da tendência até o atraso voltar ao nível de antes (no máximo 200 ms); se o
  atraso **piorar** além do da própria rajada, é congestionamento e o filtro sai na hora.
- **Fila persistente** (ideia do CoDel): se o menor atraso dos últimos 300 ms está 30 ms acima da
  base (menor atraso em 10 s), a fila nunca esvazia → sobrecarga, mesmo sem tendência (platô de
  fila cheia, gargalo com fila curta).
- **Descarte do emissor é sinal**: com ≥ 3 buracos de `seq` de vídeo por segundo, a estimativa vai
  para 0,95 × a vazão que chega. Sem isso, a fila fica no emissor (admissão/prazo), o receptor
  não vê atraso e a estimativa subiria até o dobro do que a rota aguenta.
5. **Sondagem**: para saber se cabe uma camada maior sem arriscar, quem roda o alocador daquele
   espectador (a ponte no Online com ponte; o publicador no Local e no direto) envia rajadas de
   enchimento (datagramas `0x03`) de 200 ms para que chegue `1,5 × kbps(próxima camada)`; o
   receptor mede a taxa de chegada do grupo e adota `max(estimativa, 0,95 × taxa)`. A própria
   fila da rajada não conta como sobrecarga (300 ms de carência). Sem sondas a estimativa de quem
   recebe uma camada barata fica presa em ~2× a vazão (regra de fonte limitada) e ninguém sobe
   da camada 2. Regras e números: `docs/api/balanceamento.md` §4 (decisão do M4b,
   `docs/planos/m4b-pendencias.md`).

### 6.2 Alocador de camadas (`Allocator`): o "balanceamento por pessoa"

A cada 100 ms, por trilha publicada:
1. Para cada espectador: teto = `min(L0, camada pedida pelo espectador, camada adequada ao tamanho
   do tile)`. Miniatura recebe L2; tela principal recebe até L0.
2. Começa todo mundo na menor camada que precisa receber (L2).
3. **Sobe um degrau por vez, em rodízio** (justiça max-min), enquanto
   `estimativa_do_espectador ≥ 1,15 × bitrate da próxima camada` **e** sobrar banda de upload de
   quem publica (no P2P o upload é dividido entre todos; no Online cada camada sobe uma vez só para
   o servidor).
4. **Histerese**: sobe só se a condição vale há ≥ 3 s e não houve subida há 5 s; desce na hora com
   sobrecarga ou com `estimativa < 0,95 × bitrate` por 500 ms.
5. A troca de camada acontece no próximo keyframe da camada nova (pede um, no máximo 1 por segundo
   por camada).
6. Bitrate fino de cada camada = mínimo das estimativas dos espectadores daquela camada, limitado a
   50–100% do nominal; muda o codificador sem reabrir.
7. Camadas sem assinante são desligadas.

No modo Online o **servidor** roda o mesmo alocador para a descida (servidor → espectador) e quem
publica roda a parte de subida (publicador → servidor). O algoritmo está especificado em
`docs/api/balanceamento.md`, para o servidor poder ser escrito em qualquer linguagem.

### 6.3 Prazo e descarte no envio

- Cada quadro vai num stream QUIC próprio, em ordem FIFO (`send_fairness = false`), keyframe com
  prioridade maior. Controle de congestionamento do QUIC: Cubic.
- O quadro só termina quando o outro lado **confirma** todos os bytes. Sem confirmação em
  `max(150 ms, 2 × RTT)`: o stream é resetado (`reset`, sem retransmitir). Antes de abrir o stream,
  **admissão**: se a fila à frente não esvazia no prazo na taxa atual (cwnd/RTT), o quadro é
  descartado na hora. O espectador marca a trilha como "precisa de keyframe".
- Keyframe de recuperação: no máximo 1 a cada 2 s por espectador lento, para um espectador ruim não
  forçar keyframes caros para todo mundo (regra já usada no app Python).

### 6.4 Jitter buffer e suavização (`JitterBuffer`, `MediaClock`)

Evolução do `MediaClock` do Python, agora com estatística em vez de regras fixas:
- `trânsito = chegada − timestamp_de_captura`; base = mínimo em 10 s (cancela a diferença de
  relógio entre os PCs).
- `jitter = trânsito − base`. Alvo automático = **quantil 95% do jitter em janela de 5 s** +
  margem de decodificação (1 intervalo de quadro), limitado a `[0, 1500 ms]`.
- Mudanças no alvo são aplicadas devagar: até ±2% de velocidade de reprodução (o áudio estica ou
  encolhe com reamostragem de alta qualidade via `rubato`, sem cortes). Atraso > 300 ms acima do
  alvo pula direto (no vídeo, até o próximo keyframe; no áudio, na próxima pausa).
- Manual: a interface fixa o alvo (slider "Suavização").
- Vídeo e áudio de uma pessoa usam o **mesmo relógio**, então ficam sincronizados.

### 6.5 Métricas expostas (tela da sala e "Diagnóstico")

Por trilha assinada: camada, resolução/fps, Mbps, RTT, suavização atual, quadros perdidos/
atrasados, motivo da última troca de camada. Por trilha publicada: camadas ativas, espectadores
por camada, tempo de captura/conversão/codificação (média e p95).

## 7. Rede

### 7.1 Conexão QUIC (`zap/4`)

- **Stream bidirecional 0**: controle. Mensagens JSON com prefixo de tamanho (u32 BE). Mesmo
  esquema de mensagens usado no WebSocket do servidor (fonte única: `docs/api/mensagens.md`).
- **Streams unidirecionais**: um por quadro de vídeo. Cabeçalho binário fixo (trilha, camada,
  número, flags, timestamp de captura em µs, tamanho) + H.264 Annex B.
- **Datagramas**: áudio Opus (trilha, número, timestamp + pacote) e padding de sondagem.
- Keep-alive 1 s; queda detectada em 5 s (hoje 15 s), reconexão automática com backoff.
- Certificado autoassinado por instalação (persistido); a impressão digital vai no anúncio da
  descoberta e no código de convite (fixação de certificado). Senha de sala: HMAC-SHA256 do nonce,
  como hoje.

### 7.2 Modos

- **Local**: quem compartilha é o próprio servidor QUIC da sua trilha (porta UDP 47800; se
  estiver ocupada, a próxima). Descoberta por broadcast UDP 47801 em todas as interfaces, Radmin
  primeiro, com presença ("recebe você"/"não recebe") como hoje. Conexão reversa continua: se A não
  alcança B, B conecta em A.
- **Online**: sinalização e lista de salas pelo servidor (REST + WebSocket); mídia direta P2P
  quando possível (endereços trocados pela sinalização, tentativa em paralelo por 3 s) e **ponte
  pelo servidor** quando a direta falha (configurável: com `use_relay = false` o app não conecta
  na ponte e a trilha espera a rota direta). O contrato do servidor está em `docs/api/`.

## 8. Atualização

- `tauri-plugin-updater` lendo `latest.json` da última Release (gerado pelo CI) com assinatura
  ed25519; a chave pública vai embutida no app, a privada fica em segredo do GitHub
  (`TAURI_SIGNING_PRIVATE_KEY`).
- Instalação **por usuário** (NSIS `currentUser`): atualizar não pede administrador. O
  administrador só é pedido uma vez, em "Corrigir firewall".
- Baixa em segundo plano; **nunca reinicia com você numa sala**: instala ao fechar, ou pergunta
  ao sair da sala.
- Canais `estável` e `beta` (endpoints diferentes).
- Se um amigo (descoberta) ou o servidor anunciar protocolo maior que o seu, o app oferece a
  atualização na hora.

## 9. Interface

Porte do design (`docs/design/`) para React/TS com os mesmos tokens visuais (fundo `#0A100E`,
texto `#E6F4EC`, verde de destaque; fontes Unbounded, Space Grotesk e JetBrains Mono embutidas via
`@fontsource`). Telas: **Salas**, **Criar sala**, **Dentro da sala**, **Configurações**,
**Diagnóstico**, e o componente **prévia de vídeo**.

Ponte com o núcleo:
- **Comandos** (`invoke`): `rooms_list`, `room_create`, `room_join`, `room_leave`,
  `share_start/update/stop`, `camera_toggle`, `subscribe(track, tile_size, max_layer)`,
  `set_volume`, `chat_send`, `settings_get/set`, `diagnostics_run`, `firewall_fix`,
  `update_check/install`, `list_sources` (telas, janelas, câmeras, apps com som).
- **Eventos** (`listen`): `state` (snapshot ≤ 4/s), `chat`, `stats`, `update`.
- **Canais de vídeo**: um `Channel` por tile, levando quadros H.264 já no tempo de exibição.

## 10. Testes e CI

- Linux: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` dos crates portáveis
  (incluindo o simulador de rede e testes de ponta a ponta com o backend sintético) e
  `cargo check --target x86_64-pc-windows-gnu --workspace` (pega erro de compilação do código
  Windows sem precisar de Windows).
- Windows (CI `windows-latest`): `cargo test --workspace`, incluindo testes reais de captura
  (imagem não preta), conversão NV12, codificação (o H.264 decodifica de volta pelo MF) e
  loopback de áudio.
- Front-end: `tsc --noEmit`, `vitest` nos módulos de lógica (agenda de quadros, formatação).
- Release: `tauri-action` gera o instalador NSIS, o `.zip` portátil, as assinaturas e o
  `latest.json`.

## 11. Marcos

| Marco | Entrega | Pronto quando |
|---|---|---|
| **M0** | Plano, doc da API, hierarquia de agentes, esqueleto do workspace e CI | `cargo check` passa em Linux e `windows-gnu`; CI verde |
| **M1** | Núcleo portável: `zap-proto` (mensagens, cabeçalhos, código de sala), `zap-rate` (BWE, alocador, jitter buffer, simulador), `zap-net` (QUIC + descoberta) | Testes no simulador provam as metas (seção 6); 2 processos trocam mídia sintética por QUIC no loopback com perda e atraso artificiais |
| **M2** | Mídia no Windows: WGC + DXGI, Video Processor NV12, encoder MF com camadas, WASAPI loopback por app, Opus, saída de áudio, câmera | Testes no CI Windows; medições de tempo por etapa |
| **M3** | `zap-core` + app Tauri: salas Local de ponta a ponta, interface nova portada, WebCodecs, chat, várias pessoas compartilhando | Dois PCs numa rede local assistem e conversam |
| **M4** | Modo Online (cliente da API do servidor, ponte, convites `ZAP-XXXX`, prévia das salas) | Funciona com um servidor que siga `docs/api/` |
| **M5** | Updater assinado, firewall e diagnóstico portados, instalador NSIS, release no CI | Atualiza sozinho de uma versão para a outra |
| **M6** | Paridade e remoção do app Python | README atualizado, Python removido |

## 12. Riscos

| Risco | Mitigação |
|---|---|
| MFT de hardware varia por fabricante (opções suportadas, latência) | Testar opções com `ICodecAPI::IsSupported`; MFT de software de reserva; telemetria local de tempo por etapa |
| Limite de sessões NVENC em placas GeForce (8 hoje) | No máximo 3 camadas de tela + 2 de câmera; desligar camadas sem assinante |
| WebCodecs indisponível ou sem hardware em algum PC | Detectar com `VideoDecoder.isConfigSupported`; cair para decodificação por software do próprio WebCodecs |
| Código Windows sem teste local | `cargo check` em `windows-gnu` em todo commit + testes no CI Windows |
| QUIC/UDP bloqueado em alguma rede | No Online, ponte pelo servidor; diagnóstico mostra se o UDP passa |

## 13. Fluxo de trabalho com agentes

Ver `CLAUDE.md` e `.claude/agents/`. Resumo: **arquiteto (Opus)** escreve ou atualiza o plano de um
marco → **decompositor (Sonnet)** quebra em tarefas pequenas com dono de arquivo e critérios →
**implementador (Haiku)** faz cada tarefa e roda os testes → **revisor (Sonnet)** confere contra a
especificação e assume o que o Haiku não resolver em 2 tentativas. O workflow salvo
`.claude/workflows/zap-marco.js` executa esse ciclo.
