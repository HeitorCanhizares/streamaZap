//! Constantes do protocolo 4.

pub const PROTOCOL_VERSION: u32 = 4;
pub const ALPN: &[u8] = b"zap/4";
pub const DEFAULT_MEDIA_PORT: u16 = 47800;
pub const DISCOVERY_PORT: u16 = 47801;
pub const DISCOVERY_MAGIC: &str = "streamazap";
pub const MAX_CONTROL_MESSAGE: usize = 64 * 1024;
pub const MAX_DISCOVERY_DATAGRAM: usize = 1200;

/// Recursos além do protocolo 4 base que este app sabe (`recursos` do `ola`).
pub const FEATURES: &[&str] = &["palco"];

/// [`FEATURES`] como `Vec<String>`, pronto para o `ola` e o `POST /v1/sessoes`.
pub fn features() -> Vec<String> {
    FEATURES.iter().map(|f| (*f).to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn features_tem_palco() {
        assert_eq!(features(), vec!["palco".to_string()]);
        assert!(FEATURES.contains(&crate::stage::FEATURE));
    }

    #[test]
    fn protocolo_continua_4() {
        assert_eq!(PROTOCOL_VERSION, 4);
        assert_eq!(ALPN, &b"zap/4"[..]);
    }
}
