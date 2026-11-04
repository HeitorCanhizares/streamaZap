//! Pontos de entrada do `Core` para o modo Online.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::engine::{Core, Shared};
use crate::types::{CreateRoom, TileId};
use crate::CoreError;

/// Resultado de "Testar conexão" (Configurações). Sempre devolvido (falha vira `ok = false`).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ServerTestResult {
    pub ok: bool,
    /// `nome` do servidor.
    pub name: Option<String>,
    pub ping_ms: Option<u32>,
    /// Código do erro (`protocolo_incompativel`, `senha_servidor`, `conexao`) quando `!ok`.
    pub code: Option<String>,
    /// Texto em português para a interface (sucesso ou erro).
    pub message: String,
}

/// `Core::create_room` com `mode == Online`: exige que não haja sala; `OnlineRoom::create`,
/// guarda `ActiveRoom::Online` em `shared.room`, `ensure_media_server` se
/// `media.direct_allowed`, `refresh_room_view`, `notifier.flush()`. Falha: nada fica guardado.
pub(crate) async fn create_online(shared: &Arc<Shared>, req: CreateRoom) -> Result<(), CoreError> {
    let _ = (shared, req);
    todo!("tarefa core-online-15")
}

/// `JoinTarget::Code`: `convert::parse_code` + `OnlineRoom::join`; mesmo acabamento de
/// `create_online`. Já em sala → `AlreadyInRoom`.
pub(crate) async fn join_online(shared: &Arc<Shared>, code: String, password: Option<String>) -> Result<(), CoreError> {
    let _ = (shared, code, password);
    todo!("tarefa core-online-15")
}

impl Core {
    /// "Testar conexão": **sem** mexer na sessão atual. `ServerClient::info(base)` (mede o
    /// tempo), confere o protocolo (mesma regra e mensagem de `OnlineSession::connect`), e se
    /// `requer_senha` ou `password` não é vazia, tenta `login` com `LoginRequest` das
    /// configurações (`senha_servidor` → `ok = false, code = "senha_servidor", message = "Senha
    /// do servidor incorreta"`). Endereço vazio → `ok = false, code = "invalido", message =
    /// "Informe o endereço do servidor"`. Rede → `code = "conexao"`.
    pub async fn server_test(&self, address: String, password: String) -> ServerTestResult {
        let _ = (address, password);
        todo!("tarefa core-online-15")
    }
    /// Abre a prévia da sala (`code` digitado/da lista) no `tile`. Ver `preview::start`.
    pub async fn room_preview(&self, code: String, tile: TileId) -> Result<(), CoreError> {
        let _ = (code, tile);
        todo!("tarefa core-online-15")
    }
    pub fn room_preview_stop(&self, tile: TileId) {
        let _ = tile;
        todo!("tarefa core-online-15")
    }
    /// Só para testes: rota atual (`ponte`/`direta`) da trilha assistida, se em sala Online.
    #[doc(hidden)]
    pub fn online_route(&self, track_id: &str) -> Option<zap_proto::control::Route> {
        let _ = track_id;
        todo!("tarefa core-online-15")
    }
    /// Só para testes: endereço (`ip:porta` + impressão digital) do meu `MediaServer`, para a
    /// rota direta; `None` se ainda não existe.
    #[doc(hidden)]
    pub fn online_media_address(&self) -> Option<zap_proto::MediaAddress> {
        todo!("tarefa core-online-15")
    }
    /// Só para testes: já mandei `midia.direta_ok` para essa trilha?
    #[doc(hidden)]
    pub fn online_direct_ok_sent(&self, track_id: &str) -> bool {
        let _ = track_id;
        todo!("tarefa core-online-15")
    }
    /// Só para testes: fecha a conexão QUIC da ponte como numa queda de rede (a sala deve
    /// reconectar sozinha).
    #[doc(hidden)]
    pub fn online_drop_bridge(&self) {
        todo!("tarefa core-online-15")
    }
    /// Só para testes: `OnlineSession::restart` (derruba o WebSocket e refaz o login).
    #[doc(hidden)]
    pub fn online_restart_session(&self) {
        todo!("tarefa core-online-15")
    }
    /// Só para testes: entrega `msg` à sala Online ativa como se tivesse vindo do WebSocket
    /// (o servidor de teste não manda `p2p.candidatos`).
    #[doc(hidden)]
    pub fn online_inject(&self, msg: zap_proto::ControlMsg) {
        let _ = msg;
        todo!("tarefa core-online-15")
    }
}
