# M4: modo Online (cliente da API do servidor)

Plano do arquiteto. Contrato: `docs/api/`. O servidor de produção é feito à parte (pelo dono do
projeto). Este marco faz o **app** falar com ele e um **servidor de teste** mínimo para os testes
automáticos (e de referência para quem for escrever o servidor de verdade).

## Frentes

| Id | Crate | Depende de | Resumo |
|---|---|---|---|
| `api` | `crates/zap-net` (módulo `online/`) | — | Cliente REST (`reqwest` + rustls) e WebSocket (`tokio-tungstenite`), tipos de `rest.md` |
| `servidor-teste` | `crates/zap-test-server` (novo, `publish = false`) | `api` | Servidor mínimo: REST + WS + ponte QUIC com o alocador de `zap-rate` |
| `core-online` | `crates/zap-core` | `api` | Sessão Online: salas do servidor, entrar por código, ponte/direta, prévia |
| `ui-online` | `app/src` | `core-online` | Aba Online na lista de salas, criar sala Online, "Entrar com código", config. do servidor e "Testar conexão" |

## Estado de partida (depois do M5) e travas

- A interface já tem a aba Online, o campo de servidor/senha e "Testar conexão" em Configurações,
  "Entrar com código" e o status do servidor na barra (hoje "não configurado").
- O `zap-core` já tem a sala Local completa; o Online entra como outro tipo de sessão no mesmo
  `Core` (mesmos comandos do Tauri: `room_create` com `mode: "online"`, `room_join` com código).
- Dependências já declaradas na raiz: `reqwest` 0.12 (rustls), `tokio-tungstenite` 0.24,
  `futures-util`, `axum` 0.8 (só o servidor de teste).

| Frente | Trava (`crate`) | Depende de |
|---|---|---|
| `api` | `zap-net` | — |
| `servidor-teste` | `zap-test-server` (crate novo, `publish = false`) | `api` |
| `core-online` | `zap-core` | `api` |
| `tauri-online` | `streamazap` | `core-online` (comandos novos: `server_test`, `room_preview`; status do servidor no snapshot) |
| `ui-online` | `app-ui-*` por área | `tauri-online` |

## `api` (cliente)
```rust
pub struct ServerClient { /* base_url, token, http */ }
impl ServerClient {
    pub async fn info(base: &str) -> Result<ServerInfo, ApiError>;                         // GET /v1/info
    pub async fn login(base: &str, req: LoginRequest) -> Result<Self, ApiError>;           // POST /v1/sessoes (+ renovação automática 1 h antes de expirar)
    pub async fn rooms(&self) -> Result<Vec<RoomListing>, ApiError>;
    pub async fn create_room(&self, req: CreateRoomRequest) -> Result<RoomListing, ApiError>;
    pub async fn room(&self, code: &RoomCode) -> Result<RoomDetails, ApiError>;
    pub async fn join(&self, code: &RoomCode, password: Option<&str>) -> Result<JoinResponse, ApiError>;
    pub async fn leave(&self, code: &RoomCode) -> Result<(), ApiError>;
    pub async fn preview(&self, code: &RoomCode) -> Result<PreviewResponse, ApiError>;
    pub async fn diagnostics(&self) -> Result<ServerDiagnostics, ApiError>;
    pub async fn connect_ws(&self) -> Result<(WsTx, WsRx), ApiError>;                      // mensagens = zap_proto::ControlMsg; ping/pong 15 s
}
pub enum ApiError { Http { status: u16, code: String, message: String }, Network(String), Protocol(String) }
```
Erros da API viram `ApiError::Http` com o `codigo` de `docs/api/README.md#erros`; a interface
mostra a `mensagem`.

## `servidor-teste`
Binário e biblioteca para testes (`zap_test_server::spawn(config) -> Handle { base_url, quic_addr }`):
- REST de `rest.md` em memória (axum), certificado autoassinado (o app aceita `--dev-cert-fp`).
- WebSocket com `salas.lista`, `sala.estado`, eventos, chat.
- Ponte QUIC: recebe camadas dos publicadores, repassa a cada espectador a camada escolhida pelo
  `zap_rate::Allocator` (modo `Relay`) a partir do `banda_kbps` dos relatórios; manda
  `camadas.pedidas`, `camada.atual`; reescreve `envio_us`; troca de camada em keyframe.
- Sem persistência, sem limites de taxa. **Não é para produção.**

## `core-online`
- Login ao abrir (se houver endereço configurado), status na barra (`Servidor online · 42 ms`).
- Lista de salas = Online (WS) + Local (descoberta), com filtro por modo.
- Criar sala Online; entrar por código (`ZAP-XXXX`) ou da lista; sair.
- Mídia: conexão QUIC com a ponte usando o `ingresso`; publicação manda só as camadas de
  `camadas.pedidas`; assinatura usa `midia.rota`.
- Rota direta (se `use_relay` e o servidor permitir): `p2p.meus_enderecos` ao entrar; ao receber
  `p2p.candidatos`, tenta conectar em paralelo por 3 s; sucesso → `midia.direta_ok` e a trilha
  passa a vir direto (publicador roda o alocador em `Budget::Direct` para esse espectador).
- Prévia na lista: ingresso de prévia, assina camada 2 sem áudio da trilha principal.
- Reconexão do WS e do QUIC com backoff; ao voltar, `sala.entrar` + reassinar.

## Testes de aceite
1. Com o servidor de teste: dois apps (tarefas) logam, um cria sala com senha, o outro entra pelo
   código, chat e mídia sintética passam pela ponte.
2. Espectador com enlace limitado a 4 Mbps recebe camada 1 de uma trilha 1080p60, o outro (sem
   limite) recebe camada 0; o publicador recebe `camadas.pedidas` com 0, 1 e 2. (Era 3 Mbps, mas
   pela folga de `balanceamento.md` — disponível = banda × 0,9 ≥ 1,15 × kbps — a camada 1 de
   2500 kbps pede ≥ ~3,2 Mbps; com 3 Mbps o espectador fica na camada 2.)
3. Protocolo incompatível → erro `protocolo_incompativel` com mensagem para atualizar.
4. Rota direta no loopback funciona e `midia.direta_ok` é enviado.

## Estado do fechamento (revisor)

**Atualização (2026-11-04, Windows 10, MSVC): o M4 ainda NÃO está fechado, mas quase.**

Pronto:
- Núcleo `core-online`: sem `todo!` em `crates/zap-core` nem em `app/src-tauri`. Tarefas 04 a 15,
  18, 19, 20, 21, 23, 25 e 26 aprovadas ou corrigidas. A 21 (aceite 1) passa; prévia (25) e
  reconexão (26) passam; o servidor de teste agora tolera 5 s de queda da ponte antes de tirar as
  trilhas do participante (`TOLERANCIA_QUEDA` em `zap-test-server/src/bridge.rs`).
- `tauri-online` 01 a 03 e `ui-online` 02, 03, 04, 05, 07, 08, 11, 12, 15 aprovadas.
- Fechamento: `cargo fmt --all --check` ok; `cargo clippy --workspace --all-targets -- -D warnings`
  ok (corrigidos 5 "doc list item without indentation" em `bridge.rs`, `room_events.rs`,
  `session.rs` e o campo `Feed::track` sem uso em `publish_bridge.rs`); `cargo check --workspace
  --all-targets` ok; `app`: `npm run check` ok, `npx vitest run` 666 testes ok (ajustado
  `DiagScreen.test.tsx`: o mock agora traz `server` online, então o teste fixa `server: undefined`
  para o caso "não configurado").
- `cargo test --workspace`: passam `zap-core` (366 de unidade e os aceites 1, 3, prévia,
  reconexão e o harness), `zap-test-server`, `zap-rate`, `streamazap` (44) e o resto.

Falta:
- **core-online-22 (aceite 2, camadas por enlace)**: `aceite_online_camadas_por_enlace` falha.
  Bug de produto, não de teste: em modo ponte nenhum espectador sai da camada 2 porque o `Bwe`
  (`zap-rate/src/bwe.rs`) em estado Normal só cresce se `vazao >= 0,5 * alvo` (limita a ~2x a
  vazão) e ninguém manda sondas ao espectador. Tarefa nova recomendada: (1) `Bwe::on_probe_cluster`
  adotando 0,95 da taxa medida no grupo de sondas; (2) `zap-net/report.rs` agrupa sondas por
  cluster; (3) `zap-test-server` (relay/ticker/bridge) e publicador em `Budget::Direct/Local`
  mandam sonda de 200 ms a cada 5 s (`1,3*kbps(próxima) - kbps(atual)`); (4) corrigir a taxa da
  sonda em `docs/api/balanceamento.md` §4.
- **core-online-24 (aceite 4, rota direta)**: cenários A (direta funciona) e B (falha e fica na
  ponte) e a primeira metade do C passam. `sem_use_relay_direta_com_candidatos` está `#[ignore]`:
  o modo "só direta" (`use_relay=false`) não existe sem ponte (`subscribe/pool.rs` devolve "ponte
  indisponível"; `online/direct.rs` exige `room.bridge`). Ou vira tarefa de produto (watch
  pendente sem ponte, `on_candidates` reconhecendo trilha pendente, `Router` próprio para a PubConn
  direta, queda volta a pendente), ou corta-se a segunda metade do C da spec (os documentos
  divergem: `m4-online.md` diz "se `use_relay`", `arquitetura-v3.md` 7.2 diz "configurável").
- **core-online-27**: pulada por dependência; é este fechamento, que só termina quando 22 e 24
  forem resolvidas.
- Testes que falham neste PC e **não são do M4** (nenhum arquivo do M4 toca neles; conferir antes
  de culpar o M4): `zap-encode --test windows tamanho_medio_nao_chave` (codificador de software do
  Media Foundation, média de 270 B contra 16666), `zap-net --test impaired
  rede_ruim_3mbps_fonte_6mbps` (banda média 2032 kbps fora de 2200..3600; sensível a carga da
  máquina) e `zap-capture --lib` com `STATUS_ACCESS_VIOLATION` quando roda junto dos demais (passa
  sozinho, 33 testes).
- Não rodados: `vite build`, clippy do alvo `windows-gnu` (substituído por `cargo check` nativo),
  xvfb do binário. O endereço padrão do servidor continua sendo gravado só por `Core::start`.
