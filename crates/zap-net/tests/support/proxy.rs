//! Proxy UDP com atraso, perda e limite de taxa para testar a rede em condições ruins.
//!
//! Sentido cliente->upstream: perda, fila FIFO com limite de taxa (drop-tail) e atraso.
//! Sentido upstream->cliente: só perda e atraso.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;

/// Configuração do proxy.
#[derive(Debug, Clone)]
pub struct ProxyConfig {
    /// Atraso fixo em cada sentido.
    pub delay: Duration,
    /// Probabilidade de perda (0..1) em cada sentido.
    pub loss: f64,
    /// Limite de taxa no sentido cliente->servidor.
    pub rate_kbps: Option<u32>,
    /// Espera máxima na fila drop-tail (ms); costuma ser 50.
    pub queue_ms: u64,
    /// Semente do gerador de perda.
    pub seed: u64,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            delay: Duration::ZERO,
            loss: 0.0,
            rate_kbps: None,
            queue_ms: 50,
            seed: 1,
        }
    }
}

/// Contadores do proxy (somados nos dois sentidos).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProxyStats {
    pub forwarded_packets: u64,
    pub forwarded_bytes: u64,
    pub dropped_packets: u64,
    pub dropped_bytes: u64,
}

#[derive(Default)]
struct Counters {
    fwd_pkts: AtomicU64,
    fwd_bytes: AtomicU64,
    drop_pkts: AtomicU64,
    drop_bytes: AtomicU64,
}

impl Counters {
    fn forwarded(&self, n: usize) {
        self.fwd_pkts.fetch_add(1, Ordering::Relaxed);
        self.fwd_bytes.fetch_add(n as u64, Ordering::Relaxed);
    }
    fn dropped(&self, n: usize) {
        self.drop_pkts.fetch_add(1, Ordering::Relaxed);
        self.drop_bytes.fetch_add(n as u64, Ordering::Relaxed);
    }
}

/// xorshift64* próprio, para não depender de `rand` nos testes.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // Estado zero travaria o gerador.
        Self(if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed })
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Uniforme em [0, 1).
    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

struct Shared {
    /// 0 significa sem limite.
    rate_kbps: AtomicU32,
    counters: Counters,
    client: Mutex<Option<SocketAddr>>,
}

/// Proxy em execução; as tasks terminam ao descartá-lo.
pub struct Proxy {
    pub listen_addr: SocketAddr,
    shared: Arc<Shared>,
    tasks: Vec<JoinHandle<()>>,
}

impl Drop for Proxy {
    fn drop(&mut self) {
        for t in &self.tasks {
            t.abort();
        }
    }
}

type Pending = (Instant, Vec<u8>);

/// Libera os pacotes na ordem em que chegaram, cada um no seu instante.
async fn releaser(mut rx: mpsc::UnboundedReceiver<Pending>, send: impl Fn(Vec<u8>) -> SendFut) {
    while let Some((at, data)) = rx.recv().await {
        tokio::time::sleep_until(at).await;
        send(data).await;
    }
}

type SendFut = std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>;

impl Proxy {
    /// Sobe o proxy em 127.0.0.1:0 repassando para `upstream`.
    pub async fn start(upstream: SocketAddr, cfg: ProxyConfig) -> std::io::Result<Proxy> {
        let front = Arc::new(UdpSocket::bind("127.0.0.1:0").await?);
        let back = Arc::new(UdpSocket::bind("127.0.0.1:0").await?);
        let listen_addr = front.local_addr()?;
        let shared = Arc::new(Shared {
            rate_kbps: AtomicU32::new(cfg.rate_kbps.unwrap_or(0)),
            counters: Counters::default(),
            client: Mutex::new(None),
        });

        let (up_tx, up_rx) = mpsc::unbounded_channel::<Pending>();
        let (down_tx, down_rx) = mpsc::unbounded_channel::<Pending>();
        let mut tasks = Vec::new();

        // Libera cliente -> upstream.
        {
            let back = back.clone();
            let shared = shared.clone();
            tasks.push(tokio::spawn(releaser(up_rx, move |d: Vec<u8>| {
                let back = back.clone();
                let shared = shared.clone();
                Box::pin(async move {
                    if back.send_to(&d, upstream).await.is_ok() {
                        shared.counters.forwarded(d.len());
                    }
                })
            })));
        }
        // Libera upstream -> cliente.
        {
            let front = front.clone();
            let shared = shared.clone();
            tasks.push(tokio::spawn(releaser(down_rx, move |d: Vec<u8>| {
                let front = front.clone();
                let shared = shared.clone();
                Box::pin(async move {
                    let dest = *shared.client.lock();
                    if let Some(c) = dest {
                        if front.send_to(&d, c).await.is_ok() {
                            shared.counters.forwarded(d.len());
                        }
                    }
                })
            })));
        }

        // Recepção cliente -> upstream: perda, fila com taxa, atraso.
        {
            let front = front.clone();
            let shared = shared.clone();
            let mut rng = Rng::new(cfg.seed);
            let (delay, loss, queue) = (cfg.delay, cfg.loss, Duration::from_millis(cfg.queue_ms));
            tasks.push(tokio::spawn(async move {
                let mut buf = vec![0u8; 65536];
                // Instante em que a "linha" fica livre para transmitir.
                let mut next_free = Instant::now();
                while let Ok((n, from)) = front.recv_from(&mut buf).await {
                    {
                        // O primeiro pacote fixa o endereço do cliente.
                        let mut c = shared.client.lock();
                        if c.is_none() {
                            *c = Some(from);
                        }
                    }
                    if rng.next_f64() < loss {
                        shared.counters.dropped(n);
                        continue;
                    }
                    let now = Instant::now();
                    let kbps = shared.rate_kbps.load(Ordering::Relaxed);
                    let release = if kbps == 0 {
                        now + delay
                    } else {
                        let start = next_free.max(now);
                        if start - now > queue {
                            shared.counters.dropped(n);
                            continue;
                        }
                        let tx = Duration::from_secs_f64(n as f64 * 8.0 / (kbps as f64 * 1000.0));
                        next_free = start + tx;
                        // Sai no fim da transmissão; instantes crescentes mantêm a ordem.
                        next_free + delay
                    };
                    let _ = up_tx.send((release, buf[..n].to_vec()));
                }
            }));
        }

        // Recepção upstream -> cliente: só perda e atraso.
        {
            let shared = shared.clone();
            let mut rng = Rng::new(cfg.seed ^ 0xA5A5_5A5A_DEAD_BEEF);
            let (delay, loss) = (cfg.delay, cfg.loss);
            tasks.push(tokio::spawn(async move {
                let mut buf = vec![0u8; 65536];
                while let Ok((n, _)) = back.recv_from(&mut buf).await {
                    if rng.next_f64() < loss {
                        shared.counters.dropped(n);
                        continue;
                    }
                    let _ = down_tx.send((Instant::now() + delay, buf[..n].to_vec()));
                }
            }));
        }

        Ok(Proxy {
            listen_addr,
            shared,
            tasks,
        })
    }

    /// Troca o limite de taxa em tempo de execução (`None` remove).
    pub fn set_rate_kbps(&self, kbps: Option<u32>) {
        self.shared.rate_kbps.store(kbps.unwrap_or(0), Ordering::Relaxed);
    }

    /// Pacotes e bytes encaminhados/descartados até agora.
    pub fn stats(&self) -> ProxyStats {
        let c = &self.shared.counters;
        ProxyStats {
            forwarded_packets: c.fwd_pkts.load(Ordering::Relaxed),
            forwarded_bytes: c.fwd_bytes.load(Ordering::Relaxed),
            dropped_packets: c.drop_pkts.load(Ordering::Relaxed),
            dropped_bytes: c.drop_bytes.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Eco UDP simples como upstream.
    async fn eco() -> SocketAddr {
        let s = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = s.local_addr().unwrap();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 65536];
            while let Ok((n, from)) = s.recv_from(&mut buf).await {
                let _ = s.send_to(&buf[..n], from).await;
            }
        });
        addr
    }

    async fn cliente(p: &Proxy) -> UdpSocket {
        let c = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        c.connect(p.listen_addr).await.unwrap();
        c
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn repassa_nos_dois_sentidos() {
        let p = Proxy::start(eco().await, ProxyConfig::default()).await.unwrap();
        let c = cliente(&p).await;
        c.send(b"ola").await.unwrap();
        let mut buf = [0u8; 16];
        let n = tokio::time::timeout(Duration::from_secs(2), c.recv(&mut buf))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(&buf[..n], b"ola");
        assert_eq!(p.stats().forwarded_packets, 2);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn atraso_minimo() {
        let delay = Duration::from_millis(40);
        let cfg = ProxyConfig {
            delay,
            ..ProxyConfig::default()
        };
        let p = Proxy::start(eco().await, cfg).await.unwrap();
        let c = cliente(&p).await;
        let t0 = std::time::Instant::now();
        c.send(b"x").await.unwrap();
        let mut buf = [0u8; 16];
        tokio::time::timeout(Duration::from_secs(2), c.recv(&mut buf))
            .await
            .unwrap()
            .unwrap();
        assert!(
            t0.elapsed() + Duration::from_millis(5) >= 2 * delay,
            "RTT {:?}",
            t0.elapsed()
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn perda_total() {
        let cfg = ProxyConfig {
            loss: 1.0,
            ..ProxyConfig::default()
        };
        let p = Proxy::start(eco().await, cfg).await.unwrap();
        let c = cliente(&p).await;
        for _ in 0..20 {
            c.send(b"x").await.unwrap();
        }
        let mut buf = [0u8; 16];
        let r = tokio::time::timeout(Duration::from_millis(300), c.recv(&mut buf)).await;
        assert!(r.is_err(), "nada deveria chegar");
        assert_eq!(p.stats().dropped_packets, 20);
        assert_eq!(p.stats().forwarded_packets, 0);
    }

    /// Envia 200 pacotes de 1200 B de uma vez e devolve os instantes de chegada no upstream.
    async fn rajada(p: &Proxy, sink: &UdpSocket) -> Vec<std::time::Instant> {
        let c = cliente(p).await;
        // Envia em paralelo à leitura: o buffer do socket de destino não comporta 200 pacotes.
        tokio::spawn(async move {
            let pkt = [7u8; 1200];
            for _ in 0..200 {
                let _ = c.send(&pkt).await;
            }
        });
        let mut buf = vec![0u8; 2048];
        let mut chegadas = Vec::new();
        while let Ok(Ok(_)) = tokio::time::timeout(Duration::from_millis(400), sink.recv(&mut buf)).await {
            chegadas.push(std::time::Instant::now());
        }
        chegadas
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn limita_taxa() {
        let sink = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let cfg = ProxyConfig {
            rate_kbps: Some(1000),
            queue_ms: 50,
            ..ProxyConfig::default()
        };
        let p = Proxy::start(sink.local_addr().unwrap(), cfg).await.unwrap();
        let ch = rajada(&p, &sink).await;
        assert!(ch.len() >= 3 && ch.len() < 100, "chegaram {}", ch.len());
        let media = (ch[ch.len() - 1] - ch[0]).as_secs_f64() * 1000.0 / (ch.len() - 1) as f64;
        assert!((7.0..=13.0).contains(&media), "intervalo médio {media} ms");
        assert!(p.stats().dropped_packets > 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn set_rate_kbps_none_remove_limite() {
        let sink = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let cfg = ProxyConfig {
            rate_kbps: Some(1000),
            queue_ms: 50,
            ..ProxyConfig::default()
        };
        let p = Proxy::start(sink.local_addr().unwrap(), cfg).await.unwrap();
        p.set_rate_kbps(None);
        let ch = rajada(&p, &sink).await;
        // Sem limite quase tudo chega (folga para o buffer do socket).
        assert!(ch.len() > 150, "chegaram {}", ch.len());
        assert_eq!(p.stats().dropped_packets, 0);
    }
}
