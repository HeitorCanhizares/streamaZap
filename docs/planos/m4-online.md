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
2. Espectador com enlace limitado a 3 Mbps (proxy de teste) recebe camada 1 de uma trilha 1080p60,
   o outro (sem limite) recebe camada 0; o publicador recebe `camadas.pedidas` com 0, 1 e 2
   (2 = miniatura de um terceiro).
3. Protocolo incompatível → erro `protocolo_incompativel` com mensagem para atualizar.
4. Rota direta no loopback funciona e `midia.direta_ok` é enviado.

## Estado do fechamento (revisor)

M4 NAO esta fechado. Verificado:
- `cargo fmt --all --check`: ok.
- `app`: `npm run check` ok; `npm test` 78 arquivos / 617 testes ok. `vite build` nao rodado (disco).
- `cargo clippy --workspace --all-targets -- -D warnings`: FALHA em zap-core (23 erros, quase todos
  dead_code por itens online ainda nao ligados + 7 "doc list item without indentation").
- Nao rodados: `cargo test --workspace`, clippy windows-gnu, xvfb do binario (ha `todo!` pendentes e
  disco com ~330 MB livres; `target/` tem 26 GB).
- `todo!(` restantes: zap-test-server (zt-08, 09, 10, 12, 13, 14, 16, 17), zap-core
  (core-online-11, 15, 19: reconnect.rs, entry.rs, subscribe/pool.rs).
- Tarefas nao concluidas: zt-12, zt-14, zt-08/09/10/13/16-19, core-online-04..11, 13..15, 18..27,
  tauri-online-01..03, ui-online-02/03/04/05/07/08/11/12/15.
- Bloqueio de ambiente: disco cheio (0 MB livres em varias tentativas). Liberar `target/` antes de
  retomar. Dependencia observada: zt-05 depende de zt-11 (tracks.rs).
