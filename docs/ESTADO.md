# Estado da v3 (Rust + Tauri)

Atualizado em 2026-11-12 (fases 1, 2 e 3 do plano de contas, planos e câmera prontas no app, sem
lançar; o servidor com todo o plano em produção e o Supabase com as migrations; ver "Contas,
planos e câmera" abaixo). Antes: v3.2.1 (correções de áudio e música da sala); em 2026-11-05,
depois da rodada 4 (M2b: conversão NV12 pela CPU), do conversor no
Diagnóstico e do CI. Todo o trabalho (rodadas M4/M4b/M4c/M2b) está no `master`, que é a única
branch: a `m4-online` e as `claude/*` antigas entraram nele e foram apagadas.

## Antes de qualquer push no `master`

- **O app publicado agora é a v3** (primeira release: `v3.0.0`, 2026-11-05, pelo `release-v3.yml`
  ao enviar a tag). O `build.yml` do app Python foi desligado (só roda à mão): antes, todo push no
  `master` publicava uma Release do Python marcada como a última, e a tag `v3.*` também o disparava.
  Para lançar uma versão: ajuste `version` em `Cargo.toml` (workspace), `app/src-tauri/tauri.conf.json`,
  `app/package.json` e o `VERSAO` de `App.tsx`/`ConfigSobre.tsx`, e envie a tag `v3.X.Y`
  (`v3.X.Y-beta.N` sai no canal beta). Quem ainda usa o app Python reinstala à mão.
- **O CI da v3 (`rust.yml`) passa com o M4 pronto** (Linux, Windows e front-end; ver "CI da v3"
  abaixo). Rode-o antes de mexer no `master` se a mudança for grande: aba Actions, "Run workflow".

## Pendente no servidor de produção (`stremaZapi`)

**Música da sala no modo Online** (decisão do dono em 2026-11-06: fica documentado aqui e ele faz
depois). O app já está pronto desde a **v3.2.1**: no Online o servidor é a fonte da fila, e
enquanto o de produção não tiver a música, a aba **Música** de uma sala Online avisa que o
servidor precisa ser atualizado. Nas salas Locais a música já funciona (v3.2.0).

O que fazer no repositório `stremaZapi`:
1. Subir o `rev` de `zap-proto`, `zap-rate` e `zap-net` no `Cargo.toml` para o commit da v3.2.1
   (`9f32488`), que traz `zap_proto::music` (`MusicQueue`, `needed`, `is_video_id`).
2. Uma `MusicQueue` por sala. Tratar `musica.adicionar`, `musica.remover`, `musica.pular` e
   `musica.info` que chegam pelo WebSocket de quem fez `sala.entrar` (fora da sala: `erro`
   `proibido`). O servidor dá o id do item, e quem pôs é o participante que mandou. Pedido recusado:
   `erro` com `codigo: "musica_invalida"` só para quem pediu. `musica.remover` do item atual só vale
   para quem pôs ou para o dono da sala.
3. Mandar `musica.estado` (`MusicQueue::msg` com o número de membros da sala) para todos da sala:
   - no `sala.entrar`, depois do `sala.estado` e do histórico do chat;
   - a cada pedido que mudou a fila;
   - quando um membro sai (`MusicQueue::left`: o voto sai junto e o limite cai);
   - a cada 10 s com música tocando.
4. Um laço de 1 s chamando `MusicQueue::tick` (avança no fim do vídeo). O contador do reenvio de
   10 s é do servidor ou da sala, nunca um `static` do processo.
5. Testar como o `zap-test-server` (os testes `musica_*` de `src/signaling.rs`) e publicar. O push
   reinicia o servidor e zera as salas abertas.

Modelo pronto para copiar: `crates/zap-test-server/src/music.rs` e os pontos que o chamam
(`signaling.rs`, `rooms_join.rs::leave_room`, `ticker.rs`). Contrato: `docs/api/mensagens.md`,
"Música da sala". Teste do app com o servidor de teste: `musica_numa_sala_online_passa_pelo_servidor`
(`crates/zap-core/src/engine_music.rs`).

## Contas, planos e câmera (plano `docs/planos/contas-planos-camera.md`)

Trabalho conjunto com a sessão do servidor (`streamazap-servidor`). Fases na §15 do plano.

- **Contrato:** fechado nos commits `ecd1b1a` e `48f9b9b` (câmera solta ou no canto, `zap_proto::plan`,
  conta e plano no `/v1/info`, `POST /sessoes` e `renovar`, erros `conta_necessaria` e
  `plano_insuficiente`). O servidor usa os crates do app no `6814c1c` (fase 3) e está em
  produção desde 2026-11-12.
- **Fase 1 no app (v3.3.0), pronta e sem lançar:**
  - J: procura atualização na abertura e a cada 6 h, baixa sozinho e instala fora de sala (app
    minimizado ou PC parado há 2 min), com aviso de 30 s e "Adiar 1 h"; "Atualizado para a vX" na
    tela Salas; opções em Configurações → Sobre;
  - A: modo Online desligável (padrão desligado; quem já tinha senha continua ligado). Desligado, o
    app não abre conexão com o servidor e a interface não mostra nada do Online;
  - B1: criar sala sem transmitir ("Nada por enquanto");
  - B2: câmera com qualidade própria (720p30 a 1080p60, limitada ao modo real da câmera), trocada ao
    vivo;
  - B3: webcam como fonte vira a trilha `camera` de verdade (uma câmera por pessoa);
  - B4: câmera no canto escolhido ou solta, escolhida por quem transmite e trocada ao vivo; quem
    assiste pode soltar ou pôr no canto só para si.
- **Falta verificar na fase 1** (não dá sem o hardware ou sem lançar):
  - câmera de verdade: modos do Media Foundation, troca de qualidade ao vivo, troca de dispositivo;
  - bancada de 1080p60 na câmera (se não passar, a opção 1080p60 da câmera sai, §5 B2 do plano);
  - atualização sozinha pelo canal beta (instalar fora de sala e reabrir em "Atualizado para a vX");
  - o jeito da câmera numa sala Online depende do servidor repassar o campo `camera` (B4 do servidor).
- **Para lançar a v3.3.0:** subir a versão (ver "Antes de qualquer push") depois dos testes acima.
- **Fase 2 no app (frente D, login com conta), pronta e sem lançar:**
  - login pelo Supabase (`supabase-js` com PKCE) com Google, Microsoft e e-mail com código de 6
    dígitos; contas vinculadas (desvincular só com 2 ou mais), sair e excluir conta;
  - a sessão do Supabase fica no Gerenciador de Credenciais do Windows (comandos `auth_store_*`,
    em pedaços de até 1000 caracteres, porque cada credencial aceita no máximo 2560 bytes); nada
    no `localStorage` nem no `settings.json`;
  - volta do navegador pelo deep link `streamazap://auth/callback` (plugin novo; o instalador NSIS
    registra o esquema). Com o app fechado, a URL espera numa fila até o login ligar;
  - o token vai para o núcleo (`online_set_account`): entra no `POST /sessoes` só se o servidor
    tiver `conta`; trocar o token renova sem derrubar a sala; outra conta ou sair reinicia a sessão;
  - Configurações → Conexão → **Conta** (só com o Online ligado e o servidor com `conta`) e a
    **Senha do servidor** logo abaixo, enquanto o servidor pedir;
  - Salas → Online pede a senha do servidor ou a conta ali mesmo quando o login falha por
    `senha_servidor` ou `conta_necessaria` (o plano v6 trocou a decisão de pedir a senha só nas
    Configurações);
  - CSP liberando `https://*.supabase.co` e `wss://*.supabase.co`: servidor próprio precisa usar
    projeto do supabase.co;
  - erros do Supabase aparecem em português (pelo código do erro).
- **Falta verificar na fase 2** (precisa do projeto Supabase e do app instalado):
  - login Google, Microsoft e e-mail de ponta a ponta contra um projeto Supabase de verdade;
  - deep link com o app instalado pelo NSIS, com o app aberto e com o app fechado;
  - login mantido depois de fechar e abrir o app (sessão lida do Gerenciador de Credenciais);
  - renovar com um JWT de verdade do Supabase (os testes usam o token falso do servidor de teste).
- **Servidor: todo o plano em produção desde 2026-11-12** (push de `2a42530`, `e2a3f8f` e `0ae91c3`
  pelo Gitea; deploy e workflow `supabase` verdes):
  - `2a42530`: E fase 2 (sessão com conta: JWKS do Supabase, `requer_conta`, `renovar`);
  - `e2a3f8f`: B4;
  - `0ae91c3`: E fase 3 (limites de plano), F (registros de acesso), C (pasta `supabase/`, com a
    parte social; formato do Realtime no `supabase/README.md` §4b) e os crates do app no
    `6814c1c`, com um teste de compatibilidade que lê o `plano_insuficiente` do servidor com o
    `ControlMsg` do app (pelo WebSocket e pelo QUIC);
  - Supabase: as 6 migrations aplicadas e `excluir-conta`, `social-convidar` e `social-chamar`
    publicadas. Sem aviso de `pg_cron` indisponível no log;
  - o job `banco-real` (não bloqueia) falhou em 11 testes, todos com "permission denied to set
    role supabase_auth_admin": é o harness de teste no Supabase local, não o banco de produção.
    Consertado no `c926508` (o CI dá ao `postgres` o direito de trocar para o
    `supabase_auth_admin`, sem herdar nada dele): no run seguinte, o `banco-real` passou com 122
    de 123 testes (0 falhas, 1 que só roda no PGlite), incluindo os 8 do hook `plano_no_token` e
    o asaas de ponta a ponta. O hook já tem prova num Postgres de verdade;
  - VPS arrumada no mesmo dia: o `deploy.sh` novo instalado, `ZAP_SUPABASE_URL` e
    `ZAP_SUPABASE_CHAVE` no `stack.env` (`ZAP_REQUER_CONTA` de fora até a fase 6) e o servidor
    recriado. Conferido de fora: `/v1/info` traz `conta {provedor: supabase, url, chave}` com
    `requer_conta: false`, `/v1/admin/metricas` sem token responde 401, e a JWKS do projeto
    (`uwstanonwupdnmzdlqcu.supabase.co`) tem chave ES256;
  - O push do servidor vai pelo Gitea (`git.heitorcanhizares.net`, que leva ao GitHub pelo push
    mirror). No Git Bash a credencial falha; no PowerShell funciona. A porta 22 da VPS fecha
    por alguns minutos depois de muitas conexões seguidas: usar uma conexão só.
- **Com o dono, para o login funcionar de verdade:**
  - projeto do Supabase criado, os 8 secrets do CI do servidor cadastrados e a VPS com a conta
    ligada (2026-11-12);
  - painel (`supabase/README.md` §2 do servidor): hook `public.plano_no_token` (passo 11, já
    possível e provado no `banco-real`) e o que faltar dos passos 2 e 5 a 12 (`streamazap://auth/callback` nas Redirect
    URLs, provedores, SMTP, Realtime só privado); `ZAP_CHAVE_SERVICO` nos secrets das funções.
- **Fase 3 do app (planos e cadeados), pronta e sem lançar** (plano em
  `docs/planos/fase3-app-planos.md`):
  - `zap-proto`: o `erro` traz os extras (`campo`, `plano`, `tentar_em_ms`, `protocolo_min`/`max`),
    tolerantes;
  - `zap-net`/`zap-core`: o `trilha.publicar` vai com `ref` (= `local_id`), e um `erro` com esse
    `ref` fecha o pedido na hora (antes: 5 s de espera);
  - `zap-core` (`plan_gate`): calcula as qualidades travadas pelo plano da sessão Online
    (`quality_locks` no snapshot) e confere o plano antes de mandar ao servidor (publicar, trocar
    qualidade, criar e entrar em sala). A recusa vai para `plan_notice`. A câmera liga no melhor
    preset que o plano permite, sem mudar a configuração;
  - interface: cadeado com o selo do plano necessário; "Vire Transmissor" para o Espectador;
    convite do Espectador para a conta sem plano; aviso de `plano_insuficiente`; plano na Conta
    ("beta gratuito (Full HD)" sem cobrança); tela de planos com preços e `assinar`. Preço, "Ver
    planos" e a tela de planos só aparecem com `cobranca_ativa`; uma falha ao ler conta como
    cobrança desligada;
  - mock do navegador: `?plano=nenhum|espectador|hd|fullhd` e `?cobranca=1`.
- **Verificação da fase 3** neste PC: fmt, clippy, `cargo check --all-targets`, `npm run check` e
  `npm test` (1240) passam. Os fluxos foram conferidos no navegador com o mock: cadeados no `hd`,
  "Vire Transmissor", convite, assinar, "Já assinei, atualizar", e nenhum preço sem cobrança.
  `cargo test --workspace` passa, menos dois testes de tempo do hardware em crates que esta fase
  não tocou: `zap-capture` `captura_nao_segura_o_dispositivo` (p95 de 10,8 ms contra 5 ms) e
  `zap-encode` `latencia_hardware` (p95 de 12,8 ms contra 12 ms). Os dois falham também rodando
  sozinhos neste PC.
- **Falta testar na fase 3, com o Supabase de verdade:**
  - ler `config_publica` e `assinaturas` reais;
  - `refreshSession` trazendo o claim novo;
  - roteiro do sandbox do Asaas;
  - evento de foco no WebView2 instalado.
- **Próximo no app:** fase 5 (social, regras no §12 do plano); só é testada de verdade com o
  projeto Supabase.

## Resumo

| Marco | Situação | O que tem |
|---|---|---|
| M0 | ✅ | Plano mestre (`docs/arquitetura-v3.md`), API (`docs/api/`), design (`docs/design/`), hierarquia de agentes (`.claude/`), workspace e CI |
| M1 | ✅ | `zap-proto` (protocolo 4), `zap-rate` (estimador de banda, balanceamento por espectador, suavização, simulador), `zap-net` (QUIC com prazo por quadro, relatórios, descoberta) |
| M2 / M2b | ✅ na GTX 1060 (bancada release: camada 0 a 60 fps, p95 somado 6,5 ms na mediana de 5; meta de 8 ms atingida). M2b: conversão NV12 pela CPU de reserva, 60 fps também por ela (p95 somado 11,9 ms; a leitura lenta era a espera pelo quadro capturado, que o caminho da GPU também tem; a cópia em si fica em ~1 ms) | `zap-gpu`, `zap-capture`, `zap-encode`, `zap-audio`, `zap-media`: captura DXGI/WGC/câmera, NV12 na GPU, H.264 por Media Foundation, áudio por app, Opus, pipelines |
| M3 / M3b | ✅ | `zap-core` (sala Local, publicação, assinatura, playout), app Tauri (`app/src-tauri`), interface React fiel ao design (`app/src`) ligada ao núcleo |
| M5 | ✅ (primeira release `v3.0.0`; falta ver uma atualização de verdade, que só vem com a próxima versão) | Atualização assinada (instala ao fechar, nunca reinicia numa sala, canais estável/beta), firewall por COM, diagnóstico real, `release-v3.yml`, instalador NSIS que remove o app Python |
| **M4** | 🟡 **pronto no app** | Modo Online (núcleo, Tauri, telas, modo só direta, sondagem em degraus) com todos os aceites passando (aceite 2: 5 de 5, e 2 de 2 com carga); falta o servidor de produção seguir o contrato novo e um teste entre dois PCs |
| M6 | 🟡 | A v3 já é o app publicado (o `build.yml` do Python foi desligado); falta remover `streamazap/` |

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
`default_server: None`, então nenhum teste fala com o servidor de produção. O servidor **exige senha**
desde 2026-11-05 (`requer_senha: true`): quem tem a senha digita em **Configurações → Conexão**.
O app não pede a senha sozinho ao receber `401 senha_servidor`, e isso é intencional (decisão do
dono em 2026-11-06).

### 2. Teste num PC com GPU (M2)

Feito neste PC (Windows 10, GTX 1060, NVENC): captura DXGI, conversão NV12 e H.264 por hardware
rodam, e os testes de `zap-capture`, `zap-gpu`, `zap-encode` e `zap-media` passam. **A bancada bate
a meta** (rodada 3): camada 0 a 60 fps, p95 somado de 6,5 ms na mediana de 5 execuções (meta 8 ms).
Antes eram 25,9 fps e 72 ms. A causa era a captura DXGI segurar o lock do dispositivo D3D11
durante `AcquireNextFrame(100 ms)`, travando conversor e codificadores; agora ela usa
`AcquireNextFrame(0)` e espera o `WaitForVBlank`. A bancada gera movimento sozinha (`--animar`,
ligado por padrão) e avisa quando a tela fica parada. Números em
`docs/planos/m4c-estabilizacao.md` (seção 10). Ainda falta:
- testar a transmissão entre dois PCs: roteiro em `docs/planos/teste-dois-pcs.md`, com o
  executável de `cd app && npx tauri build --no-bundle` (`target/release/streamazap.exe`);
- a **conversão para NV12 pela CPU** (M2b, rodada 4) está feita e medida: o `Auto` cai para a CPU sem
  Video Processor, e a janela de tamanho ímpar passou a transmitir. A leitura lenta foi
  investigada (espera pelo quadro capturado, não a cópia; ver "Investigação da leitura" abaixo).
  O runner do CI Windows é um PC sem Video Processor de verdade: lá a cadeia cai no "Microsoft
  Basic Render Driver" e o pipeline pela CPU (captura DXGI, NV12 na CPU, H.264 de software) roda
  e passa nos testes. Falta só a medida de desempenho num PC de usuário sem VP.

### 3. Chaves do updater e uma release de teste (M5)

**Chaves prontas (2026-11-05).** O dono gerou a chave (`~/.tauri/streamazap.key`, só na máquina
dele), a pública está em `app/src-tauri/tauri.conf.json` (`plugins.updater.pubkey`) e os segredos
`TAURI_SIGNING_PRIVATE_KEY` e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` existem no GitHub. Falta testar
uma release de verdade pelo `release-v3.yml` (tag `v3.*`), mas só depois do passo 4 abaixo
(o `build.yml` parar de publicar o Python), senão a "última release" volta a ser a do Python.

### 4. Troca do app publicado (M6)

Feito em 2026-11-05: o `build.yml` não publica mais o Python e a `v3.0.0` é a última release. O dono
decidiu não migrar quem usa o Python (eles reinstalam à mão). Falta remover `streamazap/`, o
`build.yml` e o `ci.yml` do Python.

## Correções de uso (2026-11-05, depois do primeiro uso do app)

Problemas que o dono achou usando o app, corrigidos direto (sem a hierarquia):
- **Prévia das fontes** (Nova sala e painel da sala): a miniatura era um degradê falso
  (`sources::thumbnail`) e a tela chamava `source_thumbnail` com o parâmetro errado. Agora é uma
  foto de verdade (`zap_media::snapshot`: captura um quadro, lê na CPU e reduz para RGBA com
  tarjas), uma por vez, com a fonte selecionada refotografada a cada 2 s. O monitor que já estou
  transmitindo não tira foto (o Windows deixa uma duplicação por processo): fica a cena de exemplo.
- **Prévia ao vivo do que transmito**: `watch` da própria trilha virou prévia local
  (`VideoPublisher::set_local_preview`): a camada mais leve vai direto do publicador para a
  `PlayoutHub`, sem rede, começando num keyframe. O cartão Você mostra a tela e a câmera no canto,
  e dá para esconder a prévia. `RoomView.me_id` traz o id de participante (no Online não é o
  `install_id`).
- **Trocar dentro da sala**: painel "Transmissão" (`SalaTransmissao`) com fonte, som, qualidade e
  câmera; usa `share_update`. `ShareView.request` traz o pedido em vigor.
- **Só o som**: `ShareRequest.source = "none"` publica só a trilha de áudio (exige um som).
- **Câmera**: `set_camera` aceita o id da lista (`camera:<id>`, antes falhava) e, sem câmera
  escolhida, usa a primeira; o painel e a Nova sala escolhem o dispositivo; "webcam no canto" liga a
  câmera de verdade.
- **Sala aberta depois de sair** (Online): o dono que sai agora encerra a sala
  (`DELETE /v1/salas/{codigo}`); antes só saía e o servidor a listava por até 2 min.
- **Textos**: lista de salas, abas de fonte, barra de controles e cartões da sala na janela mínima
  (1100×700) sem quebrar nem vazar.

Segunda leva (mesmo dia):
- **Quem entrava na sala não via nem ouvia nada** (a imagem parada de "sem transmissão"): a
  interface só fazia `watch` dos tiles que já estavam no snapshot, e o núcleo só cria um tile depois
  do `watch`. Ninguém pedia o primeiro. Os testes não pegavam porque o mock já vem com tiles. Agora a
  `SalaScreen` deriva as transmissões de `room.tracks` (`streamsDaSala`) e assina todas sozinha
  (`useAssinaturas`): uma assinatura e um canvas por trilha, tile fixo por trilha
  (`tileDaTrilha`).
- **Várias transmissões**: cada uma vai para a **tela principal** (grade lado a lado; a câmera de
  quem está com a tela ali vai no canto dela) ou para a **faixa de baixo** (camada 2), ou fica
  **fechada** (sem receber). Mudar de lugar só pede outro tamanho no mesmo tile e move o canvas no
  DOM (`CanvasSlot`), sem reabrir a assinatura nem esperar keyframe. O cartão **Você** só aparece
  quando eu transmito.
- **Só o som para quem assiste**: o áudio só vinha junto com um vídeo assinado, então quem
  compartilhava só o som não era ouvido. `watch` aceita a trilha de áudio (assinatura só de áudio) e
  `RoomView.tracks` passou a trazer `kind: "audio"`. No publicador, `ajustar_audio` mantém o áudio da
  conexão ligado enquanto um vídeo assinado pede áudio ou a trilha de áudio está assinada direto (e
  desliga quando ninguém quer: antes continuava chegando depois de fechar tudo). No `Router`, o áudio
  é contado (`acquire`/`release`): fechar a câmera de alguém não corta o som que vem com a tela.
- **Corridas do `watch`**: um `watch` da mesma trilha enquanto a primeira ainda conecta só guarda o
  tamanho novo (antes abria uma segunda assinatura, e o `cancelar` da primeira derrubava a segunda
  no publicador); `Router::unregister_if` não deixa uma assinatura velha apagar o vídeo da nova.
- **Volume geral**: a régua e o mudo da barra não faziam nada; agora `set_master_volume`.
- **Controles da chamada** somem com o mouse parado (2,5 s; `useOcioso`) e voltam ao mexer o mouse
  no vídeo ou apertar uma tecla; ficam visíveis com o mouse ou o foco do teclado neles.
- **Sala com senha sem prévia** na lista: nem a prévia real (`room_preview`) nem a cena de exemplo.
- **Câmera nas configurações**: botão **Testar câmera** com a prévia ao vivo (`camera_preview`:
  abre a câmera só para a interface, sem sala; com a câmera ligada na sala, mostra a dela).

Terceira leva (depois da v3.1.0):
- **Minha transmissão na tela principal**: as minhas trilhas de vídeo entram no modelo das
  transmissões (`Stream.mine`); sem vídeo dos outros, a minha vai sozinha para a principal. O cartão
  Você mostra as minhas que estão na faixa e tem o botão de subir. A prévia local escolhe a camada
  pelo tamanho pedido (`publish::video::layer_for`): a mais leve na miniatura, a camada 1 na principal
  (não liga a 0 só para eu me ver).
- **Rolagem e tamanhos**: barras de rolagem finas e escuras (`theme.css`); na lista de salas só a
  lista e o detalhe rolam (topo e rodapé fixos); na sala o vídeo cabe na altura da janela (16:9,
  com a faixa de baixo) e o chat rola sozinho, sem a página crescer com as mensagens; nas
  configurações a coluna fica centrada com até 1280 px. A barra de controles fica compacta (uma
  linha) quando o vídeo tem menos de 700 px de largura (container query em `.zp-palco`).
- **Ocultar miniaturas** (depois da v3.1.1): o X de qualquer miniatura da faixa (a minha ou a de
  outra pessoa) a troca por um botão pequeno numa linha embaixo ("Dudu · Janela", "Você"), que a
  reabre. A de outra pessoa fechada não é recebida (como antes); o cartão Você oculto
  (`streamazap.voceOculto` no localStorage, lembrado entre salas) para de assinar a minha miniatura.

Quarta leva (áudio, depois da v3.1.2):
- **Lista de apps do som**: só mostrava quem estava tocando naquele instante e no dispositivo
  padrão. Agora `zap_audio::list_audio_apps(pids_com_janela)` junta as sessões de som de todas as
  saídas ativas (tocando ou quietas) e os processos com janela, um item por app: a raiz da árvore com
  o mesmo executável (`app_tree::root_of`; o Chrome e o Discord tocam num processo filho), com o nome
  da descrição do executável ("Google Chrome"). `SourceItem.sound` diz `playing`/`ready`/`silent`;
  a seção Som separa "Com som" de "Outros apps abertos" e tem **Atualizar lista**.
- **Som que parava e não voltava** (quatro causas):
  - quem assiste descartava como "atrasado" todo o som de quem parou e voltou a transmitir (a
    contagem `seq` recomeça do zero e o `AudioRx` é por participante); agora um recuo de mais de
    100 pacotes é um áudio novo (`RECOMECO`);
  - a saída abria uma vez só: com o fone desconectado ou o padrão do Windows trocado, o cpal
    encerra o stream e nada reabria. `AudioOut::maintain` (a cada 0,5 s) reabre no padrão de agora
    quando o stream dá erro, para de pedir som por 3 s ou o padrão muda; o `MixerCore` é emprestado
    ao callback e volta quando ele acaba, então as trilhas continuam. Sem saída no início, abre
    quando aparecer (antes ficava mudo até reiniciar o app);
  - o mixer de saída tem 64 trilhas e ninguém saía dele (cada entrada na sala é um id novo): as
    assinaturas agora contam o uso (`AudioRegistry::hold`/`unhold`) e o último solta a trilha; sair
    da sala limpa tudo;
  - na captura, um app que falhava uma vez nunca mais era tentado: agora volta depois de 1, 2, 4,
    5 s, e um app fechado e aberto de novo é achado pelo executável (`app_tree::root_by_exe`).

Música da sala (depois da v3.1.3; plano em `docs/planos/musica-da-sala.md`):
- Fila de vídeos e lives do YouTube tocada junto: cada um toca no próprio player (embed oficial,
  controlado por `postMessage`, sem o script do YouTube na página) na posição da sala. Pula com
  votos de metade da sala; quem pôs e o dono pulam direto. Quem pega anúncio volta para a
  posição da sala quando ele acaba (a sala nunca espera).
- Núcleo: `zap_core::music` (fila do dono, pura), mensagens `musica.*` (`docs/api/mensagens.md`),
  `zap_net::youtube::oembed` (título; recusa vídeo que não toca fora do YouTube), laço de 1 s do
  dono, `RoomView.music`/`owner_id`.
- Interface: aba **Música** no painel e a música como transmissão (principal, faixa ou fechada),
  com os controles fora da área do player (termos do YouTube). CSP com `frame-src` do YouTube e
  `img-src` do `i.ytimg.com`.
- **Online (depois da v3.2.0):** a fila foi para `zap_proto::music::MusicQueue` (o servidor usa o
  `zap-proto`), e no Online o servidor é a fonte. O app manda `musica.*` pelo WebSocket e guarda o
  `musica.estado` que volta (`online/ws_apply.rs`). O `zap-test-server` já faz o papel do
  servidor (`src/music.rs`), com o teste de duas pessoas `musica_numa_sala_online_passa_pelo_servidor`.
- **O servidor de produção ainda não tem a música**: ver "Pendente no servidor de produção" no
  topo deste arquivo.
- Falta testar com dois PCs de verdade (sincronia, anúncio, live) e no app instalado (o embed com
  a origem `http://tauri.localhost`). Na prévia do navegador o player tocou e respondeu aos
  comandos.

## Atualização da rodada 4 (M2b, 2026-11-05, fechamento)

Conversão para NV12 pela CPU (plano `docs/planos/m2b-conversao-cpu.md`). `zap-gpu` ganhou o
`CpuScaler` (bilinear igual ao Video Processor da NVIDIA dentro de 1 nível, várias threads),
stagings de leitura/escrita que não seguram o lock, `format_support`, `create_basic` e BGRA de
tamanho ímpar no `Converter`; `zap-media` escolhe GPU ou CPU por camada (`ConversionSpec`, `Auto`
padrão) e a bancada ganhou `--conversao`. `zap-core` e o app não mudaram. Verificação: fmt, clippy,
check e `cargo test --workspace --no-fail-fast` sem falha (no fechamento corrigi um `dead_code` no
exemplo `cpu_nv12`); testes novos 3 vezes seguidas verdes; `online_aceite_2` e `aceite_camadas_local`
passam.

Bancada release, 1080p60, monitor 1, ociosa (menos execuções que o plano: `cpu` 3, `auto` 1,
`cpu --software` 1):

| Modo | fps 0/1/2 | conversão camada 0 média/p95 | leitura p95 | codificação 0 média/p95 | p95 somado | CPU do processo | Descartes de captura |
|---|---|---|---|---|---|---|---|
| `auto` (GPU) | 60,1 / 30,0 / 15,0 | 0,06 / 0,09 ms | — | 3,68 / 7,61 ms | 7,7 ms | 15% | 14 |
| `cpu` (NVENC), mediana de 3 | 60,0 / 30,0 / 15,0 | 5,15 / 9,56 ms | 8,5 ms | 2,08 / 2,29 ms | 11,9 ms (5,7 a 14,1) | 68% | 15 |
| `cpu --software` | 60,1 / 30,1 / 15,0 | 5,51 / 9,26 ms | 8,0 ms | 5,90 / 7,29 ms | 17,7 ms | 215% | 1 |

`cpu_nv12` (4 threads): 1080p 1,36 ms, 720p 2,33, 360p 0,75, 720p + 360p 2,98, as 3 4,18, NV12
720p para 360p 0,37 (todas dentro da meta; 1 thread: 3,12 / 5,69 / 1,71 / 7,39 / 10,57 / 0,66).

**Meta do caminho de CPU:** atingida em fps, espera de lock (p95 0,155 ms) e CPU extra (+0,53
núcleo); o p95 somado de 12 ms bateu na mediana, mas uma de 3 execuções deu 14,1 ms; **a leitura da
origem (p95 8,5 ms, meta 2) não foi atingida**. Hipótese: o `CopyResource` espera a fila da GPU
compartilhada com NVENC, animação e captura. Portão da entrega de memória ao codificador de software:
**aberto** (codificação de software 5,9 ms). Detalhes na seção 10 do plano.

Conferência do orquestrador (seção 11 do plano): os 10 testes do pipeline de vídeo
(`win_video::tests`) **pulavam em silêncio quando rodavam em paralelo**, porque o Windows só deixa
uma duplicação do mesmo monitor por processo. Agora entram numa fila (trava `MONITOR_1`) e rodam
de verdade (3 de 3, nenhum pulou). Duas execuções a mais da bancada `cpu` (60/30/15 fps; p95
somado 26,1 ms na primeira, logo depois da compilação, e 10,6 ms na segunda) mantêm a mediana das 5
em 11,9 ms.

**Investigação da leitura** (seção 12 do plano). A leitura lenta não é a cópia, e sim a espera
pela GPU terminar o quadro capturado (a imagem que o DWM compõe e as cópias da própria captura).
Com consultas de evento antes e depois da cópia, a espera deu média 2,0 / p95 6,8 ms e a cópia
de 8,3 MB 1,0 / 1,8 ms. Quando a captura espera a imagem do DWM antes de entregar, a espera da
leitura cai para p95 0,8 ms. Durante a medida, o driver também desce a GTX 1060 de P0 para P8
(memória de 4608 para 405 MHz), o que deixa tudo mais lento. O caminho da GPU tem a mesma espera,
escondida na codificação (lá ela varia de 2 a 9 ms; na CPU fica estável em ~2 ms). A meta de 2 ms
vinha de um protótipo que lia uma textura parada; agora vale para a cópia, que já a cumpre.
`ReadbackTiming` ganhou `frame_wait_us`, `PipelineStats` ganhou `cpu_frame_wait` e a bancada mostra
a espera pelo quadro. Medida final com a tela mais pesada (4,4 a 4,8 Mbps, contra 1,4 antes),
3 execuções por modo: GPU com p95 somado de 12,3 / 13,7 / 14,0 ms (fora da própria meta de 8,
também por causa da espera), CPU 20,6 / 21,5 / 23,3 ms, leitura média ~5,2 ms, dos quais ~4,0 são
espera pelo quadro. Nas mesmas condições, a CPU acrescenta ~8 ms ao p95 (cópia, conversão e
subida).

**Conversor no Diagnóstico.** A tela Diagnóstico ganhou a checagem "Placa de vídeo": cada placa que
importa (a padrão e as que dirigem monitores, sondadas com `zap_media::probe_video_adapters`, que cria
de verdade o conversor do Video Processor) aparece com "conversão na GPU" ou, em amarelo, "conversão
na CPU". Durante uma transmissão, aparece também o codificador e o conversor de cada camada
(`ShareView::conversion`, vindo de `LayerStats::converter`). O "Copiar relatório" ganhou a seção
"== Vídeo ==". Achados novos: `video_cpu_conversion` (placa sem Video Processor; no driver básico
sugere instalar o driver) e `share_cpu_conversion` (a placa tem VP, mas a transmissão caiu para a
CPU). Se o único aviso for o de vídeo, o resumo fala de processador, não de amigos.

## Atualização da rodada 3 (m4c, 2026-11-04, fechamento)

Plano: `docs/planos/m4c-estabilizacao.md`. A seção 10 é o fechamento da hierarquia de agentes; a 11
são as correções feitas direto pelo orquestrador depois dele.

**Tudo passa no Windows** (MSVC, GTX 1060):
- `cargo fmt --all --check` e `cargo clippy --workspace --all-targets -- -D warnings`;
- `cargo test --workspace --no-fail-fast`: 1380 testes, 0 falhas, 2 ignorados;
- `npm run check` e `vitest` (83 arquivos, 666 testes), rodados no fechamento; o app não mudou
  depois dele.

Repetições:

| Teste | Resultado |
|---|---|
| aceite 2 (`online_aceite_2`) | 5 de 5 sem carga, 2 de 2 com `cargo build` em paralelo. Antes das correções falhava 4 de 4 |
| aceites 1, 3, 4 (3 funções), prévia, reconexão, `aceite_camadas_local` | 3 de 3 cada |
| `zap-net`: `prazo_keyframe`, `impaired`, `probe` | 3 de 3 cada |
| `zap-net`: `online::` | 20 de 20 |
| `zap-test-server --test bridge` | 3 de 3 |
| `zap-encode --test latencia` | 2 de 2 |

O que entrou:

**Aceite 2.** Três causas:
1. O keyframe da troca de camada levava 170 ms num enlace de 4 Mbps e era resetado pelo prazo de
   150 ms. O keyframe agora tem prazo de `max(600 ms, 2 × RTT)`, e os quadros atrás dele contam o
   prazo a partir do fim dele.
2. Sobrava sobrecarga no estimador logo depois do keyframe. Isso agora tem carência de rajada
   própria (sondas e keyframes).
3. Ao voltar para uma camada antiga, o salto de `seq` virava centenas de "descartes".

Também entraram:
- sondagem em degraus, guiada pela medida do relatório do espectador;
- um `MediaSender` por conexão na ponte;
- admissão com o RTT de base e prazo com o RTT atual.

**GPU.**
- Captura sem segurar o lock do dispositivo.
- Medida por etapa: consultas de tempo da GPU e espera de lock.
- Bancada e testes com janela animada.

**Codificador de software.** As opções do `ICodecAPI` passaram a ser aplicadas antes dos tipos de
mídia, com leitura de volta. O teste de tamanho passa.

**Testes instáveis.**
- `renovacao_automatica_dispara`: relógio de renovação injetado.
- `alloc_loop::tick_com_espectador_...`: espera de 5 s.
- Proxy de apoio do `zap-net`: rajada em blocos.

**Bancada release** (mediana de 5, 1080p60, monitor 1, máquina ociosa):

| Camada | fps | kbps | Codificação média / p95 |
|---|---|---|---|
| 0 | 60,0 | 2214 | 3,69 / 6,23 ms |
| 1 | 30 | 1506 | — |
| 2 | 15 | 323 | — |

- p95 somado da camada 0: 6,5 ms. As 5 execuções deram 6,5 / 6,3 / 6,9 / 2,2 / 9,1.
- A etapa que domina é a codificação. A conversão (0,08 ms) e a espera de lock são desprezíveis.
- Descartes de captura: mediana 27 em 10 s. O limite de 1% não foi confirmado.
- Na NVIDIA o `VideoProcessorBlt` roda no motor de vídeo, e a consulta de tempo da GPU marca 0 µs.

### O que o servidor de produção precisa mudar

**Feito: os cinco itens abaixo estão em produção desde 2026-11-05 ~21:00 UTC** (commit `7841e2b` do
repositório do servidor, crates `zap-proto`/`zap-rate`/`zap-net` fixados em `51bc222`; recado da
sessão do servidor). Diferenças e detalhes:
- sondagem: um `Prober` por conexão de membro, no mesmo `MediaSender` do repasse; a necessidade leva
  a reserva do alocador (áudio + outras trilhas), não 0 como no `zap-test-server`;
- tolerância de 5 s: a ponte guarda camada, histerese, assinaturas e banda de quem caiu; a reconexão
  do mesmo participante herda isso e recebe keyframe + `camada.atual` de novo. Depois de 5 s sem
  QUIC (corte entre 5 e 6 s), mesmo com o WebSocket vivo, as trilhas e as assinaturas saem e a pessoa
  segue na sala;
- 21 testes no CI do servidor (inclusive as sondas levando o espectador da ponte à camada 0 e as
  quedas curta e longa). Falta rodar a prova contra a produção (`prova_producao <host>:7443 20
  estimador` no repositório do servidor; precisa da senha do servidor).

O texto original, para referência: o servidor de produção (repositório privado) seguia o contrato
antigo e, para o app novo funcionar como no servidor de teste, precisava de cinco mudanças. A
referência em código é o `zap-test-server`, com `zap-rate::probe` e `zap-net::{probe, media_tx}`.

1. **Sondar quem assiste pela ponte** (`docs/api/balanceamento.md` §4). Rajadas de 200 ms em
   degraus: 1,5 × a taxa medida, ou 1,25 × depois de saturar. A medida vem do próprio
   `relatorio.sondas`/`chegadas` do espectador. O intervalo começa em 5 s e dobra até 60 s na
   falha. Sem isso, quem assiste pela ponte de produção fica preso na camada 2.
2. **Prazo do keyframe** (`docs/api/midia.md`, "Prazo"):
   - keyframe: `max(600 ms, 2 × RTT)`;
   - quadros atrás dele: o prazo conta a partir do fim do keyframe;
   - RTT: o atual, na hora de decidir o reset;
   - admissão: com o RTT de base.
3. **Um estado de envio por conexão** de espectador na ponte. Não pode haver um por quadro, senão o
   prazo do keyframe e a admissão não funcionam.
4. **Tolerar 5 s de queda da ponte** (`docs/api/midia.md`). Quando a conexão QUIC de um
   participante cai, o servidor espera 5 s antes de tirar as trilhas e as assinaturas dele. Uma
   conexão nova do mesmo participante nesse prazo substitui a antiga. Referência:
   `TOLERANCIA_QUEDA` em `zap-test-server/src/bridge.rs`.
5. **Conferir os canais.** `camadas.pedidas`, `keyframe.pedir` e `camada.atual` seguem só pelo
   controle QUIC da ponte. O `relatorio` sai quando houve chegadas, sondas ou perdas.

### Próximos passos
1. **Modo Online, lado do app** (o servidor já está pronto, ver acima; decisão do dono em
   2026-11-05: analisar depois):
   - ~~estimador na reconexão~~ **feito (2026-11-06)**: a ponte nova começava o estimador frio
     (1000 kbps, sobe no máximo 1,5×/s), e numa reconexão o primeiro `relatorio` derrubava pela
     histerese de descida (500 ms) quem estava na camada 0/1, embora o servidor guarde a camada
     por 5 s. Agora `online::reconnect::connect_bridge_for` lê `PubConn::estimate_kbps` da ponte
     antiga e passa a `PubConn::connect_online`, que monta o `ReportBuilder::with_start_kbps`
     (`zap-net`). Sem pacotes o estimador fica parado, então a ponte morta ainda guarda a última
     medida; a primeira ponte da sala continua começando em `START_KBPS`. Teste:
     `ponte_nova_herda_a_estimativa_da_antiga`. Não muda nada no servidor (o `zap-net` só ganhou
     funções);
   - depois: a prova contra a produção e o teste do app com o servidor entre dois PCs.
2. Entrega de memória ao codificador de software (portão aberto); medir o caminho pela CPU num PC
   de usuário sem Video Processor.
3. M6: remover `streamazap/`, o `build.yml` e o `ci.yml` do Python.

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

### O que o servidor de produção precisa mudar (versão da M4b, superada pela da rodada 3)

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

O app está pronto e os aceites passam (ver "Atualização da rodada 3"). Falta o servidor de
produção seguir o contrato novo e um teste entre dois PCs com ele.
- Disco: o `target/` já passou de 25 GB numa sessão. O `CLAUDE.md` manda rodar `cargo clean` se
  encher.

## O que foi verificado e como

### CI da v3 (2026-11-05, `m4-online`, run 42012019146)

Primeiro push com o M4 pronto: os três jobs do `rust.yml` passaram na primeira.
- **Linux**: `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace` (1313 testes, os
  só-Windows ficam de fora; inclui os aceites do M4 com o `zap-test-server`) e
  `cargo check --target x86_64-pc-windows-gnu` com MinGW.
- **Windows** (`windows-latest`): clippy `-D warnings` e `cargo test --workspace` (1467 testes, 0
  falhas). **O runner não tem GPU de vídeo**: a cadeia do `zap-gpu` escolhe o "Microsoft Basic
  Render Driver" (sem vídeo), e por isso rodam de verdade a captura DXGI, a conversão NV12 pela
  CPU e o H.264 de software (`cpu_gera_quadros`, `cpu_duas_camadas`, `auto_cai_para_cpu_sem_vp`,
  `gpu_sem_vp_nao_cai`...). Pulam os testes que exigem Video Processor ou MFT de hardware, e a
  câmera. Um passo de resumo do job mostra a placa escolhida e os motivos de pulo.
- **Front-end**: `tsc`, vitest (83 arquivos) e `vite build`.

O que o CI pegou e foi corrigido:
- `zap-gpu`: o último passo da cadeia pegava o primeiro adaptador de qualquer tipo, que podia ser
  a mesma GPU que acabara de falhar. Agora tenta o adaptador de software, e a ordem ficou numa
  função testada sem GPU (`em_cadeia`).
- `zap-net`: `send_us_trunca` lia o relógio na ordem errada e dava -1 us (4294967295).
- `zap-capture`: os testes que duplicam o monitor rodavam em paralelo, e o segundo
  `DuplicateOutput` dava E_INVALIDARG e pulava em silêncio (3 no CI; 1 por rodada aqui, em
  `lock_dispositivo.rs`). Agora esperam a vez num mutex, como os do `zap-media`.
- `janela_animada_gera_quadros`, que pulava por causa disso, passou a rodar e falhou uma vez no CI
  do `master` (22 quadros novos/s; a meta é 30). No runner quem compõe a tela é a CPU, e a taxa
  oscila (passou na rodada anterior). Decisão do dono do projeto: a meta de 30 vale em GPU de
  verdade (aqui dá 106–121); no adaptador de software o teste exige 10. No runner deu 22 e 36.

**Aberto:** o aceite 2 Online (`online_aceite_2`) falhou 1 vez em 5 no Windows do CI (run
42019236899, primeira tentativa): A e C certos, mas **B, que assiste direto e sem limite, ficou na
camada 1 os 30 s** em vez de subir para a 0 (8000 kbps, pede 9200 disponíveis). Passou ao repetir
o job; no Linux passou todas as vezes. Hipótese não verificada: no runner sobrecarregado (build
de debug, H.264 de software a 1080p60) a sondagem em degraus não prova a banda a tempo. O aceite
não foi mexido.

Para rodar o CI de novo sem push: aba Actions, "v3 (Rust + interface)", "Run workflow".

### Outras verificações

- **App de verdade**: o binário Tauri abre num X virtual e mostra a interface com dados do núcleo
  (`docs/design/prints/app-real-salas.png`). Neste PC (GTX 1060) o fechamento de cada rodada roda
  fmt, clippy e o `cargo test --workspace` inteiro, com captura, NV12 e NVENC de verdade.
- **Windows (CI)**, desde o M2: loopback de áudio por app (WASAPI, blocos de 20 ms), Opus, listas
  de apps/janelas/monitores, firewall por COM, VPNs e o codificador de software do Media
  Foundation.

## Melhorias conhecidas (não bloqueiam)

- Codificador de hardware: trocar a espera de eventos com `sleep(1 ms)` por `GetEvent` bloqueante.
- Teste de qualidade do Opus usa complexidade 4 (produção usa 8).
- Diagnóstico/servidor: o card do servidor só fica real com o M4.
- Servidor de teste: não manda `camadas.pedidas` vazio logo depois de `trilha.publicada` (o de
  produção manda) nem eventos de sala pelo QUIC quando não há WebSocket.
- Testes de hardware no PC do dono (2026-11-05, monitor 1 em 125% + segundo monitor na GTX 960M):
  `zap-capture` `capture_monitor_entrega_quadro_em_1s` e `dxgi_entrega_quadro_nao_preto` falham
  porque o processo de teste não é DPI-aware (lista 1536×864, a captura vem 1920×1080);
  `zap-encode` `latencia_hardware` (p95 acima do limite) e `zap-gpu` `nv12_cpu_igual_gpu` também
  falham aqui. O app (DPI-aware) não é afetado. Falta tornar esses testes DPI-aware ou comparar com o
  tamanho da duplicação.
