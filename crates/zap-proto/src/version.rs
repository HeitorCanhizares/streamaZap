//! Constantes do protocolo 4.

pub const PROTOCOL_VERSION: u32 = 4;
pub const ALPN: &[u8] = b"zap/4";
pub const DEFAULT_MEDIA_PORT: u16 = 47800;
pub const DISCOVERY_PORT: u16 = 47801;
pub const DISCOVERY_MAGIC: &str = "streamazap";
pub const MAX_CONTROL_MESSAGE: usize = 64 * 1024;
pub const MAX_DISCOVERY_DATAGRAM: usize = 1200;
