//! Modo Online no Core (marco M4, frente `core-online`; plano: docs/planos/m4-online.md).
//!
//! Visão geral (cada módulo diz o que cumpre):
//! - `account`/`account_apply`: conta e plano da sessão (token só em memória);
//! - `status`: `ServerStatus` (barra: "Servidor online · 42 ms"), vai no `Snapshot.server`;
//! - `backoff`: atrasos de reconexão (1, 2, 4, 8, 15 s);
//! - `convert`: listagens/salas do servidor → tipos da interface e do protocolo;
//! - `session`/`session_run`: login ao abrir, ping, WebSocket e lista de salas Online;
//! - `ws_apply`: aplica mensagens do WebSocket ao `RoomModel` (puro);
//! - `room`/`room_tracks`/`room_events`: `OnlineRoom` (criar, entrar, sair, chat, trilhas);
//! - `bridge`: conexão QUIC `zap/4` com a ponte usando o ingresso;
//! - `publish_bridge`: publicar só as camadas de `camadas.pedidas` pela ponte;
//! - `direct`: rota direta (`p2p.*`) com queda para a ponte;
//! - `preview`: prévia na lista (ingresso de prévia, camada 2, sem áudio);
//! - `reconnect`: reconexão do WebSocket e da ponte com backoff;
//! - `entry`: pontos de entrada do `Core` (`create_online`, `join_online`, `server_test`, prévia).
//!
//! Decisões de projeto (valem para todas as tarefas):
//! - Canais (`docs/api/mensagens.md`, "Canal de cada mensagem"): o app manda `assinar`,
//!   `cancelar`, `keyframe.pedir`, `p2p.*` etc. pelo **WebSocket** (a resposta, como `midia.rota`,
//!   volta por ele). Mas `camadas.pedidas`, `keyframe.pedir` (do servidor ao publicador) e
//!   `camada.atual` chegam **só pelo stream de controle QUIC da ponte**, junto com `relatorio` e
//!   `ping`/`pong`; o leitor de controle da ponte (`publish_bridge`, `subscribe::pool`) as entrega
//!   ao mesmo tratamento de `room_events`. Depois do `bem_vindo`, a ponte também manda `sala.estado`
//!   e o histórico do chat por esse stream (o `ola` com ingresso vale como `sala.entrar`).
//!   (O plano original punha tudo no WebSocket; mudou para seguir o servidor de produção.)
//! - Mensagens do WebSocket que não são `salas.lista` são entregues à sala ativa por um canal
//!   limitado (`OnlineSession::set_route`).
//! - Ingresso da rota direta: o espectador manda `Hello.ticket = make_ticket(direct_secret,
//!   participante_id_do_espectador)`, com `direct_secret = "<CODIGO>:<senha-da-sala-ou-vazio>"`;
//!   o publicador confere com o `MediaAuth` de sempre (`OnlineRoom::media_secret`).
//! - Fingerprint da ponte: `CoreOptions::dev_cert_fp` (testes e servidor com certificado
//!   próprio); sem ele vale `webpki-roots`. `CoreOptions::dev_bridge_addr` força o endereço da
//!   ponte (proxy de teste).

pub mod account;
mod account_apply;
#[cfg(test)]
mod account_tests;
pub mod backoff;
pub mod bridge;
pub mod convert;
pub mod direct;
pub mod entry;
pub mod preview;
pub mod publish_bridge;
pub mod reconnect;
pub mod room;
pub mod room_events;
pub mod room_tracks;
pub mod session;
pub mod session_run;
pub mod status;
pub mod ws_apply;

pub use account::{AccountProviderView, AccountView};
pub use entry::ServerTestResult;
pub use room::OnlineRoom;
pub use session::OnlineSession;
pub use status::{ServerState, ServerStatus};
