use crate::{CaptureError, MonitorInfo};
use windows::core::BOOL;
use windows::Win32::Foundation::{LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW};

/// Flag de monitor principal em `MONITORINFO::dwFlags`.
const MONITORINFOF_PRIMARY: u32 = 1;

fn os_err(context: &'static str, e: windows::core::Error) -> CaptureError {
    CaptureError::Os {
        context,
        message: e.to_string(),
    }
}

/// Junta as duas metades do LUID do adaptador em um `i64`.
fn luid_to_i64(high: i32, low: u32) -> i64 {
    ((high as i64) << 32) | low as i64
}

/// Callback do `EnumDisplayMonitors`: acumula os HMONITOR no `Vec` passado em `data`.
unsafe extern "system" fn coleta_monitor(h: HMONITOR, _dc: HDC, _rect: *mut RECT, data: LPARAM) -> BOOL {
    // SAFETY: `data` é o ponteiro para o `Vec<HMONITOR>` de `list_monitors`, vivo durante a enumeração.
    let lista = unsafe { &mut *(data.0 as *mut Vec<HMONITOR>) };
    lista.push(h);
    TRUE
}

/// Mapa (HMONITOR -> LUID) obtido pelas saídas DXGI de cada adaptador.
fn luids_por_monitor() -> Result<Vec<(isize, i64)>, CaptureError> {
    // SAFETY: chamadas COM com ponteiros locais válidos; os objetos são liberados ao sair do escopo.
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1().map_err(|e| os_err("CreateDXGIFactory1", e))?;
        let mut mapa = Vec::new();
        let mut a = 0;
        while let Ok(adapter) = factory.EnumAdapters1(a) {
            a += 1;
            let Ok(desc) = adapter.GetDesc1() else { continue };
            let luid = luid_to_i64(desc.AdapterLuid.HighPart, desc.AdapterLuid.LowPart);
            let mut o = 0;
            while let Ok(output) = adapter.EnumOutputs(o) {
                o += 1;
                if let Ok(od) = output.GetDesc() {
                    mapa.push((od.Monitor.0 as isize, luid));
                }
            }
        }
        Ok(mapa)
    }
}

/// Monitores ligados, já ordenados por `logic::order_monitors`.
pub fn list_monitors() -> Result<Vec<MonitorInfo>, CaptureError> {
    let mut handles: Vec<HMONITOR> = Vec::new();
    // SAFETY: o callback só usa o `Vec` apontado por LPARAM, que vive até o fim da chamada.
    let ok = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(coleta_monitor),
            LPARAM(&mut handles as *mut Vec<HMONITOR> as isize),
        )
    };
    if !ok.as_bool() {
        return Err(CaptureError::Os {
            context: "EnumDisplayMonitors",
            message: windows::core::Error::from_thread().to_string(),
        });
    }
    if handles.is_empty() {
        return Ok(vec![]);
    }

    // Sem LUID (falha no DXGI) o monitor ainda é listado, com 0.
    let luids = luids_por_monitor().unwrap_or_else(|e| {
        tracing::warn!("LUID dos monitores indisponível: {e}");
        Vec::new()
    });

    let mut monitores = Vec::with_capacity(handles.len());
    for h in handles {
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        // SAFETY: `info` tem `cbSize` de MONITORINFOEXW e MONITORINFOEXW começa com MONITORINFO.
        let ok = unsafe { GetMonitorInfoW(h, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO) };
        if !ok.as_bool() {
            return Err(CaptureError::Os {
                context: "GetMonitorInfoW",
                message: windows::core::Error::from_thread().to_string(),
            });
        }
        let r = info.monitorInfo.rcMonitor;
        let nome_len = info
            .szDevice
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(info.szDevice.len());
        let id = h.0 as isize;
        monitores.push(MonitorInfo {
            id,
            index: 0,
            name: String::from_utf16_lossy(&info.szDevice[..nome_len]),
            x: r.left,
            y: r.top,
            width: (r.right - r.left).max(0) as u32,
            height: (r.bottom - r.top).max(0) as u32,
            primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
            adapter_luid: luids.iter().find(|(m, _)| *m == id).map_or(0, |(_, l)| *l),
        });
    }
    crate::order_monitors(&mut monitores);
    Ok(monitores)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn luid_to_i64_combina_high_e_low() {
        assert_eq!(luid_to_i64(0, 5), 5);
        assert_eq!(luid_to_i64(1, 2), (1_i64 << 32) | 2);
        assert_eq!(luid_to_i64(0, u32::MAX), u32::MAX as i64);
        // High negativo: o bit de sinal sobe; o low não pode "vazar" sinal.
        assert_eq!(luid_to_i64(-1, 0), -(1_i64 << 32));
        assert_eq!(luid_to_i64(-1, u32::MAX), -1);
    }

    #[test]
    fn list_monitors_nao_falha_e_indices_sequenciais() {
        let ms = list_monitors().expect("list_monitors falhou");
        if ms.is_empty() {
            println!("nenhum monitor detectado; teste ignorado");
            return;
        }
        let indices: Vec<u32> = ms.iter().map(|m| m.index).collect();
        let esperado: Vec<u32> = (1..=ms.len() as u32).collect();
        assert_eq!(indices, esperado);
        assert_eq!(ms.iter().filter(|m| m.primary).count(), 1);
    }
}
