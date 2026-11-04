//! Assinaturas pelo WebSocket (`docs/api/mensagens.md`, seção "Assinaturas").

use zap_proto::ControlMsg;

use crate::state::Hub;

impl Hub {
    /// Trata `assinar`, `cancelar` e `keyframe.pedir` (C→S). Devolve `true` se era dessa família.
    ///
    /// - `assinar {trilha_id, tamanho, camada_max?, audio?}`: a trilha precisa existir na sala do
    ///   espectador (senão `erro` `invalido`); cria/atualiza o `ViewerEntry`
    ///   (`TrackRelay::subscribe`; `audio` padrão `true`); responde `midia.rota {rota: ponte}`;
    ///   com ingresso de prévia (`TicketData::preview` ativo para o participante) força
    ///   `preview = true` (teto 2, sem áudio). Atualiza `ParticipantData::watching` quando o
    ///   tamanho é `principal`/`cheia` e manda `participante.atualizou`.
    /// - `cancelar {trilha_id}`: `unsubscribe`.
    /// - `keyframe.pedir {trilha_id}`: repassa `keyframe.pedir {trilha_id, camada}` ao publicador
    ///   para a camada atual do espectador, no máximo 1 por segundo por camada.
    pub fn handle_sub_message(&self, who: &str, msg: &ControlMsg) -> bool {
        let _ = (who, msg);
        todo!("tarefa zt-12")
    }
}
