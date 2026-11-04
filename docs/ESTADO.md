# Estado da v3 (Rust + Tauri)

Atualizado em 2026-11-04, ao pausar o trabalho. O código está no `master` (e igual na branch
`claude/loving-davinci-le0qac`).

## Antes de qualquer push no `master`

- **Todo push no `master` publica uma Release do app Python.** O `build.yml` gera o instalador e
  marca como a última Release. Até o M6, ponha `[skip release]` na mensagem do commit do topo do
  push (é a do `head_commit` que conta).
- **O CI da v3 (`rust.yml`) falha no `master`** enquanto o M4 tiver `todo!`: compila, mas o clippy
  acusa código Online ainda não ligado e os testes que tocam o modo Online entram em pânico. O
  `cargo test -p zap-test-server` e os crates que não tocam o Online passam.

## Resumo

| Marco | Situação | O que tem |
|---|---|---|
| M0 | ✅ | Plano mestre (`docs/arquitetura-v3.md`), API (`docs/api/`), design (`docs/design/`), hierarquia de agentes (`.claude/`), workspace e CI |
| M1 | ✅ | `zap-proto` (protocolo 4), `zap-rate` (estimador de banda, balanceamento por espectador, suavização, simulador), `zap-net` (QUIC com prazo por quadro, relatórios, descoberta) |
| M2 | ✅ (falta validar na GPU real) | `zap-gpu`, `zap-capture`, `zap-encode`, `zap-audio`, `zap-media`: captura DXGI/WGC/câmera, NV12 na GPU, H.264 por Media Foundation, áudio por app, Opus, pipelines |
| M3 / M3b | ✅ | `zap-core` (sala Local, publicação, assinatura, playout), app Tauri (`app/src-tauri`), interface React fiel ao design (`app/src`) ligada ao núcleo |
| M5 | ✅ (falta testar uma release de verdade) | Atualização assinada (instala ao fechar, nunca reinicia numa sala, canais estável/beta), firewall por COM, diagnóstico real, `release-v3.yml`, instalador NSIS que remove o app Python |
| **M4** | ⏸️ **pela metade** | Modo Online: contrato, cliente da API e servidor de teste prontos; servidor de produção no ar; sessão Online no núcleo, comandos do Tauri e telas incompletos |
| M6 | ⏳ | Paridade e remoção do app Python |

O app Python (`streamazap/`) continua intacto e é o que as Releases atuais publicam.

## O que falta para usar o app, em ordem

### 1. Modo Online (M4), no app

O servidor de produção e o de teste estão prontos; falta o lado do app. São 33 tarefas, com a
especificação completa em **`docs/planos/m4-tarefas-pendentes.json`**.

**a) Núcleo (`core-online`, 21 tarefas).** `todo!` por arquivo, em `crates/zap-core/src/`:

| Arquivo | `todo!` | Tarefa | O que é |
|---|---|---|---|
| `online/session.rs` | 5 | 05 | conexão, login, ping, `start`/`restart`/`stop` |
| `online/session_run.rs` | 1 | 06 | laço de fundo: login, ping de 5 s, leitura do WebSocket, reconexão |
| `online/bridge.rs` | 1 | 07 | conexão QUIC com a ponte usando o ingresso |
| `subscribe/pool.rs` | 2 | 19 | conexão de assinatura pela ponte/direta |
| `online/direct.rs` | 3 | 13 | rota direta (`p2p.*`) com queda para a ponte |
| `online/room_events.rs` | 1 | 10 | despacho das mensagens para a sala |
| `online/reconnect.rs` | 3 | 11 | reconexão do WebSocket e da ponte |
| `online/room.rs` | 5 | 08 | sala Online: criar, entrar, chat, sair |
| `online/preview.rs` | 2 | 14 | prévia na lista (camada 2, sem áudio) |
| `online/entry.rs` | 11 | 15 | pontos de entrada do `Core` e ganchos de teste |

Sem `todo!`, mas ainda por fazer: **18** (ligar a sessão Online ao ciclo de vida do `Core`, em
`engine.rs` e `engine_room.rs`); **20** a **26** (harness e testes de aceite em
`crates/zap-core/tests/`: aceites 1 a 4, prévia e reconexão); **27** (fechamento: fmt, clippy,
testes, `windows-gnu`).

**Mudança de desenho obrigatória.** O núcleo foi planejado esperando `camadas.pedidas`,
`keyframe.pedir` e `camada.atual` pelo WebSocket. Pelo contrato (`docs/api/mensagens.md`, "Canal de
cada mensagem"), o servidor de produção e o de teste mandam essas três **só pelo controle QUIC da
ponte**. O leitor de controle da ponte (publicador: `online/publish_bridge.rs`; espectador:
`subscribe/pool.rs`) precisa entregá-las ao mesmo tratamento de `room_events`. As tarefas 10, 19 e
22 têm uma nota sobre isso no JSON, e o topo de `online/mod.rs` já descreve os canais certos.

**b) Comandos do Tauri (`tauri-online`, 3 tarefas).** `server_test`, `room_preview` e
`room_preview_stop` em `app/src-tauri/src/commands/online.rs` (3 `todo!`); `api.ts`, mock e tipos
na interface (`snapshot.server`); fechamento da frente.

**c) Telas (`ui-online`, 9 tarefas).** Status do servidor na barra (online / conectando /
desconectado / não configurado); "Entrar com código" (`ZAP-XXXX`) na lista de salas; prévia real
em baixa resolução; "Testar conexão" e o interruptor de ponte em Configurações; teste de
integração no mock. Arquivos com marcação pendente: `bridge/useRoomPreview.ts`,
`bridge/serverModel.ts`, `screens/config/useServerTest.ts`, `screens/diag/diagServidor.ts`,
`screens/salas/EntrarCodigo.tsx`.

**d) Endereço do servidor.** O de produção é `https://streamazap.heitorcanhizares.net:7443`, **sem
`/v1`**: o cliente (`ServerClient`) acrescenta `/v1` sozinho, e com `/v1` no endereço as chamadas
dão 404. Hoje `settings.server.address` vem vazio (`crates/zap-core/src/settings.rs`); falta
decidir se o servidor de produção vira o padrão. O servidor não exige senha por enquanto
(`requer_senha: false`); a senha do servidor ainda está para ser decidida.

### 2. Teste num PC com GPU (M2)

A captura DXGI/WGC, a conversão NV12 e o H.264 por hardware nunca rodaram de verdade: o CI não tem
GPU com vídeo. Passos: `cd app && npm install && npm run tauri dev`; criar sala, entrar de outro PC
(Radmin/LAN), compartilhar tela e câmera. E a bancada de mídia
(`cargo run -p zap-media --example bancada -- --monitor 1 --preset 1080p60 --segundos 10`).

Também falta a **conversão para NV12 pela CPU**: sem ela, um PC sem Video Processor não consegue
transmitir. E falta conferir no CI Windows a cadeia de dispositivos do `zap-gpu` (GPU com vídeo →
GPU sem vídeo → qualquer adaptador, inclusive o "Basic Render Driver"), feita na pausa anterior.

### 3. Chaves do updater e uma release de teste (M5)

`cd app && npx tauri signer generate`, colar a chave pública em `app/src-tauri/tauri.conf.json`
(`plugins.updater.pubkey`, hoje `COLE_AQUI_A_CHAVE_PUBLICA`) e criar os segredos
`TAURI_SIGNING_PRIVATE_KEY` e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` no GitHub (detalhes no README,
seção da v3). Depois, testar uma release de verdade pelo `release-v3.yml` (tag `v3.*`).

### 4. Troca do app publicado (M6)

Paridade com o app Python, remoção de `streamazap/` e, **antes da primeira release da v3**, parar o
`build.yml` de publicar o Python no `master`; senão a "última release" volta a ser a do Python e o
updater da v3 não acha o `latest.json`.

## O que está pronto no M4

- **Contrato** (`docs/api/`): inclui o que o servidor de produção faz onde a especificação deixava
  margem (canais, `ola` como `sala.entrar`, códigos de `recusado`, prévia, limites).
- **Cliente da API** (`crates/zap-net/src/online/`): REST, WebSocket, renovação de token.
- **Servidor de teste** (`crates/zap-test-server`): REST, WebSocket, ponte QUIC com o alocador,
  relatório de subida, prévia, os mesmos canais do servidor de produção. 103 testes de unidade, 7
  de REST/WebSocket (`tests/rest_ws.rs`) e 2 de aceite da ponte (`tests/bridge.rs`). O que ele não
  faz está no topo de `src/lib.rs` (rota direta, latência, `sala.saiu`…).
- **Servidor de produção**: mantido à parte, num repositório privado; o app fala com ele só como
  cliente. Como é privado, o CI do app não o usa: os testes do M4 usam o `zap-test-server`. No ar
  desde 2026-11-04 em `streamazap.heitorcanhizares.net:7443` (certificado do Let's Encrypt até
  2027-02-02). Testado com o próprio `zap-net`: QUIC passa pela rede corporativa, RTT de 26–28 ms,
  10 s de vídeo e áudio pela ponte sem perdas, troca de camada como no contrato.
- **Critério de aceite 2 corrigido**: pela folga de `docs/api/balanceamento.md`, um enlace de 3 Mbps
  fica na camada 2; o aceite usa 4 Mbps (`docs/planos/m4-online.md`).

## Como continuar o M4

- Para continuar com a hierarquia de agentes: rode o workflow salvo `zap-marco` com
  `args: {marco: "M4 (continuação)", plano: "docs/planos/m4-online.md", frentes: [...], tarefas: <conteúdo do JSON acima>}`
  (frentes `core-online` → `tauri-online` → `ui-online`, travas `zap-core`, `streamazap`, `app-ui`).
  O workflow pula a decomposição dessas frentes e só implementa; dependências de tarefas que já
  saíram do JSON (as `zt-*` do servidor de teste) contam como prontas.
- Trabalhando direto: siga a ordem das dependências (05 → 06 → 07 → 19 → 13 → 10 → 11 → 08 → 14 →
  15 → 18 → testes 20–26 → 27); cada tarefa do JSON tem spec, arquivos e testes.
- Disco: o `target/` já passou de 25 GB numa sessão. O perfil de desenvolvimento gera bem menos
  agora, e o `CLAUDE.md` manda rodar `cargo clean` se encher.

## O que foi verificado e como

- **Linux (CI)**: `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace` (inclui
  os testes de aceite no simulador de rede e os de sala com mídia sintética), e
  `cargo check/clippy --target x86_64-pc-windows-gnu` (o código do Windows compila e os testes
  linkam com MinGW). Front-end: `tsc`, ~600 testes vitest, `vite build`. Isso valia antes do M4;
  com os `todo!` do M4, o clippy e os testes do workspace falham (ver o topo).
- **Windows (aqui, sem GPU de vídeo)**: `zap-test-server` com fmt, clippy `-D warnings` e todos
  os testes; `zap-net` (`online::types`).
- **App de verdade**: o binário Tauri abre num X virtual e mostra a interface com dados do núcleo
  (`docs/design/prints/app-real-salas.png`).
- **Windows (CI `windows-latest`)**: rodaram de verdade o loopback de áudio por app (WASAPI,
  blocos de 20 ms), Opus, listas de apps/janelas/monitores, firewall por COM, VPNs, abertura do
  codificador de software do Media Foundation e o pipeline sintético.
  **O runner não tem GPU com suporte a vídeo**, então os testes de conversão NV12, captura DXGI/WGC
  e codificação H.264 **pularam**.

## Melhorias conhecidas (não bloqueiam)

- Codificador de hardware: trocar a espera de eventos com `sleep(1 ms)` por `GetEvent` bloqueante.
- Teste de qualidade do Opus usa complexidade 4 (produção usa 8).
- Diagnóstico/servidor: o card do servidor só fica real com o M4.
- Servidor de teste: não manda `camadas.pedidas` vazio logo depois de `trilha.publicada` (o de
  produção manda) nem eventos de sala pelo QUIC quando não há WebSocket.
