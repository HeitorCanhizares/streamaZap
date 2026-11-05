//! Tempo de GPU por consultas D3D11 (`TIMESTAMP_DISJOINT` + 2 × `TIMESTAMP`), sem bloquear.
use crate::{GpuDevice, GpuError};
use windows::core::{Interface, BOOL};
use windows::Win32::Foundation::S_OK;
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Query, D3D11_ASYNC_GETDATA_DONOTFLUSH, D3D11_QUERY, D3D11_QUERY_DATA_TIMESTAMP_DISJOINT, D3D11_QUERY_DESC,
    D3D11_QUERY_TIMESTAMP, D3D11_QUERY_TIMESTAMP_DISJOINT,
};

pub(crate) fn criar_consulta(dev: &GpuDevice, tipo: D3D11_QUERY) -> Result<ID3D11Query, GpuError> {
    let desc = D3D11_QUERY_DESC {
        Query: tipo,
        MiscFlags: 0,
    };
    let mut q: Option<ID3D11Query> = None;
    // SAFETY: `desc` e `q` são válidos durante a chamada.
    unsafe { dev.d3d_device().CreateQuery(&desc, Some(&mut q)) }.map_err(|e| GpuError::Os {
        context: "CreateQuery",
        message: e.message(),
    })?;
    q.ok_or(GpuError::Os {
        context: "CreateQuery",
        message: "consulta nula".into(),
    })
}

/// Lê o resultado de uma consulta sem esperar; `true` só se o retorno for exatamente `S_OK`.
/// Chama o vtable direto porque o wrapper `Result<()>` esconde `S_FALSE` (não pronto).
pub(crate) fn ler<T>(dev: &GpuDevice, q: &ID3D11Query, out: &mut T) -> bool {
    let ctx = dev.context();
    // SAFETY: `out` aponta para um `T` com o tamanho que o driver espera para esta consulta;
    // `q` e `ctx` são objetos vivos do mesmo dispositivo.
    let hr = unsafe {
        (Interface::vtable(ctx).GetData)(
            Interface::as_raw(ctx),
            Interface::as_raw(q),
            out as *mut T as *mut core::ffi::c_void,
            std::mem::size_of::<T>() as u32,
            D3D11_ASYNC_GETDATA_DONOTFLUSH.0 as u32,
        )
    };
    hr == S_OK
}

/// Quantidade de conjuntos de consultas do anel.
pub const RING: usize = 8;

/// Um conjunto de consultas do anel.
pub(crate) struct Slot {
    pub(crate) disjoint: ID3D11Query,
    pub(crate) start: ID3D11Query,
    pub(crate) end: ID3D11Query,
    /// `begin` foi chamado e o resultado ainda não foi coletado.
    pub(crate) in_use: bool,
    /// `end` já foi chamado (o resultado pode estar pronto).
    pub(crate) ended: bool,
}

/// Tempo de GPU de um trecho de comandos no contexto imediato. Anel de 8 conjuntos; os
/// resultados são lidos com `GetData(.., D3D11_ASYNC_GETDATA_DONOTFLUSH)` alguns quadros depois.
pub struct GpuTimer {
    pub(crate) slots: Vec<Slot>,
    pub(crate) next: usize,
}

impl GpuTimer {
    /// Cria as 8 × 3 consultas. Erro de driver -> `GpuError::Os`.
    pub fn new(dev: &GpuDevice) -> Result<Self, GpuError> {
        let mut slots = Vec::with_capacity(RING);
        for _ in 0..RING {
            slots.push(Slot {
                disjoint: criar_consulta(dev, D3D11_QUERY_TIMESTAMP_DISJOINT)?,
                start: criar_consulta(dev, D3D11_QUERY_TIMESTAMP)?,
                end: criar_consulta(dev, D3D11_QUERY_TIMESTAMP)?,
                in_use: false,
                ended: false,
            });
        }
        Ok(Self { slots, next: 0 })
    }
    /// Abre uma medição (`Begin` da disjunta + `End` do início). `None` se o anel está cheio.
    pub fn begin(&mut self, dev: &GpuDevice) -> Option<usize> {
        let idx = (0..RING)
            .map(|k| (self.next + k) % RING)
            .find(|&i| !self.slots[i].in_use)?;
        let ctx = dev.context();
        let s = &mut self.slots[idx];
        // SAFETY: consultas vivas, criadas neste mesmo dispositivo.
        unsafe {
            ctx.Begin(&s.disjoint);
            ctx.End(&s.start);
        }
        s.in_use = true;
        s.ended = false;
        self.next = (idx + 1) % RING;
        Some(idx)
    }
    /// Fecha a medição `slot` (`End` do fim + `End` da disjunta).
    pub fn end(&mut self, dev: &GpuDevice, slot: usize) {
        if slot >= RING {
            return;
        }
        let s = &mut self.slots[slot];
        if !s.in_use || s.ended {
            return;
        }
        let ctx = dev.context();
        // SAFETY: consultas vivas, criadas neste mesmo dispositivo.
        unsafe {
            ctx.End(&s.end);
            ctx.End(&s.disjoint);
        }
        s.ended = true;
    }
    /// Medições prontas em µs (descarta as marcadas `Disjoint`); nunca bloqueia.
    pub fn collect(&mut self, dev: &GpuDevice) -> Vec<f64> {
        let mut out = Vec::new();
        // Do mais antigo (o que `next` aponta, já que o anel é circular) ao mais novo.
        for k in 0..RING {
            let idx = (self.next + k) % RING;
            let s = &mut self.slots[idx];
            if !(s.in_use && s.ended) {
                continue;
            }
            let mut d = D3D11_QUERY_DATA_TIMESTAMP_DISJOINT {
                Frequency: 0,
                Disjoint: BOOL(0),
            };
            if !ler(dev, &s.disjoint, &mut d) {
                continue;
            }
            let (mut t0, mut t1) = (0u64, 0u64);
            if !ler(dev, &s.start, &mut t0) || !ler(dev, &s.end, &mut t1) {
                continue;
            }
            s.in_use = false;
            s.ended = false;
            if d.Disjoint.as_bool() || d.Frequency == 0 || t1 < t0 {
                continue;
            }
            out.push((t1 - t0) as f64 * 1e6 / d.Frequency as f64);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anel_enche_e_devolve_none() {
        let Ok(dev) = GpuDevice::create_default() else {
            eprintln!("pulando");
            return;
        };
        let mut t = GpuTimer::new(&dev).expect("timer");
        for _ in 0..RING {
            assert!(t.begin(&dev).is_some());
        }
        assert!(t.begin(&dev).is_none());
    }

    #[test]
    fn end_de_slot_invalido_nao_entra_em_panico() {
        let Ok(dev) = GpuDevice::create_default() else {
            eprintln!("pulando");
            return;
        };
        let mut t = GpuTimer::new(&dev).expect("timer");
        t.end(&dev, 99);
        t.end(&dev, 0);
    }

    #[test]
    fn collect_sem_nada_pronto_vazio() {
        let Ok(dev) = GpuDevice::create_default() else {
            eprintln!("pulando");
            return;
        };
        let mut t = GpuTimer::new(&dev).expect("timer");
        assert!(t.collect(&dev).is_empty());
    }
}
