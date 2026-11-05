//! Algoritmos de qualidade e latência do StreamaZap.
//!
//! Tudo aqui é determinístico: o tempo entra como parâmetro (`Us`, microssegundos de um relógio
//! monotônico qualquer). Assim cada algoritmo é testado com o simulador de enlace (`sim`) em
//! tempo virtual. Especificação: `docs/arquitetura-v3.md` (seção 6) e `docs/api/balanceamento.md`.
#![forbid(unsafe_code)]

pub mod allocator;
pub mod bwe;
pub mod jitter;
pub mod ladder;
pub mod probe;
pub mod sim;
pub mod trendline;
pub mod unwrap;
pub mod window;

/// Instante ou duração em microssegundos.
pub type Us = u64;
