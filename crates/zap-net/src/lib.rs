//! Rede do StreamaZap: conexão QUIC `zap/4` (controle, quadros de vídeo com prazo, datagramas
//! de áudio e sonda), relatórios do receptor e descoberta de salas na LAN/Radmin.
//!
//! Especificação: `docs/api/midia.md` e `docs/api/mensagens.md`.
#![forbid(unsafe_code)]
