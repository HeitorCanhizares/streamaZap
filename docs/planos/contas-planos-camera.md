# Plano: contas, planos pagos no Online, Online desligável, sala, câmera, social e atualização

Versão 3, 2026-11-06. v2: quem assiste paga um plano simbólico e vê na qualidade que for transmitida; quem transmite paga pela qualidade que manda. v3: criar sala sem transmitir, câmera como fonte de verdade, câmera solta ou no canto escolhido. v4: amigos, convites e chamar atenção (com permissão, desligáveis); atualização que baixa e instala sozinha. v5: a cobrança fica para o futuro, mas tudo pronto e desligado por uma chave (§10). v6: o Online continua fechado pela senha do servidor que já existe; criar conta é livre, mas usar o Online pede conta **e** senha. Vale para o **app** (`HeitorCanhizares/streamaZap`) e o **servidor**
(`HeitorCanhizares/streamazap-servidor`) ao mesmo tempo. Fonte do contrato continua sendo
`docs/api/` do app; o servidor sobe o `rev` de `zap-proto`, `zap-rate` e `zap-net` quando o
contrato mudar.

## 0. Decisões fechadas

| Assunto | Decisão |
|---|---|
| Acesso ao Online | **Fechado pela senha do servidor de hoje** (`ZAP_SENHA`). Qualquer um cria conta, mas sem a senha não entra no Online. A senha só sai quando o dono decidir abrir (§15) |
| Login | Supabase Auth: Google, Microsoft (Azure) e e-mail com código de 6 dígitos (OTP) enviado pelo Resend. Sem Apple por enquanto (só desktop; ver §15) |
| Contas vinculadas | Um usuário pode ter várias identidades (vínculo automático por e-mail verificado + vínculo manual pelo app) |
| Modelo | Modo **Local** continua grátis e open source, sem conta. No **Online**, todo mundo paga um plano simbólico para assistir (na qualidade que quem transmite mandar), e quem transmite paga pela qualidade que transmite |
| Planos | **Espectador** R$ 3,90/mês ou R$ 39/ano · **Transmissor HD** R$ 7,90/mês ou R$ 79/ano · **Transmissor Full HD** R$ 14,90/mês ou R$ 149/ano. Plano de transmissor já inclui assistir. Valem quando a cobrança ligar |
| Pagamento | **Fica para o futuro, com tudo pronto:** planos, limites, tabela de assinaturas, funções do Asaas (testadas no sandbox) e telas existem, mas a chave `cobranca_ativa` fica desligada. Até ligar, toda conta recebe o plano padrão do beta (`fullhd`). Quando ligar: Asaas, mensal no cartão, anual no Pix, vendido fora das lojas |
| Infra | Servidor de mídia sai da Hostinger para uma VPS com tráfego ilimitado em São Paulo, depois de teste de banda |
| Online desligável | Quem não usa o Online não abre nenhuma conexão com o servidor nem com o Supabase |
| Sala | Dá para criar a sala sem transmitir nada e começar depois |
| Câmera | Qualidade à parte da tela (até 1080p60 se a câmera suportar); a webcam como fonte vira trilha de câmera de verdade; quem transmite escolhe a câmera solta ou num dos 4 cantos da tela |
| Social (Online) | Amigos por código ou link de convite, presença, convite para sala e "chamar atenção" (som + aviso). Só entre amigos aceitos, com limite de frequência, e tudo desligável por quem recebe |
| Atualização | Três escolhas independentes: procurar sozinho, baixar sozinho e instalar (perguntar · ao fechar · sozinho quando não estiver em sala) |
| Dados guardados | E-mail e identidades (Supabase), plano, e registros de acesso (IP, porta, horário, conta, tempo transmitindo/assistindo) por 6 meses. **Nenhum conteúdo** (chat, vídeo, áudio) |

## 1. Visão geral

```
                 ┌─────────────── Supabase (sa-east-1) ───────────────┐
                 │ Auth (Google/Microsoft/OTP)  ← SMTP Resend         │
 app ──login───▶ │ tabela assinaturas + hook "plano" no JWT           │ ◀── webhook ── Asaas
     ◀─JWT────── │ Edge Functions: assinar, asaas-webhook,            │
                 │                 excluir-conta, social-*            │
 app ◀─avisos─── │ Realtime: presença, convites e chamados (amigos)   │
                 └────────────────────────────────────────────────────┘
  │ POST /v1/sessoes { acesso_conta: <JWT> }
  ▼
 servidor streamaZap (VPS ilimitada) ── confere o JWT pela JWKS do Supabase (sem chamar o Supabase)
  • REST/WS/QUIC como hoje
  • plano: entrar em sala (precisa de plano), publicar (validate_media), teto de banda por espectador
  • registros de acesso em SQLite, 6 meses
```

O servidor nunca fala com o Supabase no caminho de uma sessão: confere a assinatura do JWT com a
chave pública (JWKS, em cache) e lê o claim `plano`. Supabase fora do ar = ninguém faz login novo,
mas sessões abertas continuam.

## 2. Planos e limites

Os limites valem **só no Online**. No Local nada muda.

| Plano | Assistir | Transmitir (camada 0 de cada trilha) | Preço |
|---|---|---|---|
| `nenhum` (conta sem plano) | só a lista de salas e a prévia (camada 2, sem áudio, como hoje) | não | grátis |
| `espectador` | **tudo o que quem transmite mandar**, até 1080p60 | não (chat e música da sala, sim) | R$ 3,90/mês · R$ 39/ano |
| `hd` (Transmissor HD) | igual ao Espectador | altura ≤ 720, fps ≤ 60; tela ≤ 4000 kbps, câmera ≤ 2500 kbps | R$ 7,90/mês · R$ 79/ano |
| `fullhd` (Transmissor Full HD) | igual ao Espectador | altura ≤ 1080, fps ≤ 60; tela ≤ 8000 kbps, câmera ≤ 4500 kbps | R$ 14,90/mês · R$ 149/ano |

Regras:
- **Assistir não depende do plano**: com qualquer plano pago, o espectador recebe a melhor camada
  que a conexão dele aguenta, até a que quem transmite mandar (o balanceamento de sempre).
- **Entrar numa sala Online** (criar ou entrar) exige `espectador` ou acima. Conta `nenhum` vê a
  lista e a prévia e recebe `plano_insuficiente` ao entrar, com o convite para assinar.
- **Transmitir** (tela, janela, câmera ou só o som) exige `hd` ou `fullhd`, e a camada 0 tem de
  caber no plano.
- **Teto de banda por espectador, igual para todos: 12000 kbps.** O servidor corta o `banda_kbps`
  reportado antes do alocador. Uma tela 1080p60 grande cabe inteira; se a pessoa assiste várias
  transmissões grandes ao mesmo tempo, as outras descem de camada. É isso que deixa o preço do
  Espectador fixo: cada pessoa ocupa no máximo 12 Mbps de saída da VPS.
- Na **rota direta**, quem garante o limite de transmissão é o app de quem publica. App modificado
  consegue transmitir acima do plano por ali, mas a mídia não passa pela VPS: perde-se receita, não
  banda. Aceito.
- Plano muda no meio da sessão (pagou, venceu): vale para salas e trilhas **novas** depois da
  próxima renovação do token. Nada em andamento é derrubado.

### Enquanto a cobrança estiver desligada (beta gratuito)

- Uma tabela pública de configuração no Supabase (`config_publica`, §6) tem `cobranca_ativa`
  (começa `false`), `plano_padrao` (começa `fullhd`) e os `precos`.
- Com `cobranca_ativa = false`, o hook do token dá `plano_padrao` para toda conta. O servidor não
  muda nada: lê o claim como sempre, e os limites de §2 já valem (o teto de 12 Mbps por espectador
  inclusive, que segura o custo).
- O app lê `config_publica`: sem cobrança, não mostra preço, **Assinar** nem convite para assinar;
  em Conta aparece "Plano: beta gratuito (Full HD)".
- **Proteções do beta:**
  - **a senha do servidor de hoje continua valendo**: o servidor pede conta **e** senha
    (`--senha` + `--requer-conta`, §8). O cadastro no Supabase fica aberto; sem a senha, a conta
    existe mas não entra no Online;
  - limite de espectadores pela ponte ao mesmo tempo no servidor (`--max-espectadores-ponte`,
    §8), para a banda não estourar enquanto ninguém paga.
- Ligar a cobrança é **mudar `cobranca_ativa` para `true`** depois do checklist de §10. Quem não
  assinar passa a `nenhum` na renovação seguinte do token (em até 1 h).

### Por que esses preços (VPS ilimitada, pior caso de 8 h por dia, todos ao mesmo tempo)

Custos fixos: VPS ilimitada ~R$ 129 + MEI R$ 86 = ~R$ 215/mês. 600 Mbps úteis de saída por VPS
(a porta é full-duplex: o que quem transmite manda não disputa com a saída). Cartão no Asaas:
R$ 0,49 + 2,99%.

| Sala típica (1 transmite, 3 assistem) | Receita líquida/mês | Saída da VPS | Cabem por VPS | Paga os fixos com | Lucro com a VPS 70% cheia |
|---|---|---|---|---|---|
| Transmissor Full HD (1080p60) | R$ 23,84 | 25,2 Mbps | 23 salas | 9 salas | ~R$ 180 |
| Transmissor HD (720p60) | R$ 17,05 | 13,2 Mbps | 45 salas | 13 salas | ~R$ 330 |

- Cada espectador assistindo 1080p60 custa ~R$ 2,58 de VPS (com ela 70% cheia). Por isso o
  Espectador não pode ser R$ 2,90: o líquido seria R$ 2,32, e ele daria prejuízo. Com R$ 3,90
  sobram ~R$ 0,72 por espectador; a margem de verdade vem de quem transmite.
- Cada VPS a mais rende ~R$ 320/mês cheia a 70% (mistura de salas HD e Full HD): o modelo cresce
  somando VPS.
- No mensal, a taxa do cartão leva 16% do Espectador; no anual pelo Pix (R$ 1,99 por cobrança),
  5%. A tela de assinar destaca o anual.
- O pior caso (todos online ao mesmo tempo, 8 h por dia) é conservador: com uso espalhado no dia,
  cabe mais gente por VPS e o lucro sobe.

Os números ficam num lugar só: `zap_proto::plan` (novo), usado pelo app e pelo servidor (como
`zap_proto::music`).

```rust
pub enum PlanId { None, Viewer, Hd, FullHd }  // serde: "nenhum" | "espectador" | "hd" | "fullhd"
pub struct PlanLimits {
    pub can_join: bool,                       // entrar/criar sala Online
    pub publish: Option<PublishLimit>,        // None = não transmite no Online
}
pub struct PublishLimit { pub max_height: u16, pub max_fps: u8, pub screen_kbps: u32, pub camera_kbps: u32 }
pub const VIEWER_MAX_KBPS: u32 = 12_000;      // teto de banda por espectador, para todos
pub fn limits(id: PlanId) -> PlanLimits;
pub fn check_publish(kind: TrackKind, layers: &[LayerSpec], l: &PlanLimits) -> Result<(), PlanError>;
```

## 3. Mudanças no contrato (`docs/api/`)

Todas **aditivas**: o protocolo continua 4, e o `zap-net` ignora campos e tipos desconhecidos.

**Fechado** nos commits `ecd1b1a` e `48f9b9b` do app, com os ajustes da revisão do servidor. A fonte
é o texto de `docs/api/`; a tabela abaixo é o resumo.

| Arquivo | Mudança |
|---|---|
| `rest.md`, `GET /v1/info` | `requer_conta: bool`, `conta: { "provedor": "supabase", "url": "https://<ref>.supabase.co", "chave": "<chave publicável>" }`. Com `url` e `chave` o app cria o cliente de login, e um servidor próprio aponta para o próprio projeto. `conta` vem completo ou não vem |
| `rest.md`, `POST /v1/sessoes` | pedido ganha `acesso_conta` (JWT do Supabase). Resposta ganha `conta_id` e `plano` (`id` + limites de §2). Com `requer_senha` **e** `requer_conta`, os dois são obrigatórios: confere a senha primeiro (`senha_servidor`), depois a conta (`conta_necessaria`) |
| `rest.md`, `POST /v1/sessoes/renovar` | corpo opcional `{ "acesso_conta": "…" }`. O `renovar` **sempre** renova (Supabase fora do ar não derruba sessão aberta): sem corpo mantém conta e plano; token válido atualiza o plano ou liga a conta numa sessão que entrou só com a senha; token vencido ou inválido é ignorado; token de outra conta → `403 proibido` (trocar de conta = sessão nova). `conta_necessaria` só sai do `POST /sessoes` |
| `README.md`, erros | `conta_necessaria` (401, só no `POST /sessoes`): servidor exige conta e o pedido não trouxe JWT válido (anônimo também não vale). `plano_insuficiente` (403): ação acima do plano; traz `plano` (só o id) e, ao transmitir, `campo` (`"tipo"` = o plano não transmite nada; `"camadas"` = uma camada passou do teto) |
| `rest.md`, `POST /v1/salas` e `POST /v1/salas/{codigo}/entrar` | recusa com `plano_insuficiente` para conta `nenhum` (a prévia continua liberada) |
| `mensagens.md`, `trilha.publicar` e `trilha.atualizar` | recusa com `plano_insuficiente` para quem não é transmissor ou quando uma camada passa do plano (`zap_proto::plan::check_publish`). Sessão **sem conta** não tem limite de plano (até o servidor ligar `requer_conta`) |
| `balanceamento.md`, §2 | `banda_kbps` usado pelo alocador = `min(relatorio, 12000)` no Online com ponte |
| `balanceamento.md`, §1 | tabela da câmera com as 4 escadas novas (§5, B2) |
| `mensagens.md`, `trilha.publicar`, `trilha.atualizar` e `Trilha` (em `sala.estado`, `trilha.publicada`, `trilha.atualizada`) | trilha `camera` ganha o campo opcional `camera`: `{"modo": "canto", "canto": "sup_esq" \| "sup_dir" \| "inf_esq" \| "inf_dir"}` ou `{"modo": "solta"}`. Ausente = canto inferior direito (§5, B4). Só vale na trilha `camera` (o servidor descarta nas outras); ausente no `trilha.atualizar` = sem mudança; valor desconhecido = ausente (`zap_proto::camera_layout_tolerant`), para uma versão futura não derrubar a mensagem inteira |

App velho (v3.2) contra servidor com `requer_conta`: recebe `conta_necessaria` e mostra a
`mensagem` do servidor ("Atualize o streamaZap para entrar com sua conta").

## 4. App, frente A: Online desligável

**Objetivo:** com o Online desligado, o app não abre conexão com o servidor de produção nem com o
Supabase, e a interface não mostra nada do Online.

Hoje: `Core::start` sobe `shared.online.start()` sempre que `CoreOptions.online` é verdadeiro
(`crates/zap-core/src/engine.rs`), e o `session_run` fica tentando login com backoff mesmo para
quem nunca usou o Online (o servidor exige senha). A barra mostra o estado do servidor
(`ServerStatus`), e o Diagnóstico tem o card "Servidor online".

### Núcleo (`zap-core`)

- `ServerSettings` ganha `enabled: bool` (serde default `false`).
- **Migração** em `settings::load`: arquivo antigo sem o campo → `enabled = !server.password.is_empty()`
  (quem já usava o Online continua com ele ligado).
- `Core::start`: sobe a sessão só se `options.online && settings.server.enabled`.
- `Core::update_settings`: `enabled` passou a contar como "servidor mudou": ligar chama
  `online.start`, desligar chama `online.stop` e zera `server`, salas Online e prévias no estado
  (o mesmo que `restart` faz hoje antes de subir de novo). Sala Online aberta: desligar pede
  confirmação na interface e sai da sala antes.
- `open_in` com Online desligado é sempre `Local`.
- Com o Online desligado, `create_online`, `join_online`, `room_preview` e `server_test`
  devolvem `CoreError::Invalid("Modo Online desligado")`.

### Interface (`app/src`)

| Onde | Com Online desligado |
|---|---|
| `ConfigConexao.tsx` | seção "Online · servidor" vira um interruptor **Usar o modo Online** (texto: precisa de conta, a partir da fase 2; na v3.3.0, sem conta, o texto não fala de conta). Ligado: mostra a conta, o campo **Senha do servidor** (enquanto o `/v1/info` disser `requer_senha`) e "Servidor próprio" (§7) |
| `ModoAbas.tsx` / `SalasScreen.tsx` | sem abas: só a lista Local |
| `CriarModo.tsx` | sem a opção Online |
| `EntrarCodigo.tsx` | escondido |
| `TitleBar` / `ServerStatus` | não aparece |
| Diagnóstico (`diagServidor.ts`) | card some (não conta como problema) |
| Configurações → Conta (§7) | some |
| Supabase | `@supabase/supabase-js` carregado por `import()` dinâmico **só** com o Online ligado: sem timers de renovação nem chamadas de rede |
| Deep link `streamazap://auth/...` | ignorado com o Online desligado |

Primeira execução: Online desligado; um cartão discreto na tela Salas explica "Salas pela internet,
sem Radmin: ative o modo Online em Configurações".

### Aceite A

- `zap-core`: com `enabled = false`, um `zap-test-server` local não recebe **nenhuma** requisição
  em 20 s de app aberto; ligar faz o login acontecer em até 2 s; desligar fecha o WebSocket.
- Migração: `settings.json` antigo com senha → `enabled = true`; sem senha → `false`.
- Vitest: com `server.enabled = false`, nenhum texto "Online", aba, código de sala ou status de
  servidor na tela Salas, Criar sala, barra e Diagnóstico.

## 5. App, frente B: sala e câmera

Quatro melhorias no app. Só a B4 mexe no contrato (e o servidor só repassa um campo).

### B1. Criar sala sem transmitir

Hoje Criar sala exige uma fonte:
- `erroDoPedido` (`app/src/screens/criar/criarModel.ts`) devolve "Escolha o que compartilhar.";
- o botão fica desligado em `CriarScreen.tsx`;
- depois do `createRoom`, o app sempre chama `shareStart`.

O núcleo já separa as duas coisas (`create_room` e `start_share`), e dentro da sala o botão
**Compartilhar** já abre a Transmissão em "Começar a transmitir" (`SalaTransmissao.tsx`).

**Mudança:**
- O passo "O que compartilhar" ganha a opção **Nada por enquanto**. Com ela:
  - os passos de som e qualidade ficam escondidos;
  - o resumo mostra "Você entra na sala sem transmitir";
  - o botão vira **Criar sala**.
- `montarPedido` devolve `null` sem erro nesse caso, e `CriarScreen` chama só `createRoom`.
- Na sala nada muda: o cartão Você já mostra "Você não está transmitindo" e o Compartilhar já
  começa uma transmissão.
- **No Online, é o caminho do Espectador:** ele pode criar a sala (para chat, música ou para um
  amigo transmitir), mas não pode transmitir.

#### Aceite B1

- Criar sala Local e Online com "Nada por enquanto": a sala abre, nenhuma trilha é publicada e
  `Snapshot.share` fica vazio. Depois, "Começar a transmitir" publica normalmente.
- Vitest do passo novo e do resumo.

### B2. Câmera com qualidade própria

Hoje a câmera usa sempre `camera_ladder(1280, 720)` (`crates/zap-core/src/share.rs`, em
`ladder_for` e em `set_camera`), e `pick_camera_mode` (`crates/zap-capture/src/camera_mode.rs`)
prefere modos de até 30 fps. A qualidade escolhida em Criar sala só vale para a tela.

#### Escadas da câmera (`zap-rate/src/ladder.rs` e `balanceamento.md`)

| Câmera | Camada 0 | Camada 1 | Camada 2 |
|---|---|---|---|
| 720p30 (padrão, igual à de hoje) | 1280×720 · 30 · 1500 | — | 640×360 · 15 · 300 |
| 720p60 | 1280×720 · 60 · 2500 | 1280×720 · 30 · 1500 | 640×360 · 15 · 300 |
| 1080p30 | 1920×1080 · 30 · 3000 | 1280×720 · 30 · 1500 | 640×360 · 15 · 300 |
| 1080p60 | 1920×1080 · 60 · 4500 | 1280×720 · 30 · 1500 | 640×360 · 15 · 300 |

Números iniciais: webcam comprime melhor que tela de jogo. Calibrar na bancada (mesma do M2) antes
de fechar.

#### Mudanças

- `zap-rate`: `camera_ladder(preset: Preset, source_w, source_h, source_fps)`. A escada não promete
  mais do que o modo real da câmera: camada acima da resolução ou do fps do modo sai.
- `zap-capture`:
  - `camera_modes(id) -> Vec<CameraMode>` (Media Foundation; stub no Linux);
  - `pick_camera_mode`: a regra 1 passa a ser "só modos com `fps <= max(30, want_fps)`". Com 30 fps
    pedidos, nada muda.
- `zap-core`:
  - `VideoSettings` ganha `camera_preset: Preset` (default `P720p30`, então quem atualiza fica igual);
  - `set_camera` lê os modos da câmera, escolhe o modo com `pick_camera_mode` e monta a escada com
    o modo escolhido;
  - troca ao vivo: mesmo modo de captura → `set_ladder` + `update_track(layers)`, como a tela já faz;
    modo diferente → reinicia a captura da câmera (a trilha continua a mesma);
  - `Sources.cameras[]` ganha o melhor modo de cada câmera (`max_height`, `max_fps`), para a
    interface desligar o que a câmera não faz.
- Interface:
  - `CriarQualidade.tsx`: dois seletores, **Tela** e **Câmera**;
  - `SalaTransmissao.tsx`: seletor de qualidade na seção Câmera;
  - `ConfigVideo.tsx`: qualidade padrão da câmera;
  - em sala Online, opções acima do plano aparecem com cadeado e o selo do plano (§7).

**Risco:** 1080p60 na câmera. A regra de 30 fps existe porque "até 30 fps é o que a captura aguenta
com folga". Antes de liberar 60 fps, medir na bancada (p95 da captura + conversão + codificação,
com tela 1080p60 ao mesmo tempo). Se não passar, a câmera fica em 720p60 no máximo e a opção 1080p60
some.

#### Aceite B2

- Testes de `camera_ladder` para as 4 escadas e para câmera que só faz 720p30 pedindo 1080p60 (sai
  720p30).
- `pick_camera_mode`: pedido de 60 fps escolhe modo de 60 quando existe; de 30, igual a hoje.
- Sala Local com tela 720p30 e câmera 1080p30: `trilha.publicar` da câmera com 3 camadas e camada 0
  1920×1080.
- Trocar só a qualidade da câmera na sala não mexe na trilha da tela (mesmo `trilha_id`, sem
  keyframe pedido nela).

### B3. Webcam como fonte (correção)

Hoje, escolher a aba **Webcam** em Criar sala (ou na Transmissão) manda `source = camera:<id>`
para `ShareSession::start`. Ele publica a trilha com `kind = Screen`: só a janela é tratada à
parte (`crates/zap-core/src/share.rs`). Por isso:
- a câmera aparece como **Tela** no cartão Você e na faixa de baixo dos outros;
- ela entra na ordem e nas regras de tela de `salaModel.ts`;
- ligar "Minha câmera" ao mesmo tempo abre o mesmo dispositivo duas vezes.

**Mudança:**
- `ShareSession::start` com `SourceRef::Camera` não cria trilha de tela. Ele liga a câmera pelo
  mesmo caminho do `set_camera`, com o dispositivo escolhido: trilha `camera`, escada da câmera
  (B2), guardada em `sessao.camera`.
- Som pedido junto vira uma trilha só de som (o `start_audio_only` que já existe).
- **Uma câmera por pessoa:**
  - com a webcam como fonte, o botão "Minha câmera" (tecla C) liga e desliga essa mesma trilha;
  - trocar de câmera troca o dispositivo da trilha.
- Trocar de Tela para Webcam (ou o contrário) na Transmissão remove a trilha antiga e publica a
  nova.
- Interface: o cartão Você e a faixa passam a mostrar "Sua câmera" / "Webcam" sozinhos, porque
  `ownTracks` e `kindLabel` já olham o tipo da trilha.

#### Aceite B3

- Compartilhar só a webcam publica 1 trilha `camera` e nenhuma `screen`.
- O cartão Você e a faixa dos outros mostram "Webcam".
- A tecla C desliga a transmissão da câmera.
- Webcam + som do PC = trilha `camera` + trilha `audio`.
- Webcam como fonte e "Minha câmera" juntas nunca abrem o dispositivo duas vezes.

### B4. Câmera solta ou no canto que você escolher

Hoje, a câmera de quem tem a tela na principal vira sempre o canto inferior direito dessa tela:
- `lugaresDasStreams` e `celulasDaPrincipal` em `app/src/screens/sala/salaModel.ts`;
- posição fixa em `SalaPrincipal.tsx`.

Quem transmite não escolhe nada disso.

**Quem transmite escolhe o jeito da câmera:**
- **No canto da tela:** superior esquerdo, superior direito, inferior esquerdo ou inferior
  direito. O padrão é o inferior direito, como hoje.
- **Solta:** a câmera é uma transmissão à parte, numa célula própria ao lado da tela.

**Contrato e protocolo:**
- A trilha `camera` ganha o campo opcional `camera` (§3).
- `zap_proto::Track`, `ControlMsg::TrackPublish` e `ControlMsg::TrackUpdate` ganham
  `camera_layout: Option<CameraLayout>`:

```rust
pub enum CameraLayout { Corner(Corner), Free }        // serde: {"modo":"canto","canto":…} | {"modo":"solta"}
pub enum Corner { TopLeft, TopRight, BottomLeft, BottomRight }  // "sup_esq" | "sup_dir" | "inf_esq" | "inf_dir"
```

- **No Local**, quem publica manda o campo direto.
- **No Online**, o servidor sobe o `rev` dos crates e guarda o campo no `Track`, em
  `track_publish` e `track_update` (`src/control.rs`). Sem isso, o campo se perde na ponte e
  todo mundo vê o padrão.

**Interface de quem transmite:**
- Em `CriarFontes.tsx`, o interruptor "câmera no canto" vira **Câmera: desligada · no canto · solta**.
- Os 4 cantos aparecem num seletor visual: uma miniatura da tela com os 4 alvos.
- A mesma escolha fica na seção Câmera de `SalaTransmissao.tsx`, ao vivo: manda
  `trilha.atualizar` sem republicar a trilha.

**Interface de quem assiste:**
- `salaModel.ts`: a câmera só vai para o canto da tela do dono quando o modo é `canto`. Com
  `solta`, ela é uma transmissão como as outras: vai para a principal como célula própria
  quando a tela do dono está lá.
- `SalaPrincipal.tsx`: o canto vem do campo (4 posições, com a mesma margem dos controles que
  somem).
- **Quem assiste ainda manda na própria tela**, e a escolha fica só para ele (como as `escolhas`
  de hoje):
  - câmera no canto ganha o botão "Soltar a câmera";
  - câmera solta ganha "Pôr no canto", no canto que quem transmite escolheu.
- **Custo:** câmera solta e grande pede a camada 0 dela (até 4500 kbps em 1080p60). O teto de
  12 Mbps por espectador (§2) segura.

#### Aceite B4

- Testes de `salaModel`:
  - modo canto com cada um dos 4 cantos → `Celula.pip` com o canto certo;
  - modo solta → célula própria;
  - trilha sem o campo → canto inferior direito.
- Trocar o modo ao vivo muda o layout dos outros sem republicar a trilha (só `trilha.atualizar`).
- Servidor (`tests/e2e.rs`): o campo vai e volta em `trilha.publicada`, `trilha.atualizada` e
  `sala.estado`.
- Sala com app antigo (sem o campo): continua no canto inferior direito.

## 6. Supabase, frente C (pasta `supabase/` no repositório do servidor)

### Projeto

- Região **São Paulo**. Plano grátis no começo (50 mil usuários ativos por mês; com uso diário o
  projeto não pausa); Pro (US$ 25/mês) quando houver receita.
- **JWT signing keys assimétricas** (ES256). JWKS em `https://<ref>.supabase.co/auth/v1/.well-known/jwks.json`.
- Fluxo **PKCE**. Redirect permitido: `streamazap://auth/callback`.
- **Vínculo manual** ligado. Só provedores que verificam e-mail (Google, Microsoft, OTP):
  houve falha divulgada de tomada de conta por vínculo manual com e-mail não verificado; não ligar
  provedores como Discord.

### Provedores

| Provedor | O que criar | Atenção |
|---|---|---|
| Google | Cliente OAuth "Web" no Google Cloud, escopos `email` e `profile` | Tela de consentimento em produção (escopos básicos não pedem verificação) |
| Microsoft | App no Entra ID com contas pessoais e corporativas (`common`) | Sem verificação de editor aparece aviso na tela da Microsoft |
| E-mail | OTP de 6 dígitos (modelo com `{{ .Token }}`) | Código, não link: no app desktop é mais simples que tratar link mágico |

### E-mail (Resend)

- SMTP próprio no Supabase (`smtp.resend.com`), remetente `streamazap@heitorcanhizares.net`,
  domínio com SPF e DKIM verificados no Resend.
- Plano grátis do Resend: 100 e-mails por dia. Passou disso em códigos de login → Pro (US$ 20/mês).

### Banco

```sql
create table public.assinaturas (
  user_id      uuid primary key references auth.users on delete cascade,
  plano        text not null check (plano in ('espectador','hd','fullhd')),
  status       text not null check (status in ('ativa','atrasada','cancelada')),
  ciclo        text not null check (ciclo in ('mensal','anual')),
  valido_ate   timestamptz not null,
  asaas_cliente     text,
  asaas_assinatura  text unique,
  atualizado_em timestamptz not null default now()
);
alter table public.assinaturas enable row level security;
create policy "dono lê" on public.assinaturas for select using (auth.uid() = user_id);
-- escrita só pelas Edge Functions (service role)

create table public.asaas_eventos (id text primary key, recebido_em timestamptz default now());

create table public.config_publica (
  id boolean primary key default true check (id),          -- linha única
  cobranca_ativa boolean not null default false,
  plano_padrao text not null default 'fullhd' check (plano_padrao in ('nenhum','espectador','hd','fullhd')),
  precos jsonb not null                                    -- {"espectador":{"mensal":390,"anual":3900}, …} em centavos
);
alter table public.config_publica enable row level security;
create policy "todos leem" on public.config_publica for select using (true);
-- escrita só pelo painel do Supabase
```

- **Custom Access Token Hook** (`public.plano_no_token(event jsonb)`): põe o claim `plano`
  (`fullhd` | `hd` | `espectador` | `nenhum`), nesta ordem:
  1. assinatura `ativa` ou `atrasada` com `valido_ate > now()` → o plano dela;
  2. senão, com `cobranca_ativa = false` → `plano_padrao`;
  3. senão → `nenhum`.

  Permissão só para `supabase_auth_admin`.
- Preços em `config_publica.precos`: mudar preço não pede versão nova do app.
- **CPF não entra no Supabase:** fica só no Asaas.

### Edge Functions (TypeScript)

| Função | Faz |
|---|---|
| `assinar` | Com o JWT do usuário: cria ou reaproveita o cliente no Asaas (`externalReference = user_id`), cria a assinatura (Espectador, Transmissor HD ou Transmissor Full HD; mensal no cartão ou anual no Pix) e devolve o link da primeira cobrança |
| `asaas-webhook` | Confere o token do cabeçalho do Asaas e é idempotente por id do evento (`asaas_eventos`). Pagamento confirmado ou recebido → `ativa`, com `valido_ate` = fim do ciclo + 3 dias. Vencido → `atrasada` (mantém até `valido_ate`). Assinatura removida → `cancelada` |
| `excluir-conta` | Cancela a assinatura no Asaas (se houver) e apaga o usuário. Os registros de acesso no servidor ficam até completar 6 meses (obrigação legal) e depois somem com a limpeza normal |

Deploy: `supabase db push` e `supabase functions deploy` no CI do servidor (job à parte, só na
`main`, com `SUPABASE_ACCESS_TOKEN` em secret).

`assinar` e `asaas-webhook` ficam **prontas e testadas no sandbox do Asaas**, mas só vão para
produção quando a cobrança ligar (§10). `excluir-conta` vai desde já (sem assinatura, só apaga a
conta).

### Aceite C

- Login com os 3 métodos num projeto de teste; com `cobranca_ativa = false` o JWT traz
  `plano: "fullhd"` (o padrão); com `true` e sem assinatura, `plano: "nenhum"`.
- Webhook de teste do Asaas (sandbox) → `assinaturas` ativa → token renovado traz `plano: "espectador"`.
- Mesmo evento duas vezes não muda nada na segunda.

## 7. App, frente D: conta e plano

Fase 3 do app (cadeados, `plano_insuficiente`, telas de plano escondidas sem `cobranca_ativa`):
plano detalhado em [`fase3-app-planos.md`](fase3-app-planos.md).

### Login no app desktop (Tauri)

- `@supabase/supabase-js` com `flowType: 'pkce'` e `detectSessionInUrl: false`.
- A sessão (tokens) fica no **Gerenciador de Credenciais do Windows**: storage do supabase-js →
  comandos Tauri `auth_store_get/set/remove` → crate `keyring` em `app/src-tauri`. Nada de token no
  `localStorage`.
- OAuth:
  1. `signInWithOAuth({ provider, options: { redirectTo: 'streamazap://auth/callback', skipBrowserRedirect: true } })`;
  2. abre a URL no navegador do sistema (`tauri-plugin-opener`, novo);
  3. volta pelo deep link (`tauri-plugin-deep-link`, novo; o esquema é registrado pelo instalador
     NSIS; o `tauri-plugin-single-instance`, que já existe, ganha a feature `deep-link` para entregar
     a URL à instância aberta). Se o app estava fechado, a URL que o abriu espera numa fila no
     Tauri e a interface busca com `deep_link_pendentes` quando o login liga;
  4. `exchangeCodeForSession(code)`.
- Erros do supabase-js vêm em inglês: a interface só mostra texto próprio em português, pelo
  `code` do Supabase Auth (ex.: `otp_expired`), com uma mensagem padrão para o resto. A
  `excluir-conta` mostra a `mensagem` que o servidor devolve em `{"erro","mensagem"}`.
- E-mail: `signInWithOtp({ email })` → o usuário digita o código → `verifyOtp({ email, token, type: 'email' })`.
- Vínculo: `getUserIdentities()` para listar; `linkIdentity({ provider })` pelo mesmo caminho do
  deep link; desvincular só com 2 ou mais identidades.

### Token para o núcleo

- Comando novo `online_set_account(access_token | null)`: o `OnlineSession` guarda o token e o usa
  no `POST /v1/sessoes` (`acesso_conta`) e no `renovar`.
- O front chama esse comando em `onAuthStateChange` (`SIGNED_IN`, `TOKEN_REFRESHED`, `SIGNED_OUT`).
  Sair da conta reinicia a sessão sem conta.
- `zap-net`:
  - `LoginRequest` ganha `acesso_conta: Option<String>` (omitido quando `None`, como `senha_servidor`);
  - a resposta ganha `conta_id` e `plano`;
  - os códigos `conta_necessaria` e `plano_insuficiente` entram no `ApiError`.
- Depois de pagar: `refreshSession()` → token novo com o plano → `online_set_account` →
  `renovar` no servidor.

### Telas

- **Configurações → Conta** (nova, só com o Online ligado):
  - entrar (Google, Microsoft, e-mail);
  - contas vinculadas;
  - plano atual e validade (sem cobrança: "Plano: beta gratuito (Full HD)");
  - **Assinar / trocar plano** (abre o link da função `assinar` no navegador), só com
    `cobranca_ativa`;
  - sair;
  - **Excluir conta**.
- **Senha do servidor** (Configurações → Conexão, logo abaixo da conta): aparece enquanto o
  `/v1/info` disser `requer_senha: true`. Hoje, quando o `POST /v1/sessoes` responde
  `senha_servidor`, o app não pede a senha sozinho (`BACKLOG.md` do servidor, item E). Passa a
  pedir: logado e sem senha (ou com a errada), a tela Salas → Online mostra "O Online está em
  teste fechado. Digite a senha do servidor" com o campo ali mesmo.
- **Servidor próprio** (avançado, para quem roda o próprio servidor open source): endereço e senha,
  como hoje. Se o `/v1/info` do servidor não pede conta, o app nem mostra login.
- **Lista de salas Online com conta `nenhum`:** vê as salas e a prévia; **Entrar** abre o
  convite "Assine o Espectador (R$ 3,90/mês ou R$ 39/ano) para assistir".
- **Transmitir (Criar sala, Transmissão):** para Espectador, os botões de transmitir mostram
  "Vire Transmissor" em vez de falhar; qualidades acima do plano aparecem com cadeado e selo
  ("Full HD").
- `plano_insuficiente` vira aviso com botão "Ver planos". O servidor manda `plano` e `campo` no
  `erro`, mas o `ControlMsg::Error` do `zap-proto` só tem `codigo`, `mensagem` e `ref`. Na fase 3,
  ele ganha os extras opcionais (`campo`, `plano`, `tentar_em_ms`, `protocolo_min`/`protocolo_max`)
  para o app saber, pelo QUIC, qual parte da trilha foi recusada. Hoje eles chegam e são ignorados
  sem erro.
- Todas as telas de preço e convite para assinar já existem e têm teste, mas só aparecem com
  `cobranca_ativa = true` (lido de `config_publica` ao entrar e a cada 1 h).
- Assistir não tem cadeado nem etiqueta: o espectador vê o que quem transmite manda.
- O plano exibido vem da resposta do servidor (`plano`), não só do claim decodificado no front.

### Aceite D

- Login Google e e-mail OTP de ponta a ponta contra o projeto de teste, com deep link chegando à
  instância já aberta.
- Token renovado chega ao núcleo sem derrubar a sala.
- Fechar e abrir o app mantém o login (token no Gerenciador de Credenciais, não em arquivo).
- Vitest das telas de Conta, do convite para conta `nenhum` e do cadeado nas qualidades de transmissão.

## 8. Servidor, frente E: conta e plano

### Configuração (`src/config.rs`)

| Opção | Variável | Padrão | Uso |
|---|---|---|---|
| `--supabase-url` | `ZAP_SUPABASE_URL` | vazio | liga a conferência de JWT (JWKS, `iss = <url>/auth/v1`, `aud = authenticated`) |
| `--requer-conta` | `ZAP_REQUER_CONTA` | `false` | sem JWT válido → `conta_necessaria` |
| `--max-espectadores-ponte` | `ZAP_MAX_ESPECTADORES_PONTE` | sem limite | quantos espectadores a ponte serve ao mesmo tempo; acima disso, assinatura nova recebe só a camada 2 até abrir vaga (proteção do beta gratuito) |

`--senha` continua e é a porta do beta fechado:

| `--senha` | `--requer-conta` | O que o `POST /v1/sessoes` pede | Quando |
|---|---|---|---|
| sim | não | só a senha (a conta, se vier, vale para o plano) | fase 2: apps velhos sem login continuam entrando |
| sim | sim | **senha e conta** | fases 3 a 6: beta fechado |
| não | sim | só a conta | quando o dono decidir abrir o Online |

Para abrir, falta um detalhe de deploy: hoje apagar o secret `ZAP_SENHA` **não** tira a senha
(vazio = mantém o que está no `stack.env`; `BACKLOG.md`, item E). O `deploy.sh` ganha um valor
explícito para limpar (por exemplo, o secret com o texto `-`), com teste no `vps-deploy.sh`.

### Módulo novo `src/auth.rs`

- Cache da JWKS:
  - busca na subida e a cada 10 min;
  - de novo quando chega um `kid` desconhecido (no máximo 1 vez por minuto);
  - falha de rede mantém a última JWKS boa.
- Conferência com `jsonwebtoken` (ES256/RS256, `exp` com 60 s de folga) → `Account { id: sub, plan: PlanId }`.
  Claim ausente ou desconhecido = `nenhum`.
- Nunca segurar o lock do estado durante a busca da JWKS (regra do `CLAUDE.md`).

### Sessão (`src/http.rs`, `src/state.rs`)

- `create_session`:
  - confere o protocolo, depois a senha, depois `acesso_conta` (a ordem de `rest.md`);
  - `Participant` ganha `account: Option<Account>`;
  - o mesmo `conta_id` em duas instalações vira dois participantes (pode ver no PC e no notebook);
  - a resposta inclui `conta_id` e `plano` com os limites de `zap_proto::plan::limits`.
- `renovar`: a semântica de §3 (sempre renova; `acesso_conta` válido atualiza o plano ou liga a
  conta; inválido é ignorado; outra conta → `proibido`).

### Limites

| Onde | Mudança |
|---|---|
| Entrar e criar sala (`src/http.rs`, `create_room` e `enter_room`) | `limits(plano).can_join` falso → `plano_insuficiente`. `preview` continua liberado |
| Publicar (`src/control.rs`, `track_publish` / `track_update` → `validate_media`) | depois da validação atual, `zap_proto::plan::check_publish(kind, &layers, &limites)`. Sem plano de transmissor ou camada 0 acima → `plano_insuficiente` (403, com `campo: "camadas"`) |
| Banda (`src/relay.rs`, `RoomRelay::on_report`) | `bandwidth_kbps = min(relatorio, VIEWER_MAX_KBPS)` antes do alocador e do `Prober`, para todos |
| Vagas da ponte (`src/relay.rs`, `RoomRelay::subscribe`) | com `--max-espectadores-ponte`, quem passa do limite fica com teto 2 (como a prévia) até abrir vaga; a rota direta não conta |

Assistir não muda: `SubSpec::ceiling` e o alocador ficam como estão, e a rota direta também.

### Aceite E

- `tests/e2e.rs`:
  - JWT válido (chave de teste servida por uma JWKS falsa) cria sessão com o plano certo;
  - JWT vencido, sem assinatura ou de outro `iss` → `conta_necessaria`;
  - com `--senha` e `--requer-conta`: só a conta → `senha_servidor`; só a senha →
    `conta_necessaria`; as duas → sessão criada;
  - conta `nenhum`: lista e prévia funcionam; criar e entrar → `plano_insuficiente`;
  - Espectador entra e assiste uma tela 1080p60 até a camada 0 (pelas sondas); publicar →
    `plano_insuficiente`;
  - Transmissor HD publicando 1080p60 → `plano_insuficiente`; 720p60 passa;
  - teto de banda: espectador com duas telas 1080p60 grandes → uma na camada 0, a outra abaixo;
  - `renovar` com plano novo vale para a próxima sala ou trilha;
  - `--max-espectadores-ponte 2`: o terceiro espectador pela ponte fica na camada 2 e sobe quando
    um dos dois sai.
- Os testes atuais (com `--senha`) passam sem mudança.

## 9. Servidor, frente F: registros de acesso e métricas

Marco Civil (art. 15): provedor de aplicação com fins econômicos guarda os registros de acesso
(IP, data e hora) por 6 meses, sob sigilo. O STJ inclui a **porta lógica de origem**. Hoje o
estado é todo em memória, então nada disso existe.

- SQLite (`rusqlite`, com `spawn_blocking`) num volume novo de leitura e escrita: `/srv/streamazap/db`
  montado em `/db`. O `/data` continua só leitura.
- Uma tarefa escritora recebe os eventos por um **canal limitado** (4096). Canal cheio: loga aviso
  e conta a perda numa métrica (nunca trava a mídia).
- Tabela:

```sql
create table acessos (
  id integer primary key,
  conta_id text, participante_id text not null,
  evento text not null,          -- 'sessao' | 'transmissao' | 'assistir'
  rota text,                     -- 'ponte' | 'direta' | null
  trilha_tipo text,              -- 'tela' | 'janela' | 'camera' | 'audio' | null
  ip text not null, porta integer not null,
  inicio_utc text not null, fim_utc text
);
create index acessos_fim on acessos(fim_utc);
```

- **Origem do IP e da porta:**
  - REST/WebSocket: `ConnectInfo<SocketAddr>` (já usado em `src/http.rs`);
  - QUIC: `conn.remote_address()` (`src/quic.rs`);
  - com `--atras-de-proxy`, a porta se perde. Por isso a produção continua exposta direto, sem
    proxy (já é assim).
- Limpeza diária: apaga linhas com `fim_utc` de mais de 183 dias.
- Dá para tirar da mesma tabela:
  - tempo em chamada e tempo transmitindo por conta;
  - **% de assinaturas que fecharam pela rota direta** (o número que mais mexe no custo);
  - **pico de espectadores pela ponte ao mesmo tempo**, por hora (é o que diz se o preço do
    Espectador e a capacidade da VPS estão certos);
  - endpoint `GET /v1/admin/metricas` com token de administrador próprio (`ZAP_ADMIN_TOKEN`),
    nunca exposto no app.
- Nada de conteúdo: chat e música continuam só em memória.

### Aceite F

- Sessão + transmissão + assistir geram 3 linhas com IP e porta reais do teste e `fim_utc`
  preenchido ao sair.
- Linha com `fim_utc` de 184 dias atrás some na limpeza; de 182 dias fica.
- Derrubar o SQLite (arquivo sem permissão) não derruba o servidor: loga erro e segue.

## 10. Pagamento, frente G (futuro: pronto e desligado)

A cobrança **não liga agora**. Tudo fica feito, testado e atrás da chave
`config_publica.cobranca_ativa` (§2): ligar é mudar a chave depois do checklist abaixo, sem
versão nova do app nem do servidor.

### Preços (já em `config_publica.precos`)

| Plano | Mensal (cartão) | Anual (Pix) |
|---|---|---|
| Espectador | R$ 3,90 | R$ 39 |
| Transmissor HD | R$ 7,90 | R$ 79 |
| Transmissor Full HD | R$ 14,90 | R$ 149 |

Uma assinatura por conta: quem é Transmissor não paga o Espectador à parte.

### O que fica pronto agora (fases 2 e 3)

| Peça | Estado |
|---|---|
| `zap_proto::plan`, limites no servidor, teto de banda | **no ar** (com o plano padrão `fullhd`, ninguém é barrado) |
| Tabelas `assinaturas`, `asaas_eventos`, `config_publica` e o hook | **no ar** |
| Edge Functions `assinar` e `asaas-webhook` | escritas, testadas no **sandbox** do Asaas, não publicadas em produção |
| Telas de plano, preço e convite para assinar no app | prontas e testadas, escondidas sem `cobranca_ativa` |

### Como vai funcionar quando ligar

- **Fluxo:**
  1. Conta → **Assinar** chama a função `assinar`;
  2. o app abre o link da cobrança no navegador;
  3. o Asaas chama o `asaas-webhook`;
  4. o app faz `refreshSession()` e renova a sessão no servidor.
- **Troca de plano na v1** (ex.: Espectador → Transmissor): cancela a assinatura atual e cria a
  nova, sem cobrança proporcional.
- **CPF:** o Asaas pede CPF do cliente para cobrar (confirmar na documentação atual). Ele é
  coletado na página do Asaas ou passado direto pela função, e nunca fica no Supabase.
- **Reembolso:** pedido em até 7 dias da compra (direito de arrependimento em compra pela
  internet) devolve o valor e cancela a assinatura.

### Checklist para ligar a cobrança

- [ ] MEI de serviços aberto (DAS R$ 86,05/mês em 2026).
- [ ] Conta Asaas de produção; chave da API e token do webhook como secrets das Edge Functions.
- [ ] NFS-e pelo Asaas testada (conferir se Primavera do Leste está integrada ao emissor dele).
- [ ] Termos de uso com cobrança, renovação e reembolso.
- [ ] Publicar `assinar` e `asaas-webhook` em produção e cadastrar a URL do webhook no Asaas.
- [ ] Uma compra real de cada plano (e um reembolso) com conta de teste.
- [ ] **Aviso aos usuários do beta com 30 dias de antecedência** (cartão no app e e-mail): quando
      acaba o gratuito e quanto vai custar.
- [ ] VPS ilimitada no ar (§11) e `--max-espectadores-ponte` revisto.
- [ ] Decidir a senha do servidor: tirar (Online aberto e pago) ou manter (beta pago fechado).
- [ ] Mudar `cobranca_ativa` para `true`.

## 11. Infra, frente H

- **VPS com tráfego ilimitado em São Paulo:**
  - contratar 1 mês e medir com `iperf3` de dia e à noite;
  - critério: ≥ 300 Mbps sustentados por 10 min e termos sem "uso justo" que corte streaming;
  - passou → migrar o `docker-compose` igual ao de hoje e apontar `streamazap.heitorcanhizares.net`
    para o IP novo;
  - a Hostinger fica só com o JusGestio.
- `docker-compose.yml`:
  - volume `/srv/streamazap/db:/db` (leitura e escrita);
  - variáveis `ZAP_SUPABASE_URL`, `ZAP_REQUER_CONTA` e `ZAP_ADMIN_TOKEN` (este pelo stdin do
    deploy, como a senha).
- Monitorar a banda (`vnstat`) e o pico de pessoas ao mesmo tempo. **Capacidade de referência:**
  600 Mbps úteis de saída ÷ 8,4 Mbps (tela 1080p60 + câmera pequena + som) ≈ 71 espectadores ao
  mesmo tempo; com todos no teto de 12 Mbps, 50. Passou de 70% no horário de pico por uma semana →
  segunda VPS (cada uma rende ~R$ 320/mês a mais, §2).

## 12. App + Supabase, frente I: social

Só existe no **Online** (precisa de conta) e desliga junto com ele (frente A). Nada passa pelo
servidor de mídia: amigos, presença, convites e chamados vivem no Supabase (tabelas com RLS,
Realtime e Edge Functions). No **Local** continua o que já existe (salas aparecem sozinhas e o aviso
"amigo abriu sala"), mais um botão **Copiar convite** com o IP do Radmin e a porta.

### I1. Amigos

- Cada conta ganha um **código de amigo** (`heitor#4821`), mostrado em Configurações → Conta.
  **Não existe busca por e-mail nem por nome**: só se adiciona alguém pelo código ou pelo link de
  convite (I2). Isso evita contato de desconhecidos.
- Pedido de amizade → a outra pessoa **aceita, recusa ou bloqueia**. Só amigos aceitos veem a
  presença um do outro, convidam e chamam.
- Limites: 20 pedidos pendentes enviados por conta, 90 amigos (o Realtime aceita até 100 canais
  por conexão, e a presença usa um canal por amigo).
- Tela **Salas → Amigos** (painel novo): cada amigo com o estado (online, numa sala, transmitindo,
  offline) e os botões **Entrar na sala**, **Convidar** e **Chamar**. Menu: silenciar, desfazer
  amizade, bloquear.

```sql
create table public.perfis (
  user_id uuid primary key references auth.users on delete cascade,
  apelido text not null check (char_length(apelido) between 1 and 32),
  codigo  text not null unique            -- "apelido#1234", gerado no cadastro
);
create table public.amizades (
  de uuid not null references auth.users on delete cascade,     -- quem pediu
  para uuid not null references auth.users on delete cascade,
  estado text not null check (estado in ('pendente','aceita','bloqueada')),
  criado_em timestamptz not null default now(),
  primary key (de, para), check (de <> para)
);
create table public.preferencias_sociais (
  user_id uuid primary key references auth.users on delete cascade,
  aceita_convites boolean not null default true,
  aceita_chamados boolean not null default true,
  mostra_presenca boolean not null default true,
  mostra_sala boolean not null default true,
  nao_perturbe_ate timestamptz
);
create table public.silenciados (user_id uuid, amigo uuid, primary key (user_id, amigo));
create table public.chamados (de uuid, para uuid, tipo text, em timestamptz default now()); -- só para o limite; apaga em 24 h
```

- RLS: cada um lê só as próprias linhas e as amizades em que aparece; pedir = `insert` com
  `de = auth.uid()`; aceitar = `update` só por quem é o `para`; bloquear vale para os dois lados.
- Função `pode_interagir(a, b)` (`security definer`): amizade aceita e ninguém bloqueou. Usada
  nas políticas do Realtime e nas Edge Functions.

### I2. Convidar

- **Para o app:** link `https://<site>/c/<codigo>` (página estática, na Vercel). Com o app
  instalado, abre `streamazap://amigo/<codigo>` e o app mostra "Mandar pedido de amizade para
  Heitor?". Sem o app, a página leva ao download. Nada é enviado sem a pessoa confirmar.
- **Para a sala:** em sala Online, **Convidar amigos** lista os amigos online; o convite chega
  como cartão no app e notificação do Windows ("Heitor te chamou para a sala *Noite do jogo*")
  com **Entrar** e **Agora não**.
- **Copiar convite** (Online): texto com o código `ZAP-XXXX` e o link `streamazap://sala/ZAP-XXXX`,
  para mandar por WhatsApp ou Discord. O deep link entra na sala pelo código (o `JoinTarget::Code`
  que já existe).
- Sala com senha: na v1 o convite leva só o código, e a senha continua sendo pedida. Ingresso de
  convite (entrar sem a senha) fica para depois.
- **Senha do servidor:** convite e link nunca levam a senha do servidor. No beta fechado, um amigo
  sem a senha recebe o convite, mas ao entrar vê o aviso de teste fechado (§7). O próprio convite
  pode dizer isso: "O Online está em teste fechado: peça a senha a quem te convidou".

### I3. Chamar atenção ("barulho")

- **Chamar** num amigo: o app dele toca um som curto (diferente do som do chat), pisca o ícone
  na barra de tarefas (`requestUserAttention`) e mostra a notificação "Heitor está te chamando"
  (com **Entrar na sala** se quem chamou estiver numa).
- **Limite:** 1 chamado por amigo a cada 60 s e no máximo 5 por hora. Convites: 1 por amigo e
  sala a cada 2 min.

### I4. Permissões e como desligar

Quem **recebe** decide. Tudo em Configurações → Social (só com o Online ligado), gravado em
`preferencias_sociais` para o Supabase também respeitar:

| Opção | Padrão | Efeito |
|---|---|---|
| Receber convites para salas | ligado | desligado: convites de amigos não chegam |
| Deixar amigos me chamarem | ligado | desligado: o botão Chamar some para os amigos |
| Som do chamado | ligado | desligado: chega só a notificação |
| Notificações do Windows | pede permissão na primeira vez | sem permissão: só o cartão dentro do app |
| Aparecer online para amigos | ligado | desligado: aparece offline para todos |
| Mostrar em que sala estou | ligado | desligado: aparece só "online" |
| Não perturbe | desligado | por 1 h, 8 h ou "enquanto eu transmito" |
| Silenciar um amigo | — | sem chamados nem convites dele (ele não fica sabendo) |

- A permissão do Windows é pedida pelo `tauri-plugin-notification` (que já existe) na primeira
  vez que um convite ou chamado chegaria, não na abertura do app.
- Com o **app fechado, nada chega** (não há serviço de push). Opção nova, desligada por padrão:
  **Iniciar com o Windows, minimizado na bandeja** (ícone da bandeja do Tauri), para receber
  convites.

### Como os avisos viajam (Realtime)

- Cada conta tem um canal **privado** `usuario:<id>`. Política do `realtime.messages`: só o dono
  **lê**; ninguém **escreve** pelo app. Quem escreve são as Edge Functions.
- `social-convidar` e `social-chamar`: conferem o JWT, `pode_interagir`, as preferências de quem
  recebe, se ele silenciou quem manda, o "não perturbe" e o limite (`chamados`). Passou → mandam o
  aviso no canal de quem recebe. Não passou → respondem "ok" do mesmo jeito (quem manda não
  descobre se foi silenciado).
- **Presença:** cada um anuncia o próprio estado no canal `presenca:<id>` (só o dono escreve; só
  amigos aceitos leem, pela `pode_interagir`). O estado leva `online`, `sala` (código, se
  `mostra_sala`) e `transmitindo`. Com `mostra_presenca` desligado, o app não anuncia nada.
- O app do cliente também respeita as preferências (não toca som com "não perturbe", por
  exemplo), mas a regra que vale é a do Supabase: um app modificado não consegue mandar spam.
- Nada de conteúdo fica guardado: o Realtime não grava as mensagens, e `chamados` só guarda o
  horário para o limite e é apagado em 24 h.

**Capacidade do Supabase:** o Realtime grátis aceita 200 conexões ao mesmo tempo, e o Pro aceita
500 (10 mil sem teto de gasto). Cada app aberto com o Online ligado é uma conexão. Passou de ~150
pessoas online no pico → Supabase Pro (US$ 25/mês, já previsto quando houver receita).

**Formato dos avisos e da presença:** a fonte é o `supabase/README.md` do servidor (§4 API e §4b
Realtime). O que o app precisa respeitar, pelos limites do Realtime (página "Limits" do Supabase,
grátis/Pro):
- **Entradas em canal:** 100/500 por segundo. Com até 92 canais por app, o app entra nos canais de
  presença aos poucos (uns 10 por segundo) e tenta de novo, com espera, quando recebe
  `too_many_joins`.
- **Presença:** 5 chamadas a cada 30 s por cliente e 20/50 mensagens por segundo. O app junta as
  mudanças de `sala` e `transmitindo` e anuncia no máximo uma vez a cada 6 s.
- **Mensagens:** 100/500 por segundo. Passar disso (`tenant_events`) desconecta todo mundo. A
  presença de quem tem muitos amigos se espalha para todos eles: medir no projeto de teste antes
  de abrir o Online.
- **Autorização:** a permissão de um canal privado é conferida ao entrar e fica guardada até a
  conexão receber um JWT novo. Quem desfaz a amizade ou bloqueia some para o outro só quando o
  token dele renovar (até 1 h). Os convites e chamados param na hora, porque as funções conferem a
  cada envio.
- **Mesma conta em dois PCs:** a mesma chave de presença tem dois estados. O app junta: online se
  houver algum, transmitindo se algum estiver, e a sala do mais recente.
- **"Não perturbe enquanto eu transmito":** essa opção fica só no app. Enquanto transmite, ele grava
  `nao_perturbe_ate = max(atual, agora + 15 min)` a cada 5 min, para nunca encurtar um "não
  perturbe por 8 h".

### Aceite I

- Pedido de amizade por código e por link; aceitar, recusar e bloquear (com teste das políticas
  RLS: estranho não lê amizade alheia; bloqueado não consegue pedir de novo).
- Presença: amigo aparece online em até 5 s depois de abrir o app e some em até 30 s depois de
  fechar; com "Aparecer online" desligado, nunca aparece.
- Chamado: chega com som e notificação; o segundo em menos de 60 s é descartado; silenciado,
  desligado ou "não perturbe" → não chega, e quem mandou recebe "ok" igual.
- Convite para sala: **Entrar** leva à sala pelo código; sala com senha pede a senha.
- Online desligado: nenhuma conexão com o Realtime (vale o aceite A).

## 13. App, frente J: atualização automática

Hoje (`app/src-tauri/src/commands/update.rs`): com `settings.update.auto` ligado, 10 s depois de
abrir o app ele procura, baixa e marca **instalar ao fechar**. Só procura na abertura, e quem deixa
o app aberto (ou só minimiza) nunca recebe a versão nova. A regra "nunca reiniciar numa sala"
(`updater.rs`) continua valendo em tudo abaixo.

### Opções (Configurações → Sobre e atualizações)

| Opção | Valores | Padrão |
|---|---|---|
| Procurar atualizações sozinho | ligado / desligado | ligado |
| Baixar sozinho | ligado / desligado | ligado |
| Instalar | **Perguntar** · **Ao fechar o app** · **Sozinho, quando eu não estiver em sala** | Sozinho |
| Canal | Estável / Beta (como hoje) | Estável |

- `UpdateSettings` passa a ter `check: bool`, `download: bool` e `install: InstallMode`
  (`ask` | `on_exit` | `auto`) no lugar de `auto`.
- **Migração** em `settings::load`: `auto: true` → tudo ligado e `install = auto` (o texto de
  hoje já promete "baixa, confere e reabre já atualizado"); `auto: false` → tudo desligado e
  `install = ask`. Instalação nova já começa com `auto`.

### Como funciona

- **Procurar:** na abertura (depois de 10 s) e **a cada 6 h** com o app aberto, com uns minutos de
  sorteio para não bater todo mundo no GitHub na mesma hora.
- **Baixar:** em segundo plano assim que achar.
- **Instalar sozinho** só quando for seguro, conferido a cada 30 s:
  - fora de sala, sem transmitir, sem prévia aberta;
  - e o app minimizado, ou o PC sem uso há 2 min (tempo ocioso do Windows por
    `GetLastInputInfo`, função nova em `zap-sys`, com stub no Linux).
  - Então aparece um aviso de 30 s: "Atualizando para a v3.4.0 em 30 s · **Adiar 1 h**". Sem
    resposta, instala.
  - `plugins.updater.windows.installMode = "passive"` fica explícito no `tauri.conf.json`. Nesse
    modo, o updater do Tauri chama o instalador NSIS com os argumentos de modo passivo e de
    reiniciar, e o app abre de novo sozinho.
- **Depois de atualizar:** o app abre na tela Salas e mostra "Atualizado para a v3.4.0" com as
  notas da versão (as `notas` que o `update_check` já devolve).
- **Perguntar:** baixa (se "Baixar sozinho" estiver ligado) e mostra o aviso **Instalar agora** /
  **Ao fechar**, como hoje.

### Aceite J

- Função pura `pode_instalar_agora(estado)` com a tabela de casos (em sala, transmitindo, prévia,
  minimizado, ocioso) e testes.
- Migração de `auto: true` e `auto: false`.
- Agenda de 6 h com relógio falso.
- Teste de verdade pelo canal beta: app minimizado e fora de sala recebe a versão nova, instala
  sozinho e reabre na tela Salas; numa sala, espera sair.

## 14. Fora do código (antes de abrir o Online ao público)

O que é só para cobrar (MEI, Asaas, NFS-e, termos de cobrança) está no checklist de §10.

Rascunho da política, dos termos do beta e da página inicial em `site/` (2026-11-12; ver
`site/README.md`), publicado em <https://heitorcanhizares.github.io/streamaZap/>: falta a revisão
do advogado.

- [ ] **Política de privacidade:**
  - o que guarda: e-mail, contas vinculadas, plano, IP + porta + horário por 6 meses;
  - o que não guarda: chat, vídeo, áudio;
  - social: código de amigo, amizades e preferências (presença e convites não ficam guardados);
  - com quem compartilha: Supabase e Resend (e o Asaas, quando a cobrança ligar).
- [ ] Termos de uso do beta gratuito (regras de uso, o que é guardado, que o beta pode acabar).
- [ ] Botão de excluir conta (Conta → Excluir) funcionando.
- [ ] **ECA Digital:** confirmar com advogado se o Online é de "acesso provável" por adolescentes e
      o que isso exige (verificação de idade). A parte social (I) pesa nessa análise: contato só
      entre amigos aceitos, sem busca, com bloqueio, ajuda, mas o advogado precisa olhar. Pode virar
      uma frente nova antes da abertura.

## 15. Ordem de entrega

| Fase | Repositório | Entrega | Depende de |
|---|---|---|---|
| 1 | app (+ servidor só para repassar o campo da B4) | **v3.3.0**: atualização automática (J) + Online desligável (A) + sala e câmera (B1 a B4) + escadas da câmera em `balanceamento.md` e campo `camera` em `mensagens.md`. No servidor: subir o `rev` e repassar o campo | — |
| 2 | ambos | Supabase (C, sem pagamento) + login no app (D, sem telas de plano) + servidor aceitando conta **ou** senha (E, só sessão). Todo logado é `fullhd` no beta fechado | 1 |
| 3 | ambos | `zap_proto::plan`, limites no servidor (E), `config_publica` e hook (C), telas de plano escondidas no app (D), funções do Asaas testadas no sandbox (G) | 2 |
| 4 | servidor | Registros de acesso e métricas (F) | 2 |
| 5 | app + Supabase | Social (I): amigos, presença, convites e chamar atenção | 2 |
| 6 | servidor + ops | VPS ilimitada (H) e `requer_conta` ligado **com a senha mantida**: Online em **beta fechado gratuito** (conta + senha) | 3, 4 e §14 |
| Quando decidir | ops | **Abrir o Online**: tirar a senha (com o jeito explícito de limpar no deploy, §8) | 6 e §14 |
| Futuro | Supabase + ops | **Ligar a cobrança**: checklist de §10 e `cobranca_ativa = true` | 6 |

Na fase 3, a ordem de deploy é: servidor primeiro (aceita os campos novos), depois o app.

A atualização automática (J) vai na **primeira** versão de propósito: é ela que faz as versões das
fases seguintes chegarem a todo mundo sem depender de a pessoa fechar o app.

**Mais faixas de transmissor depois:** como o plano só limita o que se transmite (não o que se
assiste), dá para criar outras faixas (ex.: "Transmissor 1080p30") só com uma entrada nova em
`zap_proto::plan`, sem mexer na escada nem no protocolo.

**Quando houver versão para iOS/macOS:** a regra 4.8 da App Store pede, junto com login do Google
ou da Microsoft, uma opção equivalente que limite os dados a nome e e-mail e deixe esconder o
e-mail. Aí entra o Sign in with Apple: Services ID + chave `.p8` (conta Apple Developer, US$ 99/ano,
segredo que vence em até 6 meses) e o cadastro do domínio de envio do Resend no "Ocultar meu e-mail"
da Apple (senão o e-mail para `@privaterelay.appleid.com` volta).

## 16. Riscos e pontos em aberto

| Risco | O que fazer |
|---|---|
| Câmera 1080p60 pesa na captura | Bancada antes de liberar (§5, B2); senão, máximo 720p60 |
| Servidor de produção sem o `rev` novo na fase 1 | O campo `camera` se perde no Online e todos veem o canto padrão; nada quebra. Subir o servidor junto com a v3.3.0 |
| Quem assiste várias transmissões grandes estoura o custo por pessoa | Teto de 12 Mbps por espectador (§2), no servidor |
| Transmitir acima do plano pela rota direta (app modificado) | Aceito: não custa banda |
| Espectador barato demais se o pico de uso for maior que o previsto | Preço calculado no pior caso (§2); medir pela métrica de pico (§9) e revisar em 3 meses |
| Taxa do cartão pesa no Espectador mensal (16%) | Destacar o anual no Pix (5%) |
| Resend grátis (100/dia) | Alerta no painel; Pro quando passar |
| VPS "ilimitada" sem banda garantida | Teste de 1 mês (§11) antes de migrar |
| Supabase fora do ar | Sessões abertas seguem (JWKS em cache); só login novo para |
| Plano vence no meio da sala | Vale na próxima sala ou trilha (§2); nada cai |
| Spam de chamados por app modificado | Envio só pelas Edge Functions, com limite e preferências conferidos no Supabase (§12) |
| Mais de 200 pessoas online no pico estoura o Realtime grátis | Supabase Pro (500; 10 mil sem teto de gasto) |
| Convite não chega com o app fechado | Opção de iniciar com o Windows na bandeja (§12, I4) |
| Instalação sozinha interrompe alguém | Só fora de sala, minimizado ou ocioso, com aviso de 30 s e Adiar (§13) |
| Beta gratuito sem receita paga a VPS (~R$ 129/mês) | Senha do servidor (só entra quem você der a senha) e `--max-espectadores-ponte` (§2); ligar a cobrança quando o uso justificar |
| A senha do servidor vaza | Trocar pelo secret `ZAP_SENHA` (o README do servidor tem o passo a passo); as salas reiniciam e todo mundo entra de novo com a nova |
| Usuários do beta reclamam quando a cobrança ligar | Aviso com 30 dias (§10). Opcional: mostrar desde o começo "beta gratuito, depois a partir de R$ 3,90" |
| Primeira execução com Online desligado esconde o produto pago | Cartão discreto na tela Salas (§4); medir quantos ligam |
