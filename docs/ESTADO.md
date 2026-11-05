# Estado da v3 (Rust + Tauri)

Atualizado em 2026-11-04, no fechamento da rodada M4b + M2. O trabalho desta rodada está na branch
`m4-online` (sem commit ainda); `master` e `claude/loving-davinci-le0qac` têm o estado anterior.

## Antes de qualquer push no `master`

- **Todo push no `master` publica uma Release do app Python.** O `build.yml` gera o instalador e
  marca como a última Release. Até o M6, ponha `[skip release]` na mensagem do commit do topo do
  push (é a do `head_commit` que conta).
- **O CI da v3 (`rust.yml`) ainda não foi conferido com o M4 pronto.** Não há mais `todo!`, mas o
  `cargo test --workspace` tem falhas abertas (ver "Atualização do M4") e o clippy do alvo
  `windows-gnu` não foi rodado (o fechamento usou o alvo nativo MSVC).

## Resumo

| Marco | Situação | O que tem |
|---|---|---|
| M0 | ✅ | Plano mestre (`docs/arquitetura-v3.md`), API (`docs/api/`), design (`docs/design/`), hierarquia de agentes (`.claude/`), workspace e CI |
| M1 | ✅ | `zap-proto` (protocolo 4), `zap-rate` (estimador de banda, balanceamento por espectador, suavização, simulador), `zap-net` (QUIC com prazo por quadro, relatórios, descoberta) |
| M2 | 🟡 (rodou numa GTX 1060; bancada fora da meta) | `zap-gpu`, `zap-capture`, `zap-encode`, `zap-audio`, `zap-media`: captura DXGI/WGC/câmera, NV12 na GPU, H.264 por Media Foundation, áudio por app, Opus, pipelines |
| M3 / M3b | ✅ | `zap-core` (sala Local, publicação, assinatura, playout), app Tauri (`app/src-tauri`), interface React fiel ao design (`app/src`) ligada ao núcleo |
| M5 | ✅ (falta testar uma release de verdade) | Atualização assinada (instala ao fechar, nunca reinicia numa sala, canais estável/beta), firewall por COM, diagnóstico real, `release-v3.yml`, instalador NSIS que remove o app Python |
| **M4** | 🟡 **quase** | Modo Online implementado (núcleo, Tauri, telas, modo só direta, sondagem); falta o aceite 2 (camadas por enlace) e o servidor de produção sondar |
| M6 | ⏳ | Paridade e remoção do app Python |

O app Python (`streamazap/`) continua intacto e é o que as Releases atuais publicam.

## O que falta para usar o app, em ordem

### 1. Modo Online (M4), no app

Implementado: núcleo, comandos do Tauri e telas (sem `todo!` no repositório). O que falta está em
"Atualização do M4" abaixo; a especificação das tarefas, com `estado` e `nota` das três que a
rodada M4b fechou ou deixou abertas, continua em `docs/planos/m4-tarefas-pendentes.json`.

**Canais (mantido do contrato).** `camadas.pedidas`, `keyframe.pedir` e `camada.atual` chegam só
pelo controle QUIC da ponte (`docs/api/mensagens.md`, "Canal de cada mensagem"), não pelo
WebSocket; os leitores de controle da ponte (`online/publish_bridge.rs`, `subscribe/pool.rs`)
entregam essas mensagens ao tratamento de `room_events`.

**Endereço do servidor.** O de produção é `https://streamazap.heitorcanhizares.net:7443`, **sem
`/v1`**: o `ServerClient` acrescenta `/v1` sozinho. Só `Core::start` grava
`settings::PRODUCTION_SERVER` na primeira execução; `CoreOptions::default()` tem
`default_server: None`, então nenhum teste fala com o servidor de produção. O servidor não exige
senha por enquanto (`requer_senha: false`).

### 2. Teste num PC com GPU (M2)

Feito neste PC (Windows 10, GTX 1060, NVENC): captura DXGI, conversão NV12 e H.264 por hardware
rodam; os testes de `zap-capture`, `zap-gpu` e `zap-media` passam, a violação de acesso do
`zap-capture` e da bancada foi resolvida (MTA preso, `GpuDevice: Clone`) e a bancada release não
caiu. **Mas a bancada não bate a meta**: com movimento na tela, 25,9 fps na camada 0 e p95 somado
de 72 ms (meta 8 ms), com quadros descartados (números em `docs/planos/m4b-pendencias.md`, seção 8).
Com a tela parada a duplicação entrega ~0,5 quadro/s e a bancada mede espera (os 2,9 fps / 703 ms
registrados antes): a bancada precisa gerar movimento sozinha. Falta investigar por que conversão
(~12 ms) e codificação (~21 ms) estão tão altas numa GTX 1060 (contenção do contexto D3D11 entre
conversor e codificador, relógio da GPU, três sessões NVENC). Ainda falta
testar a transmissão entre dois PCs (`cd app && npm install && npm run tauri dev`), a **conversão
para NV12 pela CPU** (sem ela, um PC sem Video Processor não transmite) e a cadeia de dispositivos
do `zap-gpu` no CI Windows.

### 3. Chaves do updater e uma release de teste (M5)

`cd app && npx tauri signer generate`, colar a chave pública em `app/src-tauri/tauri.conf.json`
(`plugins.updater.pubkey`, hoje `COLE_AQUI_A_CHAVE_PUBLICA`) e criar os segredos
`TAURI_SIGNING_PRIVATE_KEY` e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` no GitHub (detalhes no README,
seção da v3). Depois, testar uma release de verdade pelo `release-v3.yml` (tag `v3.*`).

### 4. Troca do app publicado (M6)

Paridade com o app Python, remoção de `streamazap/` e, **antes da primeira release da v3**, parar o
`build.yml` de publicar o Python no `master`; senão a "última release" volta a ser a do Python e o
updater da v3 não acha o `latest.json`.

## Atualização do M4 (2026-11-04, rodada M4b + M2, fechamento no Windows)

Sem `todo!(` em `crates/zap-core` nem em `app/src-tauri`. Passam: `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo check --workspace --all-targets`,
`npm run check` e `vitest` (83 arquivos, 666 testes). Passam 3 vezes seguidas os aceites 1, 3, 4
(3 funções, o modo só direta existe e o `#[ignore]` saiu), prévia, reconexão e
`aceite_camadas_local`. Entraram nesta rodada: sondagem (`zap-rate::probe`,
`Bwe::on_probe_cluster`, `zap-net::Prober`, ponte do servidor de teste e publicador), modo só
direta, `Drop` do `Core` seguro, timer de 1 ms com sala ativa, MTA preso no COM, captura com
`GpuDevice` próprio.

`cargo test --workspace --no-fail-fast` deixou **4 falhas**:
1. **`zap-core --test online_aceite_2` (aceite 2, camadas por enlace)**: passou 1 de 4 execuções.
   Nas outras, A (enlace de 4 Mbps) fica na camada 2 (esperado 1) ou as camadas oscilam nos 3 s de
   estabilidade (A=2, B=0, C=2). Bug de produto na sondagem (carência, intervalo ou adoção da taxa
   do grupo), não do teste. É o que impede fechar o M4 (`core-online-22` e `-27`).
2. **`zap-encode --test windows tamanho_medio_nao_chave_software`**: 42199 B contra o esperado
   8333..24999 B. O MFT de software (Microsoft H.264) passa de 2,5× o nominal com ruído; o de
   hardware passa. Falta um limite próprio para o software ou um controle de taxa melhor.
3. **`zap-media --lib win_video::tests::desligar_camada_para_de_emitir`**: instável sob carga
   (passou isolado). A duplicação de tela só entrega quadros quando a tela muda e o teste exige
   quadros em 1 s. Os testes `win_video` pulam sem GPU com Video Processor ou sem monitor.
4. **`zap-core --lib engine::tests::set_active_room_pega_e_solta_timer`**: teste desatualizado
   (`create_room` já pega o timer). **Corrigido** nesta rodada; passa.

Também aberto: a **bancada** fora da meta (ver seção 2 acima); `vite build`, clippy do alvo
`windows-gnu` e o binário no X virtual não foram rodados.

### O que o servidor de produção precisa mudar

O servidor de produção (repositório privado) segue o contrato antigo. Para o app novo funcionar
como no servidor de teste:
1. **Sondar quem assiste pela ponte** (`docs/api/balanceamento.md` §4 e `docs/api/midia.md`):
   rajadas de sondas de 200 ms para cada espectador abaixo do teto, alvo `1,5 × (kbps da camada
   seguinte + reserva)`, a cada 5 s e dobrando até 60 s na falha, medidas pelo `relatorio.sondas`
   (`[cluster, seq, envio_us, chegada_us, bytes]`). Sem isso quem assiste pela ponte de produção
   fica preso na camada 2. A referência em código é `zap-rate::probe` e `zap-net::probe`, usada
   pela ponte do `zap-test-server`.
2. **Tolerar 5 s de queda da ponte** (`docs/api/midia.md`): quando a conexão QUIC de um
   participante cai, esperar 5 s antes de tirar as trilhas e as assinaturas dele da sala; a nova
   conexão do mesmo participante nesse prazo substitui a antiga e nada é removido (referência:
   `TOLERANCIA_QUEDA` em `zap-test-server/src/bridge.rs`). O app reconecta a ponte sem perder
   a sala.
3. Conferir que `camadas.pedidas`, `keyframe.pedir` e `camada.atual` seguem só pelo controle QUIC
   da ponte, e que o `relatorio` sai quando houve chegadas, sondas ou perdas.

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

1. Estabilizar o aceite 2 (`docs/planos/m4b-pendencias.md`, seção 9; `Bwe`, `Prober` e a ponte do
   servidor de teste).
2. Decidir o limite do teste de tamanho do codificador de software e o teste `win_video` instável.
3. Bancada: gerar movimento na tela durante a medição e investigar conversão/codificação lentas e
   quadros descartados (contenção do D3D11, relógio da GPU, sessões NVENC).
4. Depois disso, o fechamento (`core-online-27`) e o servidor de produção (acima).
- Disco: o `target/` já passou de 25 GB numa sessão. O perfil de desenvolvimento gera bem menos
  agora, e o `CLAUDE.md` manda rodar `cargo clean` se encher.

## O que foi verificado e como

- **Linux (CI)**: `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace` (inclui
  os testes de aceite no simulador de rede e os de sala com mídia sintética), e
  `cargo check/clippy --target x86_64-pc-windows-gnu` (o código do Windows compila e os testes
  linkam com MinGW). Front-end: `tsc`, ~600 testes vitest, `vite build`. Isso valia antes do M4;
  o fechamento atual foi no Windows (ver "Atualização do M4").
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
