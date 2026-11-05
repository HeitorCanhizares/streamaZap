//! Relógios de reprodução por participante e a thread que entrega quadros no instante certo.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use zap_rate::jitter::{PlayoutClock, PlayoutConfig};

use crate::engine::UiSink;
use crate::types::{TileId, VideoPacket};

/// Antecedência de entrega (tempo para decodificar), em µs.
pub const LEAD_US: u64 = 8_000;

/// Fila ordenada por instante de entrega. Puro (sem relógio nem threads).
pub struct PlayoutQueue {
    items: BTreeMap<(u64, u64), (TileId, VideoPacket)>,
    counter: u64,
}

impl Default for PlayoutQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayoutQueue {
    pub fn new() -> Self {
        Self {
            items: BTreeMap::new(),
            counter: 0,
        }
    }
    /// Insere com instante `due_us` (ordem estável para empates).
    pub fn push(&mut self, due_us: u64, tile: TileId, packet: VideoPacket) {
        // O contador desempata `due_us` iguais mantendo a ordem de inserção.
        self.counter += 1;
        self.items.insert((due_us, self.counter), (tile, packet));
    }
    /// Remove e devolve, em ordem, tudo com `due_us <= now_us`.
    pub fn pop_due(&mut self, now_us: u64) -> Vec<(TileId, VideoPacket)> {
        // Tudo com chave <= (now_us, MAX) vence; o resto fica em `rest`.
        let rest = match now_us.checked_add(1) {
            Some(limite) => self.items.split_off(&(limite, 0)),
            None => BTreeMap::new(),
        };
        let due = std::mem::replace(&mut self.items, rest);
        due.into_values().collect()
    }
    pub fn next_due(&self) -> Option<u64> {
        self.items.keys().next().map(|(due, _)| *due)
    }
    pub fn remove_tile(&mut self, tile: TileId) {
        self.items.retain(|_, (t, _)| *t != tile);
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Um `PlayoutClock` por participante (compartilhado entre vídeo e áudio dele) + a fila + a
/// thread de entrega.
pub struct PlayoutHub {
    pub(crate) inner: Arc<HubInner>,
}

pub(crate) struct HubInner {
    pub clock: zap_net::Clock,
    pub clocks: Mutex<HashMap<String, Arc<Mutex<PlayoutClock>>>>,
    pub queue: Mutex<PlayoutQueue>,
    pub stop: std::sync::atomic::AtomicBool,
    pub smoothing: Mutex<(bool, u32)>,
}

impl PlayoutHub {
    /// Sobe a thread "zap-playout" (`thread_prio::raise_current_thread`, laço de 2 ms): a cada
    /// volta, `pop_due(clock.now_us() + LEAD_US)` e entrega cada item a `ui.video(tile, packet)`
    /// (todos os vencidos, em ordem). Termina quando `stop` é verdadeiro (Drop).
    pub fn start(clock: zap_net::Clock, ui: Arc<dyn UiSink>) -> Self {
        let inner = Arc::new(HubInner {
            clock,
            clocks: Mutex::new(HashMap::new()),
            queue: Mutex::new(PlayoutQueue::new()),
            stop: AtomicBool::new(false),
            smoothing: Mutex::new((true, 200)),
        });
        let th = Arc::clone(&inner);
        let spawned = std::thread::Builder::new()
            .name("zap-playout".into())
            .spawn(move || run_loop(th, ui));
        if let Err(e) = spawned {
            tracing::error!("falha ao subir a thread zap-playout: {e}");
        }
        Self { inner }
    }
    /// Relógio do participante (criado sob demanda com a suavização atual).
    pub fn clock_for(&self, participant_id: &str) -> Arc<Mutex<PlayoutClock>> {
        let (auto, ms) = *self.inner.smoothing.lock();
        let mut clocks = self.inner.clocks.lock();
        Arc::clone(clocks.entry(participant_id.to_string()).or_insert_with(|| {
            Arc::new(Mutex::new(PlayoutClock::new(PlayoutConfig {
                auto,
                manual_delay_ms: ms,
                ..Default::default()
            })))
        }))
    }
    /// Agenda o quadro: `play_time(capture_us)` do relógio do participante; `None` → entrega já
    /// (`due = 0`). `due = play_time` (a antecedência `LEAD_US` é aplicada na thread).
    pub fn push(&self, participant_id: &str, tile: TileId, packet: VideoPacket) {
        let clock = self.clock_for(participant_id);
        let due = clock.lock().play_time(packet.capture_us).unwrap_or(0);
        self.inner.queue.lock().push(due, tile, packet);
    }
    /// Entrega já, sem relógio de participante (prévia local do que eu transmito).
    pub fn push_now(&self, tile: TileId, packet: VideoPacket) {
        self.inner.queue.lock().push(0, tile, packet);
    }
    pub fn remove_tile(&self, tile: TileId) {
        self.inner.queue.lock().remove_tile(tile);
    }
    /// Aplica a todos os relógios (`set_auto`, `set_manual_delay`) e guarda para os novos.
    pub fn set_smoothing(&self, auto: bool, ms: u32) {
        *self.inner.smoothing.lock() = (auto, ms);
        let todos: Vec<_> = self.inner.clocks.lock().values().cloned().collect();
        for c in todos {
            let mut c = c.lock();
            c.set_auto(auto);
            c.set_manual_delay(ms);
        }
    }
}

/// Intervalo entre chamadas de `tick` dos relógios, em µs.
const TICK_US: u64 = 50_000;

/// Laço da thread de entrega. Guarda só o `Arc<HubInner>`, então termina sozinha após o Drop.
fn run_loop(inner: Arc<HubInner>, ui: Arc<dyn UiSink>) {
    crate::thread_prio::raise_current_thread();
    let mut ultimo_tick = inner.clock.now_us();
    while !inner.stop.load(Ordering::Relaxed) {
        let now = inner.clock.now_us();
        // Solta o lock da fila antes de chamar a UI.
        let due = inner.queue.lock().pop_due(now.saturating_add(LEAD_US));
        for (tile, packet) in due {
            ui.video(tile, packet);
        }
        if now.saturating_sub(ultimo_tick) >= TICK_US {
            ultimo_tick = now;
            let todos: Vec<_> = inner.clocks.lock().values().cloned().collect();
            for c in todos {
                c.lock().tick(now);
            }
        }
        // Dorme até o próximo vencimento se for antes de 2 ms.
        let mut espera = Duration::from_millis(2);
        if let Some(next) = inner.queue.lock().next_due() {
            let falta = next.saturating_sub(now.saturating_add(LEAD_US));
            espera = espera.min(Duration::from_micros(falta));
        }
        std::thread::sleep(espera);
    }
}

impl Drop for PlayoutHub {
    fn drop(&mut self) {
        self.inner.stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::types::ChatLine;
    use crate::Snapshot;
    use std::time::Instant;

    fn pkt(seq: u32) -> VideoPacket {
        VideoPacket {
            track: 1,
            layer: 0,
            keyframe: false,
            width: 16,
            height: 16,
            seq,
            capture_us: 0,
            data: bytes::Bytes::new(),
        }
    }

    fn seqs(v: &[(TileId, VideoPacket)]) -> Vec<u32> {
        v.iter().map(|(_, p)| p.seq).collect()
    }

    #[test]
    fn fila_ordena_por_due() {
        let mut q = PlayoutQueue::new();
        q.push(30, 1, pkt(3));
        q.push(10, 1, pkt(1));
        q.push(20, 1, pkt(2));
        assert_eq!(seqs(&q.pop_due(100)), vec![1, 2, 3]);
        assert!(q.is_empty());
    }

    #[test]
    fn fila_empate_mantem_ordem_de_insercao() {
        let mut q = PlayoutQueue::new();
        for s in [5, 2, 9, 1] {
            q.push(10, 1, pkt(s));
        }
        assert_eq!(seqs(&q.pop_due(10)), vec![5, 2, 9, 1]);
    }

    #[test]
    fn pop_due_so_vencidos() {
        let mut q = PlayoutQueue::new();
        q.push(10, 1, pkt(1));
        q.push(20, 1, pkt(2));
        q.push(30, 1, pkt(3));
        assert_eq!(seqs(&q.pop_due(20)), vec![1, 2]);
        assert_eq!(q.len(), 1);
        assert!(q.pop_due(29).is_empty());
        assert_eq!(seqs(&q.pop_due(u64::MAX)), vec![3]);
    }

    #[test]
    fn next_due_menor() {
        let mut q = PlayoutQueue::new();
        assert_eq!(q.next_due(), None);
        q.push(50, 1, pkt(1));
        q.push(7, 1, pkt(2));
        assert_eq!(q.next_due(), Some(7));
    }

    #[test]
    fn remove_tile_remove_so_dele() {
        let mut q = PlayoutQueue::new();
        q.push(1, 1, pkt(1));
        q.push(2, 2, pkt(2));
        q.push(3, 1, pkt(3));
        q.remove_tile(1);
        assert_eq!(q.len(), 1);
        assert_eq!(seqs(&q.pop_due(10)), vec![2]);
    }

    #[derive(Default)]
    struct FakeUi {
        got: Mutex<Vec<(TileId, u32, Instant)>>,
    }
    impl UiSink for FakeUi {
        fn state(&self, _snapshot: Arc<Snapshot>) {}
        fn chat(&self, _msg: ChatLine) {}
        fn video(&self, tile: TileId, packet: VideoPacket) {
            self.got.lock().push((tile, packet.seq, Instant::now()));
        }
    }

    fn hub() -> (PlayoutHub, Arc<FakeUi>) {
        let ui = Arc::new(FakeUi::default());
        let h = PlayoutHub::start(zap_net::Clock::new(), ui.clone());
        (h, ui)
    }

    #[test]
    fn entrega_em_ordem_de_seq() {
        let (h, ui) = hub();
        for s in 0..5 {
            h.push("a", 1, pkt(s));
        }
        std::thread::sleep(Duration::from_millis(200));
        let seqs: Vec<u32> = ui.got.lock().iter().map(|(_, s, _)| *s).collect();
        assert_eq!(seqs, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn entrega_futuro_so_no_horario() {
        let (h, ui) = hub();
        let t0 = Instant::now();
        let due = h.inner.clock.now_us() + 100_000;
        h.inner.queue.lock().push(due, 1, pkt(1));
        std::thread::sleep(Duration::from_millis(50));
        assert!(ui.got.lock().is_empty());
        std::thread::sleep(Duration::from_millis(250));
        let got = ui.got.lock();
        assert_eq!(got.len(), 1);
        let dt = got[0].2.duration_since(t0);
        assert!(dt >= Duration::from_millis(85), "cedo demais: {dt:?}");
        assert!(dt < Duration::from_millis(300), "tarde demais: {dt:?}");
    }

    #[test]
    fn clock_for_devolve_o_mesmo_arc() {
        let (h, _ui) = hub();
        assert!(Arc::ptr_eq(&h.clock_for("a"), &h.clock_for("a")));
        assert!(!Arc::ptr_eq(&h.clock_for("a"), &h.clock_for("b")));
    }

    #[test]
    fn set_smoothing_vale_para_novos_relogios() {
        let (h, _ui) = hub();
        let antigo = h.clock_for("a");
        h.set_smoothing(false, 400);
        assert_eq!(*h.inner.smoothing.lock(), (false, 400));
        let novo = h.clock_for("b");
        assert!((novo.lock().current_delay_ms() - 400.0).abs() < 1e-6);
        assert!((antigo.lock().current_delay_ms() - 400.0).abs() < 1e-6);
    }

    #[test]
    fn drop_para_a_thread() {
        let (h, ui) = hub();
        let inner = Arc::clone(&h.inner);
        drop(h);
        std::thread::sleep(Duration::from_millis(50));
        inner.queue.lock().push(0, 1, pkt(1));
        std::thread::sleep(Duration::from_millis(50));
        assert!(ui.got.lock().is_empty());
    }
}
