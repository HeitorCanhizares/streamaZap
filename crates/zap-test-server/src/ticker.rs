//! Tarefa periódica (100 ms) que roda o alocador de cada trilha e age sobre o resultado.

use std::sync::Arc;

use crate::state::Hub;

impl Hub {
    /// Um passo: para cada trilha de cada sala, `TrackRelay::tick(clock.now_us())` e então
    /// - `LayerChanged` → `camada.atual {trilha_id, camada, largura, altura, fps, motivo}` ao
    ///   WebSocket do espectador (largura/altura/fps da `LayerSpec` da camada);
    /// - `LayersRequested` → `camadas.pedidas {trilha_id, camadas:[{camada,kbps}]}` ao publicador;
    /// - `KeyframeNeeded(c)` → `keyframe.pedir {trilha_id, camada: c}` ao publicador.
    ///
    /// Também chama `sweep(120_000)` uma vez por segundo.
    pub fn tick_all(&self) {
        todo!("tarefa zt-14")
    }
}

/// Laço: `tokio::time::interval(100 ms)`, chamando `hub.tick_all()` até ser abortado.
pub async fn run(hub: Arc<Hub>) {
    let _ = hub;
    todo!("tarefa zt-14")
}
