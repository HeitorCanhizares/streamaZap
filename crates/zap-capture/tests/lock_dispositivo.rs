//! Contenção do lock do dispositivo D3D11 com a captura DXGI rodando (só Windows; pula sem
//! GPU/monitor). Ver docs/planos/m4c-estabilizacao.md 5.6 b.
#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::ffi::c_void;
use std::time::{Duration, Instant};

use windows::Win32::Graphics::Direct3D11::{ID3D11Query, D3D11_QUERY_DESC, D3D11_QUERY_EVENT};
use zap_capture::movimento::JanelaAnimada;
use zap_capture::{capture_monitor, list_monitors};
use zap_gpu::{GpuDevice, PixelFormat, Texture};

/// Captura o monitor 1 num `GpuDevice` com a tela parada e, numa thread ao lado no mesmo
/// dispositivo, mede 200 iterações de `CopyResource` 64x64 + `End(evento)` + `GetData`: p95 <= 5 ms.
///
/// Com a tela parada, `AcquireNextFrame` espera até 100 ms; se isso acontecer com o lock do
/// dispositivo (ID3D11Multithread) preso, qualquer outra thread no mesmo dispositivo trava junto.
/// Mede-se o tempo de parede da iteração porque o zap-gpu ainda não tem `GpuDevice::lock`; quando
/// a frente gpu entregar `dev.lock()`, pode-se medir só a espera pelo lock.
#[test]
fn captura_nao_segura_o_dispositivo() {
    let _vez = common::vez_da_duplicacao();
    let Some(mon) = list_monitors().ok().and_then(|v| v.into_iter().next()) else {
        eprintln!("pulado: sem monitores");
        return;
    };
    let Ok(dev) = GpuDevice::create_for_adapter(mon.adapter_luid) else {
        eprintln!("pulado: sem dispositivo GPU para o adaptador do monitor");
        return;
    };
    let (sink, _rx, _erros) = common::novo_sink();
    let Ok(handle) = capture_monitor(&dev, &mon, sink) else {
        eprintln!("pulado: duplicação de tela indisponível");
        return;
    };
    // Deixa a captura entrar no AcquireNextFrame (tela parada, sem JanelaAnimada).
    std::thread::sleep(Duration::from_millis(300));

    let mut amostras: Vec<u128> = std::thread::scope(|s| {
        s.spawn(|| {
            let src = Texture::new(&dev, 64, 64, PixelFormat::Bgra8, false).unwrap();
            let dst = Texture::new(&dev, 64, 64, PixelFormat::Bgra8, false).unwrap();
            let desc = D3D11_QUERY_DESC {
                Query: D3D11_QUERY_EVENT,
                MiscFlags: 0,
            };
            let mut query: Option<ID3D11Query> = None;
            // SAFETY: `desc` e `query` são válidos durante a chamada.
            unsafe { dev.d3d_device().CreateQuery(&desc, Some(&mut query)).unwrap() };
            let query = query.unwrap();

            let mut v = Vec::with_capacity(200);
            for _ in 0..200 {
                let t0 = Instant::now();
                // SAFETY: texturas do mesmo dispositivo; `out` vive até o fim do laço e tem o
                // tamanho de BOOL informado ao GetData.
                unsafe {
                    dev.context().CopyResource(dst.raw(), src.raw());
                    dev.context().End(&query);
                    // BOOL (4 bytes): TRUE quando o evento terminou; S_FALSE também vira Ok, então olha-se `out`.
                    let mut out: i32 = 0;
                    while dev
                        .context()
                        .GetData(
                            &query,
                            Some(&mut out as *mut _ as *mut c_void),
                            std::mem::size_of::<i32>() as u32,
                            0,
                        )
                        .is_err()
                        || out == 0
                    {
                        std::hint::spin_loop();
                    }
                }
                v.push(t0.elapsed().as_micros());
                // Espalha as amostras para cobrir vários ciclos do timeout de 100 ms.
                std::thread::sleep(Duration::from_millis(5));
            }
            v
        })
        .join()
        .unwrap()
    });
    drop(handle);

    amostras.sort_unstable();
    let idx = (amostras.len() * 95).div_ceil(100) - 1;
    let p95 = amostras[idx];
    eprintln!("p95_us={p95}");
    assert!(
        p95 <= 5000,
        "p95 = {p95} us (> 5000 us): a captura segura o dispositivo"
    );
}

/// Com a `JanelaAnimada` aberta no monitor 1, `capture_monitor` entrega >= 30 quadros com
/// `new_content` em 1 s.
#[test]
fn janela_animada_gera_quadros() {
    let _vez = common::vez_da_duplicacao();
    let Some(mon) = list_monitors().ok().and_then(|v| v.into_iter().next()) else {
        eprintln!("pulado: sem monitores");
        return;
    };
    let Ok(dev) = GpuDevice::create_for_adapter(mon.adapter_luid) else {
        eprintln!("pulado: sem dispositivo GPU para o adaptador do monitor");
        return;
    };
    // A janela abre antes da captura para que cada pintura gere um quadro novo.
    let Ok(janela) = JanelaAnimada::abrir(&mon) else {
        eprintln!("pulado: JanelaAnimada indisponível");
        return;
    };
    let (sink, rx, _erros) = common::novo_sink();
    let Ok(handle) = capture_monitor(&dev, &mon, sink) else {
        eprintln!("pulado: duplicação de tela indisponível");
        return;
    };
    // Aquecimento: a captura entra no laço de quadros e descarta o que já estava na fila.
    std::thread::sleep(Duration::from_millis(500));
    let _ = rx.try_iter().count();
    std::thread::sleep(Duration::from_secs(1));
    let novos = rx.try_iter().filter(|f| f.new_content).count();
    let pinturas = janela.quadros();
    drop(handle);
    drop(janela);
    assert!(
        novos >= 30,
        "quadros com new_content em 1 s: {novos} (esperado >= 30); janela.quadros() = {pinturas}"
    );
}
