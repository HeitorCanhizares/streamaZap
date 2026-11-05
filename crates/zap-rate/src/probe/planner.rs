//! Planejador de sondagem (tarefa rate-03).

use super::*;
use std::collections::VecDeque;

/// Decide quando e quanto sondar um espectador. Um por conexão.
#[derive(Debug, Clone)]
pub struct ProbePlanner {
    need: Option<ProbeNeed>,
    elegivel_em: Us,
    intervalo: Us,
    ultimo_fim: Us,
    em_rajada_ate: Us,
    avaliando: Option<(Us, u32)>,
    cluster: u16,
    enviados: VecDeque<(Us, u64)>,
}
impl Default for ProbePlanner {
    fn default() -> Self {
        Self::new()
    }
}
impl ProbePlanner {
    pub fn new() -> Self {
        Self {
            need: None,
            elegivel_em: 0,
            intervalo: INTERVALO_MIN_US,
            ultimo_fim: 0,
            em_rajada_ate: 0,
            avaliando: None,
            cluster: 0,
            enviados: VecDeque::new(),
        }
    }
    /// Chamar a cada ~100 ms. `need`: `None` = nada a testar. `bandwidth_kbps`: último
    /// `relatorio.banda_kbps` (0 = ainda sem relatório). `sent_bytes_total`: contador acumulado de
    /// bytes enviados a esse espectador (o planejador mede a taxa do último 1 s).
    pub fn poll(
        &mut self,
        now: Us,
        need: Option<ProbeNeed>,
        bandwidth_kbps: u32,
        sent_bytes_total: u64,
    ) -> Option<ProbeBurst> {
        // 1. janela de envio
        self.enviados.push_back((now, sent_bytes_total));
        while self.enviados.len() >= 2 && self.enviados[1].0 <= now.saturating_sub(JANELA_ENVIADO_US) {
            self.enviados.pop_front();
        }
        let (t0, b0) = self.enviados.front().copied().unwrap_or((now, sent_bytes_total));
        let janela_us = now.saturating_sub(t0);
        let enviado_kbps = if janela_us == 0 {
            0.0
        } else {
            sent_bytes_total.saturating_sub(b0) as f64 * 8000.0 / janela_us as f64
        };

        // 2. necessidade mudou: recomeça
        if need != self.need {
            self.need = need;
            self.elegivel_em = now + PRIMEIRA_SONDA_US;
            self.intervalo = INTERVALO_MIN_US;
            self.avaliando = None;
        }

        // 3. rajada em andamento / recém-terminada
        if now < self.em_rajada_ate {
            return None;
        }
        if self.em_rajada_ate != 0 && self.ultimo_fim != self.em_rajada_ate && self.avaliando.is_none() {
            self.ultimo_fim = self.em_rajada_ate;
            self.avaliando = need.map(|n| (self.em_rajada_ate, n.required_kbps()));
        }

        // 4. resultado da última rajada
        if let Some((fim, req)) = self.avaliando {
            if bandwidth_kbps >= req {
                self.intervalo = INTERVALO_MIN_US;
                self.avaliando = None;
            } else if now >= fim + JANELA_RESULTADO_US {
                self.intervalo = (self.intervalo * 2).min(INTERVALO_MAX_US);
                self.avaliando = None;
            } else {
                return None;
            }
        }

        // 5. decide se sonda
        let need = need?;
        if bandwidth_kbps == 0
            || now < self.elegivel_em
            // ultimo_fim == 0: ainda não houve rajada, só vale a elegibilidade
            || (self.ultimo_fim != 0 && now < self.ultimo_fim + self.intervalo)
            || janela_us < 500_000
        {
            return None;
        }
        let sonda = need.burst_total_kbps().saturating_sub(enviado_kbps.round() as u32);
        if sonda < SONDA_MIN_KBPS {
            // Vídeo já ocupa quase tudo: tenta de novo depois do intervalo.
            self.ultimo_fim = now;
            return None;
        }

        // 6. sondando
        self.cluster = self.cluster.wrapping_add(1);
        self.em_rajada_ate = now + DURACAO_RAJADA_US;
        Some(ProbeBurst {
            cluster: self.cluster,
            kbps: sonda,
            duration_us: DURACAO_RAJADA_US,
        })
    }
    pub fn interval_us(&self) -> Us {
        self.intervalo
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NEED: ProbeNeed = ProbeNeed {
        next_kbps: 2500,
        reserve_kbps: 0,
    };

    /// Passo de 100 ms com envio constante de 400 kbps; `banda` decide a banda por instante.
    fn simula(
        p: &mut ProbePlanner,
        de_ms: u64,
        ate_ms: u64,
        need: Option<ProbeNeed>,
        banda: &dyn Fn(Us) -> u32,
    ) -> Vec<(Us, ProbeBurst)> {
        let mut v = Vec::new();
        let mut t = de_ms;
        while t <= ate_ms {
            let now = t * 1000;
            let bytes = t * 50; // 400 kbps = 50 B/ms
            if let Some(b) = p.poll(now, need, banda(now), bytes) {
                v.push((now, b));
            }
            t += 100;
        }
        v
    }

    fn perto(a: Us, ms: u64) -> bool {
        a.abs_diff(ms * 1000) <= 100_000
    }

    #[test]
    fn planner_primeira_rajada_1s() {
        let mut p = ProbePlanner::new();
        let r = simula(&mut p, 0, 3000, Some(NEED), &|_| 1000);
        let (t, b) = r[0];
        assert!(perto(t, 1000), "t={t}");
        assert!(b.kbps.abs_diff(3350) <= 60, "kbps={}", b.kbps);
        assert_eq!(b.cluster, 1);
        assert_eq!(b.duration_us, DURACAO_RAJADA_US);
    }

    #[test]
    fn planner_intervalo_dobra_na_falha() {
        let mut p = ProbePlanner::new();
        let r = simula(&mut p, 0, 400_000, Some(NEED), &|_| 1000);
        let esperado = [1000u64, 11_200, 31_400, 71_600, 131_800, 192_000];
        assert!(
            r.len() >= esperado.len(),
            "{:?}",
            r.iter().map(|x| x.0).collect::<Vec<_>>()
        );
        for (i, e) in esperado.iter().enumerate() {
            assert!(perto(r[i].0, *e), "rajada {i}: {} vs {e}", r[i].0);
        }
    }

    #[test]
    fn planner_sucesso_mantem_5s() {
        let mut p = ProbePlanner::new();
        let r = simula(&mut p, 0, 8000, Some(NEED), &|now| {
            if now > 1_000_000 {
                4000
            } else {
                1000
            }
        });
        assert!(r.len() >= 2);
        assert!(perto(r[0].0, 1000));
        assert!(perto(r[1].0, 6200), "t={}", r[1].0);
        assert_eq!(p.interval_us(), INTERVALO_MIN_US);
    }

    #[test]
    fn planner_sucesso_volta_a_5s() {
        let mut p = ProbePlanner::new();
        // duas falhas: intervalo 5 -> 10 -> 20 s
        simula(&mut p, 0, 15_000, Some(NEED), &|_| 1000);
        assert_eq!(p.interval_us(), 20_000_000);
        // próxima rajada em 31,4 s; banda sobe logo depois dela
        let r = simula(&mut p, 15_100, 34_000, Some(NEED), &|now| {
            if now > 31_500_000 {
                4000
            } else {
                1000
            }
        });
        assert!(perto(r[0].0, 31_400));
        assert_eq!(p.interval_us(), INTERVALO_MIN_US);
    }

    #[test]
    fn planner_need_muda_reinicia() {
        let mut p = ProbePlanner::new();
        simula(&mut p, 0, 5000, Some(NEED), &|_| 1000);
        assert_eq!(p.interval_us(), 10_000_000);
        let outra = ProbeNeed {
            next_kbps: 5000,
            reserve_kbps: 0,
        };
        assert!(simula(&mut p, 5100, 6000, Some(outra), &|_| 1000).is_empty());
        assert_eq!(p.interval_us(), INTERVALO_MIN_US);
        // elegível em 6,1 s, mas o intervalo de 5 s conta do fim da rajada anterior (1,2 s)
        let r = simula(&mut p, 6100, 9000, Some(outra), &|_| 1000);
        assert!(perto(r[0].0, 6200), "t={}", r[0].0);
    }

    #[test]
    fn planner_sem_banda_ou_sem_need_nao_sonda() {
        let mut p = ProbePlanner::new();
        assert!(simula(&mut p, 0, 30_000, Some(NEED), &|_| 0).is_empty());
        let mut p = ProbePlanner::new();
        assert!(simula(&mut p, 0, 30_000, None, &|_| 1000).is_empty());
    }

    #[test]
    fn planner_sonda_pequena_nao_sai() {
        let mut p = ProbePlanner::new();
        // 3700 kbps enviados = 462,5 B/ms; falta menos que SONDA_MIN_KBPS
        let mut t = 0u64;
        let mut rajadas = Vec::new();
        while t <= 5900 {
            if let Some(b) = p.poll(t * 1000, Some(NEED), 1000, t * 4625 / 10) {
                rajadas.push((t, b));
            }
            t += 100;
        }
        assert!(rajadas.is_empty());
        // a tentativa de 1,0 s moveu ultimo_fim; a próxima só vem após o intervalo (6,0 s)
        assert_eq!(p.ultimo_fim, 1_000_000);
    }

    #[test]
    fn planner_cluster_incrementa() {
        let mut p = ProbePlanner::new();
        let r = simula(&mut p, 0, 30_000, Some(NEED), &|_| 1000);
        assert!(r.len() >= 2);
        assert_eq!(r[0].1.cluster, 1);
        assert_eq!(r[1].1.cluster, 2);
    }
}
