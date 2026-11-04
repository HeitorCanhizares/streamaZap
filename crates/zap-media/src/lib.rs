//! Mídia do StreamaZap: captura de tela/janela (Windows.Graphics.Capture), conversão para NV12 na
//! GPU, codificação H.264 por Media Foundation, câmera, áudio por aplicativo (WASAPI) e Opus.
//!
//! O código do Windows fica atrás de `cfg(windows)`. Em outros sistemas existe só o backend
//! sintético, usado nos testes de ponta a ponta. Planejado no marco M2.
