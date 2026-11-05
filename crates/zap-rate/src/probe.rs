//! Sondagem de banda (balanceamento.md §4): planejador, ritmo e medidor.
//! Sem I/O; tempo em `Us` como parâmetro.
use crate::allocator::{FOLGA_BANDA, MARGEM_SUBIDA};
use crate::Us;

mod meter;
mod pacer;
mod planner;
#[cfg(test)]
mod planner_degraus;

pub use meter::ProbeMeter;
pub use pacer::ProbePacer;
pub use planner::ProbePlanner;

pub const DURACAO_RAJADA_US: Us = 200_000;
/// Espera depois que a necessidade aparece (ou muda) antes da primeira rajada.
pub const PRIMEIRA_SONDA_US: Us = 1_000_000;
pub const INTERVALO_MIN_US: Us = 5_000_000;
pub const INTERVALO_MAX_US: Us = 60_000_000;
/// Prazo para o relatório mostrar o resultado depois do fim da rajada.
pub const JANELA_RESULTADO_US: Us = 1_000_000;
pub const FATOR_ALVO: f64 = 1.5;
pub const SONDA_MIN_KBPS: u32 = 100;
pub const TAMANHO_SONDA: usize = 1200;
/// Janela da taxa enviada (`enviado(e)`); o planejador só sonda com ≥ 500 ms de amostras.
pub const JANELA_ENVIADO_US: Us = 1_000_000;
/// Medidor: o grupo fecha 50 ms depois da última sonda; vale com ≥ 5 sondas e ≥ 100 ms.
pub const FIM_GRUPO_US: Us = 50_000;
pub const MIN_SONDAS: u32 = 5;
pub const MIN_DURACAO_US: Us = 100_000;

/// Teto do degrau sobre a última medida que não saturou.
pub const FATOR_DEGRAU: f64 = 1.5;
/// Teto depois de uma medida que saturou.
pub const FATOR_APOS_SATURAR: f64 = 1.25;
/// Medida abaixo desta fração do alvo da rajada = saturou.
pub const FATOR_SATURADO: f64 = 0.85;
/// Espera entre um degrau e o seguinte (a partir do fim da rajada).
pub const DEGRAU_US: Us = 1_000_000;
/// Validade da medida do caminho.
pub const VALIDADE_MEDIDA_US: Us = 60_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeNeed {
    /// `kbps` nominal da camada que se quer alcançar (`c` em balanceamento.md §4).
    pub next_kbps: u32,
    /// O mesmo `reserved_kbps` que o alocador usa para esse espectador.
    pub reserve_kbps: u32,
}
impl ProbeNeed {
    /// `ceil((MARGEM_SUBIDA × next + reserva) / FOLGA_BANDA)`; 2500/0 → 3195; 8000/0 → 10223.
    pub fn required_kbps(&self) -> u32 {
        // Calculado em f64: os valores são pequenos e os dois campos cabem sem estouro.
        let necessario = (MARGEM_SUBIDA * f64::from(self.next_kbps) + f64::from(self.reserve_kbps)) / FOLGA_BANDA;
        necessario.ceil() as u32
    }
    /// `ceil(FATOR_ALVO × (next + reserva))`; 2500/0 → 3750; 8000/0 → 12000.
    pub fn burst_total_kbps(&self) -> u32 {
        (FATOR_ALVO * (f64::from(self.next_kbps) + f64::from(self.reserve_kbps))).ceil() as u32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeBurst {
    pub cluster: u16,
    /// Taxa só das sondas (o vídeo já enviado não entra).
    pub kbps: u32,
    /// Taxa total que deve chegar durante a rajada (`alvo(e)` do contrato): `kbps + enviado`.
    pub total_kbps: u32,
    pub duration_us: Us,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProbeResult {
    pub cluster: u16,
    pub kbps: u32,
    pub probes: u32,
    pub span_us: Us,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_need_numeros() {
        let a = ProbeNeed {
            next_kbps: 2500,
            reserve_kbps: 0,
        };
        assert_eq!(a.required_kbps(), 3195);
        assert_eq!(a.burst_total_kbps(), 3750);

        let b = ProbeNeed {
            next_kbps: 8000,
            reserve_kbps: 0,
        };
        assert_eq!(b.required_kbps(), 10223);
        assert_eq!(b.burst_total_kbps(), 12000);

        let c = ProbeNeed {
            next_kbps: 2500,
            reserve_kbps: 100,
        };
        assert_eq!(c.burst_total_kbps(), 3900);
        assert_eq!(c.required_kbps(), 3306);

        let zero = ProbeNeed {
            next_kbps: 0,
            reserve_kbps: 0,
        };
        assert_eq!(zero.required_kbps(), 0);
        assert_eq!(zero.burst_total_kbps(), 0);
    }
}
