//! Tipos e formatos do protocolo 4 do StreamaZap.
//!
//! Fonte da verdade: `docs/api/`. Este crate não faz I/O e não depende de plataforma: só
//! descreve mensagens de controle (JSON), cabeçalhos binários de mídia e anúncios da descoberta.
#![forbid(unsafe_code)]
