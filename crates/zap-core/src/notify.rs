//! Entrega de snapshots à interface: no máx. 4/s, e na hora quando pedido.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;

use crate::engine::UiSink;
use crate::state::UiState;

/// Intervalo entre entregas pelo caminho `touch` (4 por segundo).
const TICK: Duration = Duration::from_millis(250);

pub struct Notifier {
    ui: Arc<dyn UiSink>,
    state: Arc<Mutex<UiState>>,
    dirty: Arc<AtomicBool>,
    /// Zerado no `Drop`; a tarefa de fundo encerra ao vê-lo falso.
    alive: Arc<AtomicBool>,
}

impl Notifier {
    /// Cria o notificador e dispara a tarefa `rt.spawn` que, a cada 250 ms, se `dirty`, zera a
    /// marca e chama `ui.state(state.lock().snapshot())`. A tarefa termina quando o `Notifier`
    /// é derrubado (use `Weak` ou um token de parada).
    pub fn spawn(rt: &tokio::runtime::Handle, ui: Arc<dyn UiSink>, state: Arc<Mutex<UiState>>) -> Self {
        let dirty = Arc::new(AtomicBool::new(false));
        let alive = Arc::new(AtomicBool::new(true));
        let notifier = Self {
            ui: Arc::clone(&ui),
            state: Arc::clone(&state),
            dirty: Arc::clone(&dirty),
            alive: Arc::clone(&alive),
        };
        // Atraso (e não rajada) se a tarefa ficar presa: não queremos entregas em sequência.
        let mut tick = tokio::time::interval(TICK);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        rt.spawn(async move {
            loop {
                tick.tick().await;
                if !alive.load(Ordering::Relaxed) {
                    break;
                }
                if dirty.swap(false, Ordering::Relaxed) {
                    // Snapshot sai com o lock solto: `ui.state` não roda sob o lock do estado.
                    let snap = state.lock().snapshot();
                    ui.state(snap);
                }
            }
        });
        notifier
    }

    /// Marca que há mudança; sai no próximo tick de 250 ms.
    pub fn touch(&self) {
        self.dirty.store(true, Ordering::Relaxed);
    }

    /// Entrega agora (mudança de tela) e zera `dirty`.
    pub fn flush(&self) {
        self.dirty.store(false, Ordering::Relaxed);
        let snap = self.state.lock().snapshot();
        self.ui.state(snap);
    }
}

impl Drop for Notifier {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::Me;
    use crate::types::{ChatLine, TileId, VideoPacket};
    use std::sync::atomic::AtomicUsize;

    /// Conta entregas de `state` e guarda as versões recebidas.
    #[derive(Default)]
    struct UiFalso {
        chamadas: AtomicUsize,
        versoes: Mutex<Vec<u64>>,
    }

    impl UiSink for UiFalso {
        fn state(&self, snapshot: Arc<crate::snapshot::Snapshot>) {
            self.chamadas.fetch_add(1, Ordering::SeqCst);
            self.versoes.lock().push(snapshot.version);
        }
        fn chat(&self, _msg: ChatLine) {}
        fn video(&self, _tile: TileId, _packet: VideoPacket) {}
    }

    fn montar() -> (Arc<UiFalso>, Arc<dyn UiSink>, Arc<Mutex<UiState>>) {
        let falso = Arc::new(UiFalso::default());
        let ui: Arc<dyn UiSink> = falso.clone();
        let state = Arc::new(Mutex::new(UiState::new(Me {
            name: "teste".into(),
            color: "#000000".into(),
            install_id: "id-teste".into(),
        })));
        (falso, ui, state)
    }

    fn entregas(falso: &UiFalso) -> usize {
        falso.chamadas.load(Ordering::SeqCst)
    }

    #[tokio::test(start_paused = true)]
    async fn touch_varias_vezes_entrega_uma_por_tick() {
        let (falso, ui, state) = montar();
        let n = Notifier::spawn(&tokio::runtime::Handle::current(), ui, state);
        for _ in 0..10 {
            n.touch();
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(entregas(&falso), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn sem_touch_nao_entrega() {
        let (falso, ui, state) = montar();
        let _n = Notifier::spawn(&tokio::runtime::Handle::current(), ui, state);
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(entregas(&falso), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn flush_entrega_na_hora_e_limpa_dirty() {
        let (falso, ui, state) = montar();
        let n = Notifier::spawn(&tokio::runtime::Handle::current(), ui, state);
        n.touch();
        n.flush();
        assert_eq!(entregas(&falso), 1, "flush entrega na hora");
        // O tick seguinte não repete: `flush` zerou `dirty`.
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert_eq!(entregas(&falso), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn maximo_quatro_por_segundo() {
        let (falso, ui, state) = montar();
        let n = Notifier::spawn(&tokio::runtime::Handle::current(), ui, state);
        // 99 toques a cada 10 ms cobrem [0, 990) ms: ticks em 0, 250, 500 e 750 ms.
        for _ in 0..99 {
            n.touch();
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let total = entregas(&falso);
        assert!(total >= 1, "ao menos uma entrega, veio {total}");
        assert!(total <= 4, "no máximo 4 por segundo, veio {total}");
    }

    #[tokio::test(start_paused = true)]
    async fn drop_encerra_tarefa() {
        let (falso, ui, state) = montar();
        // `vigia` não conta referência: depois do `drop`, a tarefa precisa soltar o `UiSink`.
        let vigia = Arc::downgrade(&ui);
        let n = Notifier::spawn(&tokio::runtime::Handle::current(), ui, state);
        drop(n);
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(entregas(&falso), 0);
        drop(falso);
        assert!(vigia.upgrade().is_none(), "a tarefa deve ter soltado o UiSink");
    }
}
