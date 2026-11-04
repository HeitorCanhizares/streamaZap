//! `TransportConfig` comum (docs/api/midia.md#parâmetros-de-transporte).

use std::sync::Arc;
use std::time::Duration;

/// Troca em uma linha: `true` usa BBR, `false` usa CUBIC. CUBIC é o padrão: o BBR do quinn 0.11
/// ainda é experimental e, no teste de rede ruim, enche a fila do gargalo; quem limita a taxa de
/// verdade é o estimador do app (`zap-rate`), então o controle do QUIC raramente manda.
pub const USE_BBR: bool = false;

const MIB: u32 = 1024 * 1024;

pub fn transport_config() -> quinn::TransportConfig {
    let mut t = quinn::TransportConfig::default();

    // Se o par some, a conexão cai em 10 s; o keep-alive de 1 s evita falso positivo.
    if let Ok(idle) = quinn::IdleTimeout::try_from(Duration::from_secs(10)) {
        t.max_idle_timeout(Some(idle));
    }
    t.keep_alive_interval(Some(Duration::from_secs(1)));

    t.max_concurrent_uni_streams(quinn::VarInt::from_u32(1024));
    t.max_concurrent_bidi_streams(quinn::VarInt::from_u32(8));

    // Datagramas ligados (anunciam max_datagram_frame_size >= 1200).
    t.datagram_receive_buffer_size(Some(2 * MIB as usize));
    t.datagram_send_buffer_size(2 * MIB as usize);

    t.initial_mtu(1200);
    t.mtu_discovery_config(Some(Default::default()));
    t.initial_rtt(Duration::from_millis(100));

    // Quadros chegam a 16 MiB: janelas grandes para não travar o fluxo.
    t.stream_receive_window(quinn::VarInt::from_u32(16 * MIB));
    t.receive_window(quinn::VarInt::from_u32(64 * MIB));
    t.send_window(64 * MIB as u64);
    // Quadros na ordem em que foram escritos (FIFO), sem rodízio entre streams: com rodízio todos
    // os quadros da fila dividem a banda e estouram o prazo juntos; em ordem, o mais velho termina
    // primeiro e só os do fim da fila são descartados.
    t.send_fairness(false);

    if USE_BBR {
        t.congestion_controller_factory(Arc::new(quinn::congestion::BbrConfig::default()));
    } else {
        t.congestion_controller_factory(Arc::new(quinn::congestion::CubicConfig::default()));
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constroi_sem_panico() {
        let _ = transport_config();
    }

    #[test]
    fn usa_cubic_por_padrao() {
        let usa_bbr = std::hint::black_box(USE_BBR);
        assert!(!usa_bbr);
    }

    #[test]
    fn mtu_inicial_1200() {
        assert!(format!("{:?}", transport_config()).contains("initial_mtu: 1200"));
    }
}
