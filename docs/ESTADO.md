# Estado da v3 (Rust + Tauri)

Atualizado ao pausar o trabalho. Branch: `claude/loving-davinci-le0qac`.

## Resumo

| Marco | Situação | O que tem |
|---|---|---|
| M0 | ✅ | Plano mestre (`docs/arquitetura-v3.md`), API (`docs/api/`), design (`docs/design/`), hierarquia de agentes (`.claude/`), workspace e CI |
| M1 | ✅ | `zap-proto` (protocolo 4), `zap-rate` (estimador de banda, balanceamento por espectador, suavização, simulador), `zap-net` (QUIC com prazo por quadro, relatórios, descoberta) |
| M2 | ✅ (falta validar na GPU real) | `zap-gpu`, `zap-capture`, `zap-encode`, `zap-audio`, `zap-media`: captura DXGI/WGC/câmera, NV12 na GPU, H.264 por Media Foundation, áudio por app, Opus, pipelines |
| M3 / M3b | ✅ | `zap-core` (sala Local, publicação, assinatura, playout), app Tauri (`app/src-tauri`), interface React fiel ao design (`app/src`) ligada ao núcleo |
| M5 | ✅ (falta testar uma release de verdade) | Atualização assinada (instala ao fechar, nunca reinicia numa sala, canais estável/beta), firewall por COM, diagnóstico real, `release-v3.yml`, instalador NSIS que remove o app Python |
| **M4** | ⏸️ **pela metade** | Modo Online: cliente da API e servidor de teste prontos; sessão Online no núcleo, comandos e telas incompletos |
| M6 | ⏳ | Paridade e remoção do app Python |

O app Python (`streamazap/`) continua intacto e é o que as Releases atuais publicam.

## O que foi verificado e como

- **Linux (aqui e no CI)**: `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace` (inclui
  os testes de aceite no simulador de rede e os de sala com mídia sintética), e
  `cargo check/clippy --target x86_64-pc-windows-gnu` (o código do Windows compila e os testes
  linkam com MinGW). Front-end: `tsc`, ~600 testes vitest, `vite build`.
- **App de verdade**: o binário Tauri abre num X virtual e mostra a interface com dados do núcleo
  (`docs/design/prints/app-real-salas.png`).
- **Windows (CI `windows-latest`)**: rodaram de verdade o loopback de áudio por app (WASAPI,
  blocos de 20 ms), Opus, listas de apps/janelas/monitores, firewall por COM, VPNs, abertura do
  codificador de software do Media Foundation e o pipeline sintético.
  **O runner não tem GPU com suporte a vídeo**, então os testes de conversão NV12, captura DXGI/WGC
  e codificação H.264 **pularam**. Eles precisam de um PC com GPU (ou do exemplo de bancada,
  `cargo run -p zap-media --example bancada`).

## O que está pela metade (M4) — e como continuar

O M4 parou em duas etapas: primeiro o disco da sessão encheu (o `target/` passou de 25 GB; agora o
perfil de desenvolvimento gera bem menos e o `CLAUDE.md` manda rodar `cargo clean` se encher), e
depois o trabalho foi pausado a pedido.

- Pronto: cliente REST/WebSocket da API (`crates/zap-net/src/online/`), o **servidor de teste**
  (`crates/zap-test-server`, completo: REST, WebSocket, ponte QUIC com o alocador, relatório de
  subida, prévia; testes de aceite em `tests/rest_ws.rs` e `tests/bridge.rs`) e parte da sessão
  Online (`crates/zap-core/src/online/`).
- O servidor de teste segue o contrato de `docs/api/` como o servidor de produção, inclusive nos
  canais: `camadas.pedidas`, `keyframe.pedir` e `camada.atual` saem **só pelo controle QUIC** da
  ponte. As tarefas `core-online-10` e `-19` foram escritas esperando essas mensagens pelo
  WebSocket; a especificação delas no JSON ganhou uma nota com o ajuste. O que o servidor de teste
  não faz está no topo de `crates/zap-test-server/src/lib.rs`.
- Faltam 33 tarefas, com a especificação completa salva em
  **`docs/planos/m4-tarefas-pendentes.json`** (sessão Online: 21; comandos Tauri: 3; telas: 9).
  O código ainda tem `todo!("tarefa ...")` nesses pontos, então
  **o clippy e os testes do workspace não passam neste commit**: compila, mas os testes que tocam
  o modo Online entram em pânico.
- Para continuar com a hierarquia de agentes: rode o workflow salvo `zap-marco` com
  `args: {marco: "M4 (continuação)", plano: "docs/planos/m4-online.md", frentes: [...], tarefas: <conteúdo do JSON acima>}`
  (frentes `core-online` → `tauri-online` → `ui-online`, travas `zap-core`, `streamazap`, `app-ui`).
  O workflow pula a decomposição dessas frentes e só implementa; dependências de tarefas que já
  saíram do JSON (as `zt-*` do servidor de teste) contam como prontas.

## Mudança desta pausa ainda não validada no CI

`zap-gpu` agora cria o dispositivo em cadeia: GPU com vídeo → GPU sem vídeo → qualquer adaptador
(inclusive o "Basic Render Driver" de VMs e do CI). Sem vídeo, captura e texturas funcionam e só o
conversor (Video Processor) devolve `Unsupported`. Assim o CI passa a exercitar DXGI e texturas.
Falta conferir no próximo CI Windows. Na prática, um PC sem Video Processor ainda não consegue
transmitir: falta uma conversão para NV12 pela CPU (próximo passo do M2).

## O que depende de você

1. **Chaves do updater**: `cd app && npx tauri signer generate`, colar a chave pública em
   `app/src-tauri/tauri.conf.json` (`plugins.updater.pubkey`, hoje um placeholder) e criar os
   segredos `TAURI_SIGNING_PRIVATE_KEY` e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` no GitHub
   (detalhes no README, seção da v3).
2. **Teste num PC com GPU**: `cd app && npm install && npm run tauri dev`; criar sala, entrar de
   outro PC (Radmin/LAN), compartilhar tela e câmera. E a bancada de mídia
   (`cargo run -p zap-media --example bancada -- --monitor 1 --preset 1080p60 --segundos 10`).
3. **Servidor Online**: o servidor de produção já existe e segue o contrato de `docs/api/`. Os
   detalhes onde a especificação deixava margem foram registrados lá. Ele é mantido à parte, num
   repositório privado, e o app fala com ele só como cliente, em
   `streamazap.heitorcanhizares.net:7443`. Como o repositório é privado, o CI do app não consegue
   usá-lo. Por isso os testes do M4 usam o `crates/zap-test-server`, que roda no CI.
4. **Antes da primeira release da v3 (M6)**: parar o `build.yml` do Python de publicar Releases no
   `master`; senão a "última release" volta a ser a do Python e o updater da v3 não acha o
   `latest.json`.

## Melhorias conhecidas (não bloqueiam)

- Codificador de hardware: trocar a espera de eventos com `sleep(1 ms)` por `GetEvent` bloqueante.
- Teste de qualidade do Opus usa complexidade 4 (produção usa 8).
- Diagnóstico/servidor: o card do servidor só fica real com o M4.
