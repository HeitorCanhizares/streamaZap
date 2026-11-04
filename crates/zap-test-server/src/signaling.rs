//! Mensagens do WebSocket sobre sala, participantes e chat (`docs/api/mensagens.md`,
//! seções "Salas e participantes" e "Chat").

use tokio::sync::mpsc;
use zap_proto::ControlMsg;

use crate::state::Hub;

impl Hub {
    /// WebSocket novo de `who`: guarda `tx` em `ParticipantData::ws` (substitui o anterior) e
    /// manda `salas.lista` (formato `GET /v1/salas`).
    pub fn ws_connected(&self, who: &str, tx: mpsc::UnboundedSender<ControlMsg>) {
        let _ = (who, tx);
        todo!("tarefa zt-10")
    }

    /// WebSocket de `who` fechou: se `tx` ainda é o registrado (`same_channel`), limpa `ws` e
    /// `ws_room`. Não remove o membro da sala (ele pode reconectar; ver `Hub::drop_participant`).
    pub fn ws_disconnected(&self, who: &str, tx: &mpsc::UnboundedSender<ControlMsg>) {
        let _ = (who, tx);
        todo!("tarefa zt-10")
    }

    /// Despacha uma mensagem C→S do WebSocket:
    /// - `sala.entrar {sala_id}`: exige ser membro (senão `erro` `proibido`); define `ws_room`,
    ///   manda `sala.estado` e as últimas 200 `chat.mensagem`; se for a primeira vez na sala via
    ///   WS não repete `participante.entrou` (já foi mandado pelo REST `entrar`);
    /// - `sala.sair`: limpa `ws_room`;
    /// - `perfil.atualizar {nome?, cor?}`: atualiza e manda `participante.atualizou`;
    /// - `chat.enviar {texto, cliente_id}`: 1–1000 caracteres (senão `erro` `invalido` com
    ///   `ref = cliente_id`); guarda (limite 200) e manda `chat.mensagem` a todos da sala via WS,
    ///   com `cliente_id` só no remetente; atualiza `ultima_mensagem` e dispara `salas.lista`;
    /// - `chat.digitando`: repassa `chat.digitando {participante_id}` aos outros;
    /// - `estatisticas`: guarda `ping_ms` como `latencia_ms` e manda `participante.atualizou`;
    /// - `p2p.*`, `conexao.reversa`, `midia.direta_*`: ignoradas (rota direta fora do escopo);
    /// - trilhas e assinaturas: delegam a `handle_track_message` (`tracks.rs`);
    /// - qualquer outra: `erro` `invalido`.
    pub fn handle_ws_message(&self, who: &str, msg: ControlMsg) {
        let _ = (who, msg);
        todo!("tarefa zt-10")
    }
}
