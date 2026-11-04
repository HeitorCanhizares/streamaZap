use crate::{cursor_local_pos, CaptureError};
use windows::core::Interface;
use windows::Win32::Graphics::Dxgi::IDXGISurface1;
use windows::Win32::Graphics::Gdi::{DeleteObject, GetObjectW, BITMAP, HGDIOBJ};
use windows::Win32::UI::WindowsAndMessaging::{
    DrawIconEx, GetCursorInfo, GetIconInfo, CURSORINFO, CURSOR_SHOWING, DI_NORMAL, ICONINFO,
};
use zap_gpu::Texture;

fn os_err(context: &'static str, e: windows::core::Error) -> CaptureError {
    CaptureError::Os {
        context,
        message: e.to_string(),
    }
}

/// Lê largura/altura de um bitmap GDI (0x0 se a consulta falhar).
fn bitmap_size(h: HGDIOBJ) -> (i32, i32) {
    let mut bm = BITMAP::default();
    // SAFETY: `bm` é um BITMAP válido e o tamanho passado é o da própria struct.
    let n = unsafe {
        GetObjectW(
            h,
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bm as *mut BITMAP as *mut _),
        )
    };
    if n == 0 {
        (0, 0)
    } else {
        (bm.bmWidth, bm.bmHeight)
    }
}

/// Desenha o cursor atual (`GetCursorInfo`, se visível) por cima de `tex` (BGRA, criada com
/// `gdi_compatible = true`) via `IDXGISurface1::GetDC` + `DrawIconEx`. `monitor` = (x, y, largura,
/// altura) do monitor em pixels de tela. Sem cursor visível ou fora do monitor: não faz nada.
///
/// A textura PRECISA ser `gdi_compatible`; senão `GetDC` falha e retorna `CaptureError::Os`.
pub(crate) fn draw_cursor(tex: &Texture, monitor: (i32, i32, u32, u32)) -> Result<(), CaptureError> {
    let mut ci = CURSORINFO {
        cbSize: std::mem::size_of::<CURSORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `ci` tem cbSize preenchido, como exigido pela API.
    unsafe { GetCursorInfo(&mut ci) }.map_err(|e| os_err("GetCursorInfo", e))?;
    if (ci.flags.0 & CURSOR_SHOWING.0) == 0 {
        return Ok(());
    }

    let mut ii = ICONINFO::default();
    // SAFETY: `ci.hCursor` veio de GetCursorInfo; `ii` é válido para escrita.
    unsafe { GetIconInfo(ci.hCursor.into(), &mut ii) }.map_err(|e| os_err("GetIconInfo", e))?;
    // Cursor monocromático não tem hbmColor: a máscara guarda AND+XOR empilhados (altura dobrada).
    let (w, h) = if ii.hbmColor.is_invalid() {
        let (w, h) = bitmap_size(ii.hbmMask.into());
        (w, h / 2)
    } else {
        bitmap_size(ii.hbmColor.into())
    };
    // SAFETY: os bitmaps foram criados por GetIconInfo e são nossos para liberar.
    unsafe {
        if !ii.hbmMask.is_invalid() {
            let _ = DeleteObject(ii.hbmMask.into());
        }
        if !ii.hbmColor.is_invalid() {
            let _ = DeleteObject(ii.hbmColor.into());
        }
    }

    let icon = (w.max(0) as u32, h.max(0) as u32);
    let hotspot = (ii.xHotspot as i32, ii.yHotspot as i32);
    let Some((x, y)) = cursor_local_pos((ci.ptScreenPos.x, ci.ptScreenPos.y), hotspot, icon, monitor) else {
        return Ok(());
    };

    let surface = tex
        .raw()
        .cast::<IDXGISurface1>()
        .map_err(|e| os_err("IDXGISurface1", e))?;
    // SAFETY: a superfície é gdi_compatible; o DC é liberado abaixo em qualquer caminho.
    let hdc = unsafe { surface.GetDC(false) }.map_err(|e| os_err("IDXGISurface1::GetDC", e))?;
    // SAFETY: `hdc` é válido até o ReleaseDC.
    let desenho = unsafe { DrawIconEx(hdc, x, y, ci.hCursor.into(), 0, 0, 0, None, DI_NORMAL) };
    // Sempre libera o DC, mesmo se o desenho falhou (RECT nulo = superfície inteira).
    // SAFETY: par do GetDC acima.
    let liberado = unsafe { surface.ReleaseDC(None) };
    desenho.map_err(|e| os_err("DrawIconEx", e))?;
    liberado.map_err(|e| os_err("IDXGISurface1::ReleaseDC", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use zap_gpu::{GpuDevice, PixelFormat};

    #[test]
    fn draw_cursor_em_textura_gdi_ok() {
        let dev = match GpuDevice::create_default() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("sem GPU, teste pulado: {e}");
                return;
            }
        };
        let tex = match Texture::new(&dev, 64, 64, PixelFormat::Bgra8, true) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("sem textura GDI, teste pulado: {e}");
                return;
            }
        };
        assert!(draw_cursor(&tex, (0, 0, 64, 64)).is_ok());
    }
}
