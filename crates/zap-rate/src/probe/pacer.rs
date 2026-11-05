//! Ritmo por crédito (tarefa rate-02).

use super::*;

/// Ritmo por crédito de uma rajada: independe do espaçamento real dos tiques.
#[derive(Debug, Clone)]
pub struct ProbePacer {
    inicio: Us,
    fim: Us,
    bytes_por_us: f64,
    tamanho: usize,
    enviados: u64,
}
impl ProbePacer {
    pub fn new(start: Us, burst: &ProbeBurst, packet_bytes: usize) -> Self {
        Self {
            inicio: start,
            fim: start.saturating_add(burst.duration_us),
            // kbps → bytes por microssegundo: ×1000 bits/s, ÷8 bytes, ÷1e6 us.
            bytes_por_us: f64::from(burst.kbps) * 1000.0 / 8.0 / 1_000_000.0,
            tamanho: packet_bytes.max(1),
            enviados: 0,
        }
    }
    /// Datagramas a mandar agora: `⌊kbps × (min(now, fim) − início) / (8 × tamanho)⌋ − já enviados`.
    pub fn due(&mut self, now: Us) -> u32 {
        // Limitar em `fim` garante que o último tique entregue o crédito inteiro, mesmo atrasado.
        let t = now.min(self.fim).saturating_sub(self.inicio);
        let total = (self.bytes_por_us * t as f64 / self.tamanho as f64).floor() as u64;
        let novos = total.saturating_sub(self.enviados);
        self.enviados = self.enviados.max(total);
        // Um tique nunca chega perto de 2^32 datagramas; saturar evita truncamento silencioso.
        u32::try_from(novos).unwrap_or(u32::MAX)
    }
    pub fn finished(&self, now: Us) -> bool {
        now >= self.fim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rajada() -> ProbeBurst {
        ProbeBurst {
            cluster: 1,
            kbps: 9500,
            total_kbps: 12_000,
            duration_us: 200_000,
        }
    }

    /// Soma de `due` com tiques de `passo` até `fim` e um tique final em `fim`.
    fn soma_com_tiques(passo: Us) -> u64 {
        let mut p = ProbePacer::new(0, &rajada(), 1200);
        let mut now: Us = 0;
        let mut soma: u64 = 0;
        loop {
            now = (now + passo).min(200_000);
            soma += u64::from(p.due(now));
            if now >= 200_000 {
                break;
            }
        }
        // Tique bem depois do fim não pode dar mais nada.
        soma += u64::from(p.due(300_000));
        soma
    }

    #[test]
    fn pacer_total_independe_do_tique() {
        let a = soma_com_tiques(1_000);
        let b = soma_com_tiques(15_625);
        assert!(a == 197 || a == 198, "tique de 1 ms: {a}");
        assert!(b == 197 || b == 198, "tique de 15,6 ms: {b}");
        assert_eq!(a, b);

        let mut p = ProbePacer::new(0, &rajada(), 1200);
        assert!(!p.finished(199_999));
        assert!(p.finished(200_000));
        p.due(200_000);
        assert_eq!(p.due(200_000), 0);
        assert_eq!(p.due(400_000), 0);
    }

    #[test]
    fn pacer_antes_do_inicio_zero() {
        let mut p = ProbePacer::new(1_000_000, &rajada(), 1200);
        assert_eq!(p.due(500_000), 0);
        assert_eq!(p.due(1_000_000), 0);
        assert!(!p.finished(1_100_000));
    }

    #[test]
    fn pacer_tique_unico_gigante() {
        let mut p = ProbePacer::new(0, &rajada(), 1200);
        let total = p.due(10_000_000);
        assert!(total == 197 || total == 198, "total: {total}");
        assert_eq!(p.due(10_000_000), 0);
    }
}
