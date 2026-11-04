//! Algoritmos de qualidade e latência do StreamaZap.
//!
//! Tudo aqui é determinístico: o tempo entra como parâmetro (`Us`, microssegundos de um relógio
//! monotônico qualquer). Assim cada algoritmo é testado com o simulador de enlace (`sim`) em
//! tempo virtual. Especificação: `docs/arquitetura-v3.md` (seção 6) e `docs/api/balanceamento.md`.
#![forbid(unsafe_code)]

/// Instante ou duração em microssegundos.
pub type Us = u64;
