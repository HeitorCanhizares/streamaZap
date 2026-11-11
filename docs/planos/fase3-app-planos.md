# Fase 3 do app: planos, cadeados e telas de plano escondidas

Versão 1, 2026-11-11. Detalha a fase 3 da §15 de [`contas-planos-camera.md`](contas-planos-camera.md),
**só o lado do app**. O servidor já fez a parte dele (limites de plano, `config_publica`, hook do
token, `assinar` testada no sandbox) e o contrato em `docs/api/` **não muda**.

Leitura obrigatória antes de implementar: §2 (planos e limites, beta gratuito), §5 B2 (cadeado nas
qualidades), §7 (telas), §10 (o que fica pronto agora) e §15 do plano grande; e o
`supabase/README.md` §4 do repositório do servidor (`streamaZapi`), só leitura.

## 1. Escopo

### Entra

1. `zap-proto`: `ControlMsg::Error` com os extras do contrato (`campo`, `plano`, `tentar_em_ms`,
   `protocolo_min`, `protocolo_max`), opcionais e tolerantes.
2. `zap-net`: mandar `ref` no topo do JSON de um pedido do WebSocket (`trilha.publicar` usa o
   `local_id`), para o `erro` do servidor voltar ligado ao pedido.
3. `zap-core`:
   - calcula, com as escadas do `zap-rate` e `check_publish`, quais qualidades (tela e câmera)
     passam do plano da sessão e põe no `Snapshot` (`quality_locks`);
   - confere o plano **antes** de mandar ao servidor (publicar, trocar qualidade, criar e entrar em
     sala Online) e devolve `plano_insuficiente` na hora;
   - repassa `plano`/`campo` do `erro` do servidor e põe a última recusa no `Snapshot`
     (`plan_notice`), com um número que cresce a cada recusa.
4. Interface (`app/src`):
   - lê `config_publica` e `assinaturas` do Supabase (só com o login ligado), chama a função
     `assinar` e atualiza a sessão depois do pagamento;
   - cadeado + selo do plano necessário nas qualidades acima do plano;
   - "Vire Transmissor", convite do Espectador, aviso de `plano_insuficiente`;
   - Conta com o plano; tela de planos com preços (só com `cobranca_ativa`);
   - `mockConta` com `config_publica`, `assinaturas`, `assinar` e `refreshSession`.

### Fica de fora

- Servidor, Supabase e contrato (`docs/api/`): nada muda. Publicar `assinar`/`asaas-webhook` em
  produção e ligar a cobrança são do checklist de §10.
- Cancelar assinatura pelo app (não há função para isso; trocar de plano cancela a atual, §10).
- Reembolso, nota fiscal, termos: fora do app.
- Desfazer a escada local quando o servidor recusa um `trilha.atualizar` depois (só acontece com o
  plano desatualizado no app; ver §8, riscos). A recusa aparece no aviso.
- Números de limite na tela de planos: os cartões falam em "HD" e "Full HD", sem altura, fps nem
  kbps (decisão 2: números só em `zap_proto::plan`).
- Social (fase 5) e teste contra o Supabase/Asaas de verdade (manual, depois).

## 2. Decisões

As 8 decisões do dono (na ordem dele) e o que este plano decide em cima delas.

| # | Decisão | Como fica |
|---|---|---|
| 1 | `ControlMsg::Error` ganha os extras, tolerante | Campos soltos no variante, cada um com `deserialize_with = tolerant` (tipo errado ou valor desconhecido = `None`, a mensagem continua valendo). Construtor `ControlMsg::error(code, message, reference)` para quem não usa os extras |
| 2 | Números nunca no TypeScript; o núcleo calcula os presets travados | `Snapshot.quality_locks` (§4.3) com a lista de presets travados de tela e de câmera e o plano mais barato que libera cada um. Sem plano (Local, Online sem conta) o campo não existe |
| 3 | O núcleo confere antes de publicar | Um ponto só (`Shared::plan_ladder`) chamado por `ShareSession` **antes** de mexer em qualquer coisa local, mais a conferência de defesa em `OnlineRoom::publish_track`/`update_track` |
| 4 | Cadeado sempre que a sessão Online tem plano; preço e "Ver planos" só com `cobranca_ativa` | Igual. Sem cobrança, clicar no travado só mostra "Seu plano não permite essa qualidade" |
| 5 | Conta: beta gratuito ou plano + validade + status + "Assinar / trocar plano" | `ContaPlano` (§4.5) |
| 6 | `config_publica` e `assinaturas` ao entrar e a cada 1 h; falha = cobrança desligada | `planoSync` (§4.4) |
| 7 | Espectador: "Vire Transmissor"; `nenhum`: convite; `plano_insuficiente`: aviso com "Ver planos" | `VireTransmissor`, `ConviteEspectador`, `AvisoPlano` (§4.5) |
| 8 | `mockConta` com tudo isso | §4.4 |

Decisões novas deste plano (com o motivo):

| Assunto | Decisão | Motivo |
|---|---|---|
| Onde ficam as travas e a recusa no snapshot | Dois campos na raiz do `Snapshot`: `quality_locks` e `plan_notice`. Não dentro de `AccountView` | `AccountView` é refeito a cada `publish_account` (a recusa sumiria) e tem literal completo em `app/src-tauri/src/convert.rs` (um campo novo quebraria aquele crate) |
| Erros dos comandos continuam texto | O Tauri continua devolvendo `e.to_string()`. O que a interface precisa para o aviso vem de `plan_notice` | Não mexe nos ~40 comandos; também cobre a recusa assíncrona do `trilha.atualizar` |
| `kbps` de fonte larga e baixa | `fit_ladder`: no Online com plano, o `kbps` de cada camada é limitado ao teto do plano para o tipo, antes de conferir | A escada escala o `kbps` pela área quando a fonte é mais baixa que o preset (ex.: janela 2500×1000 em 1080p60 dá 9645 kbps). Sem isso, um preset liberado seria recusado. Só reduz: altura e fps não mudam |
| Escolha explícita × implícita | Qualidade **pedida** acima do plano (`start_share`/`update_share` com `preset`, `set_camera_quality`) é recusada. Ligar a câmera (`set_camera(true)` e webcam como fonte) usa o preset salvo **limitado** ao melhor que o plano permite, sem mudar a configuração | O preset da câmera é uma configuração que vale também no Local; recusar ligar a câmera por causa dela seria pior. A interface mostra o preset efetivo (§4.5) |
| `ref` no `trilha.publicar` | O app manda `"ref": <local_id>`; um `erro` com esse `ref` completa a publicação pendente na hora (antes: espera de 5 s e "servidor não respondeu") | O servidor de produção repete o `ref` do topo do pedido; o de teste usa o `local_id`. O mesmo valor serve aos dois |
| Criar/entrar com conta `nenhum` | O núcleo também confere `pode_entrar` antes do `POST /salas` e do `/entrar` | Mesma regra do item 3, para a recusa nunca depender só da tela |
| Descrição dos planos | Nomes, não números ("transmite em HD") | Decisão 2 |

## 3. Frentes e dependências

```
proto ──┐
        ├──▶ core ──▶ bridge ──▶ telas
net ────┘
```

| Id | Pasta | Resumo | Depende de |
|---|---|---|---|
| `proto` | `crates/zap-proto` (+ troca mecânica dos literais de `ControlMsg::Error` em `crates/zap-core` e `crates/zap-test-server`) | Extras do `erro`, `tolerant`, `ControlMsg::error` | — |
| `net` | `crates/zap-net` | `WsTx::send_ref` | — |
| `core` | `crates/zap-core` | `plan_gate`, `CoreError::Plan`, `quality_locks` e `plan_notice` no snapshot, conferências, `ref` no publicar | `proto`, `net` |
| `bridge` | `app/src/bridge` | Tipos, leitura de `config_publica`/`assinaturas`, `assinar`, sincronização de 1 h, foco, mock | `core` (nomes do JSON; dá para começar pela §4.3) |
| `telas` | `app/src/screens`, `app/src/components`, `app/src/App.tsx` | Cadeados, Vire Transmissor, convite, aviso, Conta, tela de planos | `bridge` |

Não há frente `app-tauri`: nenhum comando novo (o `abrirUrl` já abre `https://*`; o CSP já libera
`https://*.supabase.co`, que inclui `/rest/v1` e `/functions/v1`), e o `Snapshot` cruza a ponte por
serde.

## 4. Frentes em detalhe

### 4.1 `proto` (`crates/zap-proto`)

**`src/types.rs`**, ao lado de `camera_layout_tolerant`:

```rust
/// Lê um campo opcional sem derrubar a mensagem: ausente, `null`, tipo errado ou valor que esta
/// versão não conhece viram `None`. Use com `#[serde(default, deserialize_with = "crate::types::tolerant")]`.
pub fn tolerant<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    let valor = Option::<serde_json::Value>::deserialize(d)?;
    Ok(valor.and_then(|v| serde_json::from_value(v).ok()))
}
```

**`src/control.rs`**, variante `Error`:

```rust
#[serde(rename = "erro")]
Error {
    #[serde(rename = "codigo")]
    code: String,
    #[serde(rename = "mensagem")]
    message: String,
    #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
    reference: Option<String>,
    /// `invalido` (campo do pedido) e `plano_insuficiente` (`"tipo"` | `"camadas"`).
    #[serde(rename = "campo", default, deserialize_with = "crate::types::tolerant", skip_serializing_if = "Option::is_none")]
    field: Option<String>,
    /// `plano_insuficiente`: id do plano da sessão. Id desconhecido = `None`.
    #[serde(rename = "plano", default, deserialize_with = "crate::types::tolerant", skip_serializing_if = "Option::is_none")]
    plan: Option<crate::plan::PlanId>,
    /// `limite`: quanto esperar antes de tentar de novo.
    #[serde(rename = "tentar_em_ms", default, deserialize_with = "crate::types::tolerant", skip_serializing_if = "Option::is_none")]
    retry_ms: Option<u64>,
    /// `protocolo_incompativel`: faixa aceita.
    #[serde(rename = "protocolo_min", default, deserialize_with = "crate::types::tolerant", skip_serializing_if = "Option::is_none")]
    protocol_min: Option<u32>,
    #[serde(rename = "protocolo_max", default, deserialize_with = "crate::types::tolerant", skip_serializing_if = "Option::is_none")]
    protocol_max: Option<u32>,
},

impl ControlMsg {
    /// `erro` sem extras (o formato de antes da fase 3).
    pub fn error(code: impl Into<String>, message: impl Into<String>, reference: Option<String>) -> ControlMsg;
}
```

Sem `#[serde(flatten)]` (com enum de `tag` interna ele passa por buffer e tem casos ruins com
números).

**Troca mecânica** (mesma frente, porque sem ela o workspace não compila): todo literal que
**constrói** `ControlMsg::Error { code, message, reference }` passa a usar `ControlMsg::error(...)`.
Padrões com `..` não mudam. Lista de hoje:
- `zap-proto`: `src/framing.rs` (teste), `src/control/tests_session.rs`;
- `zap-test-server`: `src/music.rs`, `src/bridge.rs`, `src/signaling.rs`, `src/subs.rs`,
  `src/tracks.rs`, `src/ws.rs`;
- `zap-core`: `src/room/apply.rs` (teste), `src/room/dispatch.rs`, `src/publish/media_serve.rs`,
  `src/room/owner.rs` (testes), `src/online/ws_apply.rs` (teste).

Conferir com `grep -rn "ControlMsg::Error {" crates` que só sobraram padrões.

**Testes** (`src/control/tests_session.rs`):
1. `{"t":"erro","codigo":"plano_insuficiente","mensagem":"m","ref":"l_1","plano":"hd","campo":"camadas"}`
   → `field == Some("camadas")`, `plan == Some(PlanId::Hd)`; volta igual (`roundtrip`).
2. `{"t":"erro","codigo":"x","mensagem":"m"}` → extras todos `None`; volta igual, sem chaves novas.
3. Tolerância: `"plano":"ouro"` → `plan: None`; `"tentar_em_ms":"abc"` → `None`; `"campo":5` →
   `None`; `"protocolo_min":3,"protocolo_max":4` → `Some(3)`, `Some(4)`; em todos a mensagem
   inteira continua válida.
4. `ControlMsg::error("x", "m", Some("r".into()))` == o literal com extras `None`.

### 4.2 `net` (`crates/zap-net/src/online/ws.rs`)

```rust
/// Item da fila de saída do WebSocket.
pub(crate) struct Outgoing {
    pub msg: ControlMsg,
    pub reference: Option<String>,
}

impl WsTx {
    /// Como hoje (sem `ref`).
    pub async fn send(&self, msg: ControlMsg) -> Result<(), ApiError>;
    /// Igual a `send`, com `"ref": reference` no topo do JSON. O servidor repete o `ref` no `erro`
    /// que responder a esse pedido (`docs/api/mensagens.md`). Erro `Network("websocket fechado")`
    /// se a conexão caiu.
    pub async fn send_ref(&self, msg: ControlMsg, reference: String) -> Result<(), ApiError>;
}

/// JSON de saída (puro): `msg` serializado; com `reference`, acrescenta `"ref"` no objeto.
/// `None` se a serialização falhar (o item é descartado, como hoje).
pub(crate) fn outgoing_json(out: &Outgoing) -> Option<String>;
```

O canal interno passa a ser `mpsc::channel::<Outgoing>(64)`; o ping continua sem `ref`.

**Testes:** `outgoing_json` com e sem `ref` (com: `v["ref"] == "l_1"` e o resto igual a
`serde_json::to_value(msg)`; sem: nenhuma chave `ref`); um teste com o servidor axum de `ws.rs`
recebendo `send_ref(TrackRemove{..}, "l_1")` com `"ref":"l_1"`.

### 4.3 `core` (`crates/zap-core`)

#### Módulo novo `src/plan_gate.rs` (puro: sem I/O, sem tokio)

```rust
//! Plano da sessão Online aplicado no app (`docs/planos/fase3-app-planos.md`): cadeados das
//! qualidades e conferência antes de mandar ao servidor. Os números vêm só de `zap_proto::plan`
//! e das escadas do `zap-rate`.
use serde::{Deserialize, Serialize};
use zap_proto::plan::{check_publish, limits, PlanError, PlanId, PlanInfo, PlanLimits};
use zap_proto::{LayerSpec, TrackKind};

use crate::types::Preset;

/// As 4 qualidades, na ordem de exibição (e da menor para a maior).
pub const PRESETS: [Preset; 4] = [Preset::P720p30, Preset::P720p60, Preset::P1080p30, Preset::P1080p60];

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanAction {
    /// Criar ou entrar em sala Online.
    #[serde(rename = "entrar")]
    Join,
    /// Publicar ou trocar a escada de uma trilha.
    #[serde(rename = "transmitir")]
    Publish,
}

/// Recusa por plano (da conferência local ou do `erro` do servidor).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanRefusal {
    pub plan: PlanId,
    pub action: PlanAction,
    /// `"tipo"` | `"camadas"` ao transmitir; `None` ao entrar.
    pub field: Option<String>,
    /// Texto em português para a interface.
    pub message: String,
}

/// Um preset acima do plano.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresetLock {
    pub preset: Preset,
    /// Plano mais barato que libera (`hd` ou `fullhd`); `None` = nenhum plano libera.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs: Option<PlanId>,
}

/// `Snapshot.quality_locks`: só existe com plano na sessão Online (conta).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct QualityLocks {
    /// O plano transmite alguma coisa (tela, janela, câmera ou só o som).
    pub can_publish: bool,
    /// Tela e janela, na ordem de `PRESETS`.
    pub screen: Vec<PresetLock>,
    pub camera: Vec<PresetLock>,
}

/// `Snapshot.plan_notice`: a última recusa. `seq` cresce a cada recusa (a interface mostra o aviso
/// uma vez por `seq`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PlanNotice {
    pub seq: u64,
    pub plan: PlanId,
    pub action: PlanAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    pub message: String,
}

/// Textos (os mesmos da interface).
pub const MSG_NAO_TRANSMITE: &str = "Seu plano não permite transmitir no Online";
pub const MSG_QUALIDADE: &str = "Seu plano não permite essa qualidade";
pub const MSG_NAO_ENTRA: &str = "Seu plano não permite entrar em salas Online";

/// Escada usada para os cadeados: fonte 1920×1080 (16:9) e câmera a 60 fps.
/// Tela e janela: `screen_ladder(p, 1920, 1080)`; câmera: `camera_ladder(p, 1920, 1080, 60)`;
/// áudio: vazia.
pub fn nominal_ladder(kind: TrackKind, preset: Preset) -> Vec<LayerSpec>;

/// `kbps` de cada camada = `min(kbps, teto do tipo)` (tela/janela: `screen_kbps`; câmera:
/// `camera_kbps`). Áudio e plano sem `publish`: devolve igual. Nunca aumenta nada.
pub fn fit_ladder(kind: TrackKind, ladder: Vec<LayerSpec>, l: &PlanLimits) -> Vec<LayerSpec>;

/// `check_publish(kind, &fit_ladder(kind, nominal_ladder(kind, p), l), l).is_ok()`.
pub fn preset_allowed(kind: TrackKind, preset: Preset, l: &PlanLimits) -> bool;

/// O primeiro de `[Hd, FullHd]` (pela tabela `zap_proto::plan::limits`) em que `preset_allowed`.
pub fn cheapest_plan_for(kind: TrackKind, preset: Preset) -> Option<PlanId>;

/// Presets travados com os limites `l` (os da sessão, vindos do servidor).
pub fn quality_locks(l: &PlanLimits) -> QualityLocks;

/// O maior preset permitido com altura e fps <= os do pedido (ordem: altura, depois fps). Nenhum
/// permitido: devolve o próprio pedido (a conferência recusa depois).
pub fn best_allowed(kind: TrackKind, preset: Preset, l: &PlanLimits) -> Preset;

/// `check_publish` → `PlanRefusal { action: Publish }`: `CannotPublish` → field `"tipo"` e
/// `MSG_NAO_TRANSMITE`; `AboveLimit` → field `"camadas"` e `MSG_QUALIDADE`.
pub fn check_layers(kind: TrackKind, layers: &[LayerSpec], plan: &PlanInfo) -> Result<(), PlanRefusal>;

/// `can_join` falso → `PlanRefusal { action: Join, field: None, message: MSG_NAO_ENTRA }`.
pub fn check_join(plan: &PlanInfo) -> Result<(), PlanRefusal>;

/// Erro de pedido vindo do servidor (`erro` do WebSocket): `plano_insuficiente` →
/// `CoreError::Plan` (plano do `erro`, senão `fallback`, senão `PlanId::None`; a mensagem do
/// servidor); qualquer outro código → `CoreError::Server { code, message }`.
pub fn error_from_server(
    code: &str,
    message: &str,
    plan: Option<PlanId>,
    field: Option<String>,
    fallback: Option<PlanId>,
    action: PlanAction,
) -> crate::CoreError;
```

Valores esperados de `quality_locks(&limits(id))` (mesmos para `screen` e `camera`):

| Plano | `can_publish` | Travados (`preset` → `needs`) |
|---|---|---|
| `nenhum`, `espectador` | `false` | 720p30 → hd, 720p60 → hd, 1080p30 → fullhd, 1080p60 → fullhd |
| `hd` | `true` | 1080p30 → fullhd, 1080p60 → fullhd |
| `fullhd` | `true` | nenhum |

Conta das escadas nominais (para conferir): tela 720p60 = 1280×720 a 60, 4000 kbps (cabe no HD:
4000); tela 1080p30 = 1920×1080 (passa da altura 720); câmera 720p60 = 2500 (teto HD da câmera:
2500); câmera 1080p60 = 4500 (teto Full HD: 4500).

JSON (`hd`):

```json
"quality_locks": {"can_publish": true,
  "screen": [{"preset": "1080p30", "needs": "fullhd"}, {"preset": "1080p60", "needs": "fullhd"}],
  "camera": [{"preset": "1080p30", "needs": "fullhd"}, {"preset": "1080p60", "needs": "fullhd"}]}
"plan_notice": {"seq": 1, "plan": "hd", "action": "transmitir", "field": "camadas",
  "message": "Seu plano não permite essa qualidade"}
```

#### `src/error.rs`

```rust
/// Ação acima do plano da sessão Online (`plano_insuficiente`), conferida no app ou recusada pelo
/// servidor. A mensagem vai para a interface; o resto vai para `Snapshot.plan_notice`.
#[error("{}", .0.message)]
Plan(crate::plan_gate::PlanRefusal),
```

`code()`: `CoreError::Plan(_) => "plano_insuficiente"`.

#### `src/types.rs`

`impl Preset { pub fn height_fps(self) -> (u32, u32) }` (720/1080, 30/60): usado por `best_allowed`.

#### Estado e snapshot (`src/state.rs`, `src/snapshot.rs`)

- `Snapshot` ganha, com `#[serde(default, skip_serializing_if = "Option::is_none")]`:
  `quality_locks: Option<QualityLocks>` e `plan_notice: Option<PlanNotice>`. `Snapshot::example()`
  com os dois `None`.
- `UiState` ganha `quality_locks`, `plan_notice` e `plan_seq: u64`:
  - `set_account(a)` também calcula `quality_locks = a.plan.map(|p| plan_gate::quality_locks(&p.limits))`
    (uma vez por mudança de conta, não a cada snapshot);
  - `set_plan_notice(r: &PlanRefusal) -> u64`: `plan_seq += 1`, guarda o `PlanNotice` com esse
    `seq` e devolve o `seq`;
  - `clear_plan_notice()`.
- `OnlineSession::clear` (`online/session.rs`, chamado ao desligar o Online e ao reiniciar a
  sessão) chama `clear_plan_notice()` junto do `set_account(None)`.

#### Plano do `Shared` (arquivo novo `src/engine_plan.rs`, `impl Shared`)

```rust
impl Shared {
    /// Plano que limita a sala atual: `Some` só com `ActiveRoom::Online` e sessão com plano.
    pub(crate) fn room_plan(&self) -> Option<PlanInfo>;
    /// Plano da sessão Online (criar/entrar): `self.online.client()?.plan()`.
    pub(crate) fn session_plan(&self) -> Option<PlanInfo>;
    /// Sem `room_plan`: devolve `ladder` como veio. Com plano: `fit_ladder` e `check_layers`;
    /// recusa → `CoreError::Plan`.
    pub(crate) fn plan_ladder(&self, kind: TrackKind, ladder: Vec<LayerSpec>) -> Result<Vec<LayerSpec>, CoreError>;
    /// Preset da câmera que vale agora: sem `room_plan`, o próprio; com plano, `best_allowed(Camera, …)`.
    pub(crate) fn camera_preset_for_plan(&self, preset: Preset) -> Preset;
    /// `state.set_plan_notice(r)` e `notifier.touch()`.
    pub(crate) fn record_plan_refusal(&self, r: &PlanRefusal);
}
```

#### Onde a conferência acontece

Sempre **antes** de qualquer mudança local (publicador, escada, captura, configuração):

| Lugar | O que faz |
|---|---|
| `share.rs`, `start_locked` (tela/janela) | `let ladder = shared.plan_ladder(kind, Self::ladder_for(&src, req.preset))?;` antes do `ensure_media_server` e do `publish_track`. O `kind` é calculado antes |
| `share.rs`, `start_audio_only` | `shared.plan_ladder(TrackKind::Audio, vec![])?` antes de tudo |
| `share.rs`, `start_audio` | Igual, antes do `publish_track` (com vídeo junto já passou; o erro continua sendo engolido com aviso, como hoje) |
| `share.rs`, `update` (mesma fonte) | `plan_ladder` antes do `set_ladder`; recusa mantém a escada e o pedido antigos |
| `share_camera.rs`, `start_camera_locked` | `let preset = shared.camera_preset_for_plan(preset);` antes do `plan_with`; depois `plan.ladder = shared.plan_ladder(TrackKind::Camera, plan.ladder)?;` antes do `publish_track`. A configuração não muda |
| `share_camera.rs`, `set_camera_quality` | `shared.plan_ladder(Camera, novo.ladder)?` antes do `set_ladder`/`restart_capture`; recusa: a câmera segue igual |
| `engine_share.rs`, `Core::set_camera_quality` | Com `room_plan` e `!preset_allowed(Camera, preset, …)`: devolve `CoreError::Plan` **sem** gravar `camera_preset` |
| `engine_share.rs`, `fecha_share` | `Err(CoreError::Plan(r))` → `record_plan_refusal(r)` além do `push_error` de hoje |
| `online/entry.rs`, `create_online` e `join_online` | Depois de "ligado" e "sem sala": `if let Some(p) = shared.session_plan() { check_join(&p).map_err(CoreError::Plan)?; }`. Erro REST `plano_insuficiente` vindo de `OnlineRoom::create/join` vira `CoreError::Plan` (plano = `session_plan`, `field: None`, mensagem do servidor) |
| `engine_room.rs`, `create_room`/`join_room` | `Err(CoreError::Plan(r))` → `record_plan_refusal(r)` |
| `online/room_tracks.rs`, `publish_track` e `update_track` (com `layers`) | Defesa: `check_layers` com `shared.session_plan()` antes de mandar. Não há `fit` aqui: quem chama já ajustou |

`room_preview` continua sem conferência (a prévia é liberada para `nenhum`).

#### `ref` e recusa do servidor

- `online/room.rs`: `pending: Mutex<HashMap<String, oneshot::Sender<Result<Track, CoreError>>>>`.
- `online/room_tracks.rs::publish_track`: `ws.send_ref(msg, local_id.clone())`; resposta
  `Ok(Ok(Ok(track)))` como hoje, `Ok(Ok(Err(e)))` → `Err(e)`.
- `online/room_events.rs::run`, passo novo antes de aplicar ao modelo:

```rust
if let ControlMsg::Error { code, message, reference: Some(r), plan, field, .. } = &msg {
    if let Some(tx) = room.pending.lock().remove(r) {
        let fallback = room.shared().ok().and_then(|s| s.session_plan()).map(|p| p.id);
        let _ = tx.send(Err(plan_gate::error_from_server(code, message, *plan, field.clone(), fallback, PlanAction::Publish)));
        continue; // não vira RoomEvent: quem publicou recebe o erro e o `fecha_share` registra
    }
}
```

- `room/mod.rs`: `RoomEvent::PlanRefused(PlanRefusal)`.
- `online/ws_apply.rs`: `Error` com `code == "plano_insuficiente"` (sem pendente) →
  `RoomEvent::PlanRefused(PlanRefusal { plan: plan.unwrap_or_default(), action: Publish, field, message })`;
  os outros códigos continuam `RoomEvent::Error(message)`.
- `engine_room.rs::pump_room_events`: `PlanRefused(r)` → `record_plan_refusal(&r)` e
  `push_error(r.message)`, num lock só. Outros `match` de `RoomEvent` (ex.: `online/preview.rs`)
  ganham o braço novo se forem exaustivos.

#### Exportações

`pub mod plan_gate;` em `lib.rs`; `pub use plan_gate::{PlanAction, PlanNotice, PresetLock, QualityLocks};`.

#### Testes do `core`

Unidade (`plan_gate.rs`):
1. Tabela de `quality_locks` acima, para os 4 planos, tela e câmera.
2. `cheapest_plan_for`: tela 720p60 → `Hd`; tela 1080p30 → `FullHd`; câmera 1080p60 → `FullHd`;
   câmera 720p30 → `Hd`.
3. `fit_ladder(Window, screen_ladder(P1080p60, 2500, 1000), &limits(FullHd))`: camada 0
   2500×1000 a 60 com **8000** kbps (sem o ajuste: 9645) e `check_layers` passa; sem o ajuste,
   `check_publish` recusa com `what: "kbps"`. Câmera com camada 0 de 5000 kbps no Full HD → 4500.
   Áudio volta igual. Nenhum valor aumenta.
4. `best_allowed(Camera, P1080p60, &limits(Hd))` = `P720p60`; `P1080p30` → `P720p30`;
   `P720p30` → `P720p30`; com `limits(Viewer)` devolve o próprio pedido.
5. `check_layers`: Espectador + áudio → `field "tipo"`, `MSG_NAO_TRANSMITE`; HD + tela 1080p30
   nominal → `field "camadas"`, `MSG_QUALIDADE`. `check_join(nenhum)` → `MSG_NAO_ENTRA`, sem field.
6. `error_from_server("plano_insuficiente", "m", Some(Hd), Some("camadas"), None, Publish)` →
   `CoreError::Plan` com esses valores; `plan: None` com `fallback: Some(FullHd)` → `FullHd`;
   `"invalido"` → `CoreError::Server`. `CoreError::Plan(..).code() == "plano_insuficiente"`.
7. `UiState`: duas recusas → `seq` 1 e 2; `clear_plan_notice` → `None`; `set_account` com plano
   `hd` → `quality_locks` da tabela; sem plano → `None`. JSON do snapshot com e sem os dois campos.

Integração com `zap-test-server` (`requires_account: true`, `account` preenchido, token
`teste:u1:<plano>`; fonte `synthetic:1920x1080`; mesmo molde de `online/account_apply.rs`):
8. Snapshot: `hd` → `quality_locks` da tabela; `fullhd` → listas vazias e `can_publish`;
   `espectador` → `can_publish: false` e 4+4 travados. Servidor sem conta e sem token →
   `quality_locks` ausente.
9. Sala Online, `hd`: `start_share` com `P1080p60` → `Err(CoreError::Plan)` com `field "camadas"`
   em menos de 200 ms; a sala do servidor (`srv.hub.inner.lock().rooms`) tem **0 trilhas** (não
   foi mandado); `snapshot.plan_notice` = `{seq: 1, plan: hd, action: transmitir, field: camadas}`;
   `snapshot.share` vazio. Depois `P720p60` → `Ok`, camada 0 1280×720 a 60 com 4000 kbps.
10. `hd`, câmera: `camera_preset = P1080p60` nas configurações, `set_camera(true)` → `Ok`, camada 0
    1280×720 a 60 com 2500 kbps, e `settings.video.camera_preset` continua `P1080p60`.
    `set_camera_quality(P1080p30)` → `Err(Plan)`, a trilha da câmera é a mesma (mesmo id, mesma
    escada) e a configuração não mudou.
11. `espectador`: `create_room` Online → `Ok`; `start_share` com `P720p30` → `Err(Plan)` com
    `field "tipo"`; só o som → `Err(Plan)` `"tipo"`; servidor com 0 trilhas.
12. `nenhum`: `create_room` Online → `Err(Plan)` com `action: entrar`, sem `field`; o servidor não
    tem sala nova; `plan_notice.action == Join`.
13. Sem plano: sala Local com sessão `hd` ligada → `start_share` `P1080p60` passa; Online com
    servidor sem conta → `P1080p60` passa.
14. Recusa do servidor (unidade de `room_events`, como `track_publicada_completa_pending_e_entra_no_modelo`):
    pendente `l_9` + `erro {plano_insuficiente, ref: l_9, plano: hd, campo: camadas}` → o
    `oneshot` recebe `Err(CoreError::Plan)` com esses valores e nenhum `RoomEvent` sai; o mesmo
    `erro` sem `ref` → `RoomEvent::PlanRefused` e o pendente fica.
15. `ref` de verdade: no `zap-test-server` (que usa o `local_id` como `ref`; servidor sem conta),
    `OnlineRoom::publish_track` de uma trilha `tela` com `layers: vec![]` é recusado com
    `invalido` ("camadas inválidas") e volta em **menos de 1 s** com
    `CoreError::Server { code: "invalido" }` (antes: 5 s e "servidor não respondeu"); o pendente
    sai do mapa.
16. Desligar o Online zera `plan_notice` e `quality_locks`.

### 4.4 `bridge` (`app/src/bridge`)

#### `types.ts`

```ts
export type PlanId = 'nenhum' | 'espectador' | 'hd' | 'fullhd'
export interface PlanView { id: PlanId; pode_entrar: boolean }
/** Preset acima do plano; `needs` = plano mais barato que libera (ausente = nenhum libera). */
export interface PresetLock { preset: Preset; needs?: PlanId }
/** `Snapshot.quality_locks`: só com plano na sessão Online (conta). Ausente = nada travado. */
export interface QualityLocks { can_publish: boolean; screen: PresetLock[]; camera: PresetLock[] }
/** Última recusa por plano; `seq` cresce a cada recusa. */
export interface PlanNotice { seq: number; plan: PlanId; action: 'entrar' | 'transmitir'; field?: string; message: string }
// Snapshot: + quality_locks?: QualityLocks; plan_notice?: PlanNotice
```

#### `contaTipos.ts` (acréscimos, compatíveis com o que existe)

```ts
export interface ContaResposta<T> { data: T | null; error: ContaErro | null }
/** Só o que o app usa do PostgREST: `from(t).select(cols).maybeSingle()` (thenable no supabase-js). */
export interface ContaConsulta { select(colunas: string): { maybeSingle(): PromiseLike<ContaResposta<unknown>> } }
// ContaClient:
//   from(tabela: 'config_publica' | 'assinaturas'): ContaConsulta
//   auth.refreshSession(): Promise<{ data: { session: ContaSessao | null } | null; error: ContaErro | null }>
//   functions.invoke(nome: string, opcoes?: { body?: unknown }): Promise<{ data?: unknown; error: ContaErro | null }>
```

`data?` opcional mantém os mocks de hoje (`{ error: null }`) válidos.

#### `planoTipos.ts` (novo)

```ts
import type { PlanId } from './types'
export type PlanoPago = 'espectador' | 'hd' | 'fullhd'
export type Ciclo = 'mensal' | 'anual'
/** Centavos (`config_publica.precos`). */
export type Precos = Record<PlanoPago, Record<Ciclo, number>>
/** Invariante: `cobrancaAtiva === true` implica `precos !== null`. */
export interface ConfigPublica { cobrancaAtiva: boolean; planoPadrao: PlanId | null; precos: Precos | null }
export type StatusAssinatura = 'ativa' | 'atrasada' | 'cancelada'
export interface Assinatura { plano: PlanoPago; status: StatusAssinatura; ciclo: Ciclo; validoAte: string }
export interface PlanoEstado {
  config: ConfigPublica
  assinatura: Assinatura | null
  /** Tela de planos aberta (só vale com cobrança). */
  telaAberta: boolean
  destaque: PlanoPago | null
  /** O link do pagamento foi aberto: o foco da janela atualiza a sessão. */
  aguardandoPagamento: boolean
  /** Plano comprado no último `assinar` (para saber quando o pagamento chegou). */
  comprado: PlanoPago | null
  ocupado: boolean
  /** Erro (role="alert"). */
  aviso: string | null
  /** Informação (role="status"), ex.: "Plano atualizado: Transmissor HD." */
  info: string | null
}
```

#### `planoModel.ts` (novo, puro)

```ts
export const CONFIG_DESLIGADA: ConfigPublica = { cobrancaAtiva: false, planoPadrao: null, precos: null }
/** Linha de `config_publica`. Qualquer coisa fora do formato = `CONFIG_DESLIGADA` (com o
 *  `planoPadrao` lido, se válido). `cobrancaAtiva` só é true com `cobranca_ativa === true` e
 *  `precos` completos (3 planos × 2 ciclos, inteiros > 0). */
export function lerConfigPublica(data: unknown): ConfigPublica
/** Linha de `assinaturas`; fora do formato ou null = null. */
export function lerAssinatura(data: unknown): Assinatura | null
/** 390 → 'R$ 3,90'; 3900 → 'R$ 39'; 1490 → 'R$ 14,90'; 100000 → 'R$ 1.000'. */
export function formatarPreco(centavos: number): string
/** 'R$ 3,90/mês ou R$ 39/ano'. */
export function textoPreco(precos: Precos, plano: PlanoPago): string
/** 'Assine o Espectador (R$ 3,90/mês ou R$ 39/ano) para assistir'. */
export function textoConvite(precos: Precos): string
/** max(0, 12 - round(anual / mensal)): 390/3900 → 2. */
export function mesesDeGraca(precos: Precos, plano: PlanoPago): number
/** Selo: nenhum 'Sem plano', espectador 'Espectador', hd 'HD', fullhd 'Full HD'. */
export function seloPlano(id: PlanId): string
/** Linha "Plano:" da Conta. Sem cobrança: id = plano?.id ?? config.planoPadrao; 'nenhum' →
 *  'Plano: sem plano'; outro → 'Plano: beta gratuito (<seloPlano>)'; sem id → null.
 *  Com cobrança: 'Plano: ' + nomePlano(plano) (sem plano → null). */
export function textoPlanoConta(plano: PlanView | undefined, config: ConfigPublica): string | null
/** dd/mm/aaaa no fuso de São Paulo (`Intl`, timeZone 'America/Sao_Paulo'). */
export function formatarData(iso: string): string
/** ativa → 'Ativa · válida até 10/12/2026'; atrasada → 'Pagamento atrasado · vale até 10/12/2026';
 *  cancelada com validade no futuro → 'Cancelada · vale até 10/12/2026'; validade no passado →
 *  'Encerrada em 10/12/2026'. */
export function textoAssinatura(a: Assinatura, agora: Date): string
/** Só dígitos. '' → ''; 11 ou 14 dígitos → os dígitos; outro tamanho → null. */
export function limparDocumento(texto: string): string | null
/** A trava do preset, se houver. */
export function travaDe(preset: Preset, travas: PresetLock[]): PresetLock | undefined
/** O maior preset sem trava com altura e fps <= os do pedido; sem nenhum livre, o próprio pedido.
 *  Usa as medidas de `screens/criar/cameraModel.ts` (`MEDIDAS` passa a ser exportado). */
export function melhorPermitido(preset: Preset, travas: PresetLock[]): Preset
```

`nomePlano` (em `contaModel.ts`) passa a receber `PlanView | { id: PlanId }`.

`melhorPermitido` precisa da altura e do fps de cada preset. A `bridge` cria
`app/src/bridge/presets.ts` com `MEDIDAS_PRESET: Record<Preset, { altura: number; fps: number }>`
e `PRESETS_ORDEM: Preset[]` (do menor para o maior); a `telas` faz `cameraModel.ts` importar de lá
no lugar do `MEDIDAS` privado dele. São medidas do preset, não limites de plano.

#### `planoStore.ts` (novo)

Mesmo molde de `contaStore.ts`: `estadoInicialPlano()` (`config: CONFIG_DESLIGADA`, resto vazio ou
`false`), `getPlanoEstado`, `setPlanoEstado(patch)`, `usePlanoEstado()`, `resetPlano()`.

#### `planoAcoes.ts` (novo; usam `clienteAtual()`; nunca lançam)

```ts
/** `from('config_publica').select('cobranca_ativa,plano_padrao,precos').maybeSingle()`.
 *  Erro, exceção ou sem cliente → `config = CONFIG_DESLIGADA` (nunca mostra preço por erro).
 *  Cobrança desligou: fecha a tela de planos e zera `aguardandoPagamento`. */
export async function carregarConfig(): Promise<void>
/** `from('assinaturas').select('plano,status,ciclo,valido_ate').maybeSingle()`. Erro → null.
 *  Com `aguardandoPagamento` e uma assinatura `ativa` do plano `comprado` com validade no
 *  futuro: `aguardandoPagamento = false`, `info = 'Plano atualizado: <nomePlano>.'`. */
export async function carregarAssinatura(): Promise<void>
/** Só com `config.cobrancaAtiva`: `telaAberta = true`, `destaque`. Sem cobrança: nada. */
export function abrirPlanos(destaque?: PlanoPago): void
export function fecharPlanos(): void
/** Documento: `limparDocumento`; null → aviso 'Informe um CPF ou CNPJ válido.' sem chamar nada.
 *  `functions.invoke('assinar', { body: { plano, ciclo, cpf_cnpj? } })` (sem a chave quando ''). 
 *  Erro → `aviso = await mensagemDaFuncao(error, 'Não foi possível assinar agora. Tente de novo.')`.
 *  Ok: `data.link` precisa ser string começando com 'https://' (senão aviso 'Não foi possível
 *  abrir o pagamento.'); `api.abrirUrl(link)`, `aguardandoPagamento = true`, `comprado = plano`. */
export async function assinar(plano: PlanoPago, ciclo: Ciclo, documento: string): Promise<void>
/** `auth.refreshSession()` (o TOKEN_REFRESHED leva o token novo ao núcleo pelo `contaSync`, que
 *  já existe) e `carregarAssinatura()`. Ainda sem a assinatura nova: `info = 'Ainda não recebemos
 *  a confirmação do pagamento. Tente de novo em alguns minutos.'`. */
export async function atualizarDepoisDePagar(): Promise<void>
/** Para de esperar o pagamento (botão Fechar do aviso). */
export function pararDeEsperar(): void
```

#### `planoSync.ts` (novo) e ligação em `contaSync.ts`

```ts
export const INTERVALO_PLANOS_MS = 3_600_000
/** Foco seguido de foco: no máximo uma atualização a cada 10 s. */
export const ESPERA_FOCO_MS = 10_000
export interface PlanosSync { aoEntrar(): void; aoSair(): void; parar(): void }
/** Chamado por `iniciarConta` depois de `obterCliente`: `carregarConfig()` na hora e a cada 1 h
 *  (com sessão, `carregarAssinatura()` também); ouve `window` 'focus': com `aguardandoPagamento`,
 *  `atualizarDepoisDePagar()` (no máximo 1 a cada `ESPERA_FOCO_MS`). */
export function iniciarPlanos(): PlanosSync
```

Em `contaSync.ts::iniciarConta`:
- depois do `obterCliente`: `planos = iniciarPlanos()`;
- `INITIAL_SESSION`/`SIGNED_IN` com sessão: `setTimeout(() => planos.aoEntrar(), 0)` (fora do
  callback, como as identidades: chamar o Supabase dentro dele trava o supabase-js);
  `aoEntrar` = `carregarAssinatura()`;
- `SIGNED_OUT` e `INITIAL_SESSION` sem sessão: `planos.aoSair()` (`assinatura = null`,
  `aguardandoPagamento = false`, tela fechada);
- a limpeza devolvida também chama `planos.parar()` (intervalo, ouvinte de foco) e `resetPlano()`.

Online desligado ou servidor sem `conta`: `iniciarConta` já sai antes de tudo, então nenhuma
chamada ao Supabase acontece (aceite A).

#### `usePlano.ts` (novo)

```ts
/** Travas que valem para um modo de sala: Local, Online desligado ou snapshot sem `quality_locks`
 *  → nada travado (`ativo: false`, listas vazias, `podeTransmitir: true`, `podeEntrar: true`). */
export interface Travas { ativo: boolean; tela: PresetLock[]; camera: PresetLock[]; podeTransmitir: boolean; podeEntrar: boolean }
export function travasDo(snap: Snapshot, modo: RoomMode, onlineLigado: boolean): Travas   // puro
export function useTravas(modo: RoomMode): Travas
export function useCobrancaAtiva(): boolean
```

`podeEntrar` vem de `snap.account?.plan?.pode_entrar ?? true` (só no Online).

#### `mockConta.ts`

```ts
export const PRECOS_MOCK = { espectador: { mensal: 390, anual: 3900 }, hd: { mensal: 790, anual: 7900 }, fullhd: { mensal: 1490, anual: 14900 } }
export interface MockContaOpcoes {
  /** Padrão: `?cobranca=1` na URL (navegador), senão false. */
  cobrancaAtiva?: boolean
  /** Linha inicial de `assinaturas` (formato do banco). */
  assinatura?: { plano: string; status: string; ciclo: string; valido_ate: string } | null
  /** `from('config_publica')` devolve erro (testa "falha = cobrança desligada"). */
  falharConfig?: boolean
}
export function criarMockConta(opcoes?: MockContaOpcoes): ContaClient
```

Regras novas do mock:
- `from('config_publica')` → `{ cobranca_ativa, plano_padrao: 'fullhd', precos: PRECOS_MOCK }`;
- `from('assinaturas')` → a linha (só com sessão; sem sessão, `null`);
- `functions.invoke('assinar', { body })`, na ordem: sem sessão → `nao_autenticado`; cobrança
  desligada → `cobranca_desligada` "As assinaturas ainda não estão abertas."; plano ou ciclo fora
  da lista → `invalido` "Pedido inválido."; `cpf_cnpj` presente com tamanho diferente de 11 e 14 →
  `invalido` "Informe um CPF ou CNPJ válido."; mesmo plano da assinatura ativa → `ja_assinante`
  "Você já tem esse plano."; senão `{ data: { link: 'https://sandbox.asaas.com/i/mock-<plano>-<ciclo>' } }`
  e a assinatura vira `ativa`, com `valido_ate` = agora + 30 dias (mensal) ou 365 (anual), como se
  o pagamento fosse na hora. Erros com `context: { json: async () => ({ erro, mensagem }) }`, o
  formato que `mensagemDaFuncao` lê;
- `auth.refreshSession()` → token `mock-token-<n>` (n = 2, 3, …) e emite `TOKEN_REFRESHED`.

#### `mock.ts`

`?plano=nenhum|espectador|hd|fullhd` na URL do navegador liga `account.plan` e `quality_locks`
de exemplo, de `fixtures/travas.ts` (espelho da tabela de §4.3; comentário dizendo que o teste do
núcleo é a fonte). Sem o parâmetro, nada muda.

#### Testes da `bridge` (vitest)

1. `lerConfigPublica`: linha completa → ativa com preços; `cobranca_ativa: true` sem `hd.anual`
   → desligada; `null`, string, preço 0 ou negativo → desligada; `plano_padrao: 'ouro'` →
   `planoPadrao: null`.
2. `formatarPreco` (os 4 exemplos), `textoPreco`, `textoConvite` (texto exato), `mesesDeGraca` = 2
   para os 3 planos, `limparDocumento('123.456.789-09') === '12345678909'`, 12 dígitos → null.
3. `textoPlanoConta`: sem cobrança + plano `fullhd` → 'Plano: beta gratuito (Full HD)'; sem
   cobrança, sem plano na sessão, `planoPadrao: 'hd'` → 'Plano: beta gratuito (HD)'; com cobrança
   + `hd` → 'Plano: Transmissor HD'.
4. `textoAssinatura` com `'2026-12-10T15:00:00Z'` e agora `2026-11-11`: os 4 casos.
5. `melhorPermitido('1080p60', travas do hd) === '720p60'`; `'1080p30'` → `'720p30'`; sem travas →
   o próprio.
6. `travasDo`: Local → nada travado; Online com `quality_locks` → as listas; Online desligado →
   nada travado; Online sem `quality_locks` → nada travado.
7. Sync: `iniciarConta(conta, false)` → o mock nunca recebe `from`; ligado → `from('config_publica')`
   1 vez ao iniciar e de novo depois de `INTERVALO_PLANOS_MS` (relógio falso); `SIGNED_IN` →
   `from('assinaturas')`; `SIGNED_OUT` → `assinatura: null`; limpeza para o intervalo (avançar 2 h
   depois da limpeza não chama nada).
8. `falharConfig` → `cobrancaAtiva: false` e `abrirPlanos()` não abre.
9. `assinar('hd', 'anual', '')`: corpo `{ plano: 'hd', ciclo: 'anual' }` (sem `cpf_cnpj`);
   `api.abrirUrl` com o link; `aguardandoPagamento: true`. Com cobrança desligada no mock: aviso
   'As assinaturas ainda não estão abertas.' e `abrirUrl` não é chamado. Link `http://…` → aviso
   'Não foi possível abrir o pagamento.'. Documento com 12 dígitos → aviso sem `invoke`.
10. Foco: com `aguardandoPagamento`, `window.dispatchEvent(new Event('focus'))` → `refreshSession`
    1 vez; dois focos em 5 s → 1 vez; depois de 10 s → 2; sem esperar pagamento → 0. Depois do
    `refreshSession` do mock, `api.onlineSetAccount` recebe `mock-token-2` (fluxo que já existe) e
    `info` = 'Plano atualizado: Transmissor HD.'.

### 4.5 `telas` (`app/src/screens`, `app/src/components`, `app/src/App.tsx`)

Regra geral: as travas valem quando `useTravas(modo).ativo` (sala ou criação **Online** com plano).
Preço, "Ver planos", "Assinar" e a tela de planos só com `useCobrancaAtiva()`.

#### Componentes novos

| Arquivo | O que é |
|---|---|
| `components/SeloPlano.tsx` | `SeloPlano({ plano: PlanId, cadeado?: boolean })`: ícone de cadeado (SVG em linha) + `seloPlano(plano)` ('Full HD'). Sem texto de preço |
| `screens/planos/useCliqueTravado.ts` | `useCliqueTravado(): { aoClicar(lock: PresetLock): void; explicacao: string \| null }`. Com cobrança: `abrirPlanos(lock.needs === 'fullhd' ? 'fullhd' : 'hd')`. Sem: `explicacao = 'Seu plano não permite essa qualidade'` |
| `screens/planos/VireTransmissor.tsx` | Cartão "Vire Transmissor". Com cobrança: 'Para transmitir no Online, assine o Transmissor HD ou o Transmissor Full HD.' + botão 'Ver planos' (`abrirPlanos('hd')`). Sem: 'Seu plano não permite transmitir no Online' |
| `screens/planos/ConviteEspectador.tsx` | Com cobrança: `textoConvite(precos)` + 'Ver planos' (`abrirPlanos('espectador')`). Sem: 'Sua conta não tem um plano para entrar em salas Online' |
| `screens/planos/AvisoPlano.tsx` | Faixa global (como o `UpdateBanner`): aparece quando `snap.plan_notice.seq` > o último `seq` fechado (estado do componente, começa em 0). Texto = `message`; 'Ver planos' só com cobrança (destaque: `action === 'entrar'` → `espectador`; `'transmitir'` com `plan === 'hd'` → `fullhd`; senão `hd`); 'Fechar' guarda o `seq` |
| `screens/planos/PlanosModal.tsx` | A tela de planos (abaixo). `role="dialog"`, `aria-label="Planos do Online"`. Só renderiza com `telaAberta && config.cobrancaAtiva` |
| `screens/config/ContaPlano.tsx` | Parte do plano dentro de `ContaLogada` (abaixo) |

#### Tela de planos (`PlanosModal`)

- Título 'Planos do Online'. Ciclo: segmentado 'Anual no Pix' (padrão) e 'Mensal no cartão'; no
  anual, selo `'<n> meses de graça'` com `mesesDeGraca` (escondido se 0).
- Três cartões, nesta ordem:
  - **Espectador**: 'Entra nas salas Online e assiste a tudo o que transmitirem';
  - **Transmissor HD**: 'Tudo do Espectador, e transmite em HD';
  - **Transmissor Full HD**: 'Tudo do Espectador, e transmite em Full HD'.
- Preço do cartão no ciclo escolhido: 'R$ 39/ano' ou 'R$ 3,90/mês' (`formatarPreco`).
- O cartão de `destaque` tem a borda verde. O plano atual (assinatura `ativa`/`atrasada` com
  validade no futuro; senão `snap.account.plan.id`) tem 'Seu plano' e o botão desligado 'Plano
  atual'; os outros, 'Assinar' (sem plano pago) ou 'Trocar para este' (com plano pago, e a frase
  'Trocar de plano cancela a assinatura atual; a nova começa sem desconto proporcional.').
- Campo 'CPF ou CNPJ (opcional)', com a dica 'Vai só para o Asaas, que faz a cobrança. Se deixar
  em branco, ele pede na página de pagamento.'
- Botão → `assinar(plano, ciclo, documento)`; com `aguardandoPagamento`, no lugar dos botões:
  'Abrimos o pagamento no navegador. Depois de pagar, volte aqui.' + 'Já assinei, atualizar'
  (`atualizarDepoisDePagar`) + 'Fechar' (`pararDeEsperar`). `aviso` em `role="alert"`, `info`
  em `role="status"`. 'Fechar' do diálogo e Esc → `fecharPlanos()`.

#### Conta (`ContaLogada` → `ContaPlano`)

`ContaLogada` troca a linha `Plano: {nomePlano(plano)}` por `<ContaPlano plano={plano} />`:
- sem cobrança: só `textoPlanoConta(...)` ('Plano: beta gratuito (Full HD)'); nada de preço,
  validade nem botão;
- com cobrança: `textoPlanoConta(...)` ('Plano: Transmissor HD'); se houver assinatura,
  `textoAssinatura(...)`; botão 'Assinar / trocar plano' (`abrirPlanos()`); com
  `aguardandoPagamento`, também 'Já assinei, atualizar'. `info`/`aviso` do `planoStore`.

#### Cadeados nas qualidades

- `CriarQualidade.tsx`: props novas `travasTela?: PresetLock[]` e `travasCamera?: PresetLock[]`
  (ausentes = nada travado). Opção travada: `aria-disabled="true"`, `data-travado="true"`,
  `<SeloPlano plano={needs} cadeado />` embaixo do fps, `title` 'Precisa do plano <seloPlano>',
  clique → `useCliqueTravado().aoClicar(lock)` (nunca `onPreset`). `explicacao` aparece em
  `role="status"` embaixo do grupo.
- `CameraQualidade.tsx`: prop nova `travas?: PresetLock[]`, mesma aparência. Opção que a câmera não
  faz (regra de hoje) continua `disabled` e ganha da trava (sem selo).
- `CriarScreen.tsx`: `const travas = useTravas(modo)`. Passa as travas só com `travas.ativo`.
  Ao entrar no modo Online ou quando as travas mudam: preset da tela travado →
  `melhorPermitido(preset, travas.tela)`. Câmera: o seletor mostra como escolhido
  `melhorPermitido(cameraPreset, travas.camera)` (o que o núcleo vai usar), sem gravar; escolher
  uma opção livre grava como hoje.
- `SalaTransmissao.tsx`: igual, com `modo = snap.room.mode`; o preset inicial da tela passa por
  `melhorPermitido`.
- `ConfigVideo.tsx`: não trava (o padrão vale também no Local). Com Online ligado e
  `quality_locks` presente, cada opção acima do plano ganha o texto pequeno `'Online: <seloPlano>'`.

#### Vire Transmissor e convite

- `CriarScreen` Online com `!travas.podeTransmitir`: só a aba 'Nada por enquanto' (escolhida
  sozinha) e o cartão `VireTransmissor` no lugar da grade de fontes (`CriarFontes` ganha
  `bloqueado?: ReactNode`, que substitui abas e grade). Câmera escondida.
- `CriarScreen` Online com `!travas.podeEntrar`: `ConviteEspectador` no topo e o botão de criar
  desligado.
- `SalaControles.tsx`: prop `podeTransmitir?: boolean` (padrão `true`). Falso e sem transmitir: o
  botão de compartilhar diz 'Vire Transmissor' (abre o painel, como hoje); o de câmera fica
  `aria-disabled` com `title` 'Vire Transmissor para ligar a câmera' e não chama `onToggleCamera`.
  A tecla C em `SalaPrincipal.tsx` não faz nada nesse caso.
- `SalaTransmissao.tsx` com `!podeTransmitir`: só o cartão `VireTransmissor`.
- `SalasScreen.tsx`, modo Online, `!travas.podeEntrar`: 'Entrar' (lista, código e `EntrarCodigo`)
  não chama `api.roomJoin`; mostra `ConviteEspectador` no detalhe. A prévia continua.
- `App.tsx`: renderiza `<AvisoPlano />` (embaixo do `UpdateBanner`) e `<PlanosModal />`.

#### Testes da `telas` (vitest)

1. `CriarQualidade` com `travasTela` do `hd`: '1080p60' tem `data-travado` e o texto 'Full HD';
   clique sem cobrança → 'Seu plano não permite essa qualidade' e `onPreset` não é chamado; com
   cobrança → o diálogo 'Planos do Online' abre com o Full HD em destaque.
2. `CriarScreen` Local com snapshot de plano `hd` → nenhuma opção travada.
3. `CriarScreen` Online, `hd`, `default_preset: '1080p60'` → '720p60' escolhido; `shareStart`
   recebe `preset: '720p60'`.
4. Espectador Online: `CriarScreen` mostra 'Vire Transmissor' e só 'Nada por enquanto';
   `SalaControles` mostra 'Vire Transmissor'; `SalaTransmissao` mostra o cartão. Com cobrança há
   'Ver planos'; sem, não.
5. Conta sem cobrança: 'Plano: beta gratuito (Full HD)', e nenhum 'R$' nem 'Assinar'. Com cobrança
   e assinatura ativa: 'Plano: Transmissor HD', 'Ativa · válida até 10/12/2026', 'Assinar / trocar
   plano'.
6. `PlanosModal`: 3 cartões; anual escolhido com '2 meses de graça' e 'R$ 79/ano' no HD; trocar
   para mensal → 'R$ 7,90/mês'; 'Assinar' no HD → `abrirUrl` com o link do mock; aparece 'Já
   assinei, atualizar'; clicar → 'Plano atualizado: Transmissor HD.'.
7. Salas Online, plano `nenhum`: com cobrança, 'Entrar' mostra 'Assine o Espectador (R$ 3,90/mês ou
   R$ 39/ano) para assistir' e `roomJoin` não é chamado; sem cobrança, 'Sua conta não tem um plano
   para entrar em salas Online'.
8. `AvisoPlano`: `plan_notice` seq 1 → a mensagem aparece; 'Fechar' some; seq 2 → volta. 'Ver
   planos' só com cobrança.
9. **Sem cobrança, nenhum preço:** com cobrança desligada, para cada plano (`nenhum`, `espectador`,
   `hd`, `fullhd`), renderizar Salas, Criar sala (Online), Sala (Online), Configurações → Conexão
   e o aviso de plano: o texto 'R$' e os botões 'Assinar', 'Ver planos' não aparecem; o diálogo de
   planos não abre nem chamando `abrirPlanos()`.
10. Online desligado (`App.online-off.test.tsx`): nenhum cadeado, aviso nem convite; o mock da
    conta não é criado.

## 5. Aceite da fase 3 do app

A fase está pronta quando:
- passam os testes listados em 4.1 a 4.5 (todos);
- neste PC (Windows, MSVC): `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, `cargo check --workspace --all-targets`; no `app/`: `npm run check` e
  `npm test`;
- os testes que já existiam passam sem mudar o que conferem (só a troca mecânica de 4.1 e a
  assinatura de `functions.invoke`);
- no navegador (`npm run dev`): `?plano=hd` mostra os cadeados 'Full HD' em Criar sala Online;
  `?plano=espectador` mostra 'Vire Transmissor'; `?cobranca=1&plano=hd` mostra 'Ver planos' e a
  tela de planos com os preços; sem `?cobranca=1` nenhum preço aparece.

Fica para depois, com o projeto Supabase de verdade (§7 do plano grande): ler `config_publica` e
`assinaturas` reais, `refreshSession` trazendo o claim novo e o roteiro do sandbox do Asaas
(`supabase/README.md` §5 do servidor).

## 6. Ordem e paralelismo

1. `proto` e `net` juntas (crates diferentes, sem arquivo em comum).
2. `core` (depois das duas).
3. `bridge`: depende só dos nomes do JSON de §4.3; pode começar junto do `core` contra o mock, mas
   fecha depois dele.
4. `telas` depois da `bridge`.
5. Fechamento: verificação completa e `docs/ESTADO.md` (fase 3 do app pronta, o que falta testar
   com o Supabase). Ninguém faz commit nem push.

Deploy: servidor primeiro (já pronto), depois o app (§15).

## 7. Riscos

| Risco | Mitigação |
|---|---|
| Plano no app desatualizado (renovou com plano menor) e o servidor recusa um `trilha.atualizar` depois que a escada local já mudou | Raro: o app recebe o plano novo na mesma renovação. A recusa vira `plan_notice` e erro; a transmissão segue com a escada local maior só nessa trilha (a ponte nunca pede acima do anunciado). Não desfazemos a escada nesta fase |
| Números do `zap-proto` do app diferentes dos do servidor (`rev` diferente) | O servidor vale: a recusa dele aparece no aviso. As travas usam os limites que o servidor mandou na sessão; só o selo (`needs`) usa a tabela local |
| `ref` novo no `trilha.publicar` num servidor antigo | Campo desconhecido é ignorado (contrato); o `ref` já é previsto em `mensagens.md` |
| Fonte larga e baixa recusada mesmo com preset liberado | `fit_ladder` (teste 3 de 4.3) |
| Câmera ligando numa qualidade menor sem a pessoa pedir | O seletor mostra o preset efetivo (`melhorPermitido`) e o selo do plano que libera o resto |
| Foco da janela no WebView2 não disparar `focus` | Botão 'Já assinei, atualizar'; conferir no app instalado |
| `functions.invoke('assinar')` ainda não publicada em produção | Só é chamada com `cobranca_ativa`, que só liga depois do checklist de §10; sem a função, o aviso padrão aparece |
| Preço aparecer por erro | Falha de leitura = cobrança desligada; `cobrancaAtiva` exige preços completos; teste 9 de 4.5 varre as telas |
| Tipos do supabase-js maiores que `ContaClient` | Uma conversão só (`contaClient.ts`), como na fase 2 |
