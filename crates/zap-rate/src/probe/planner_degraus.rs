//! Testes do planejador em degraus (tarefa rate-06). Só usa a API pública de `ProbePlanner`.

use super::*;

const NEED_2500: ProbeNeed = ProbeNeed {
    next_kbps: 2500,
    reserve_kbps: 0,
};
const NEED_8000: ProbeNeed = ProbeNeed {
    next_kbps: 8000,
    reserve_kbps: 0,
};

/// Simulação em passos de 100 ms, com envio constante (`taxa_kbps`) e resultados pendentes.
struct Sim {
    p: ProbePlanner,
    t_ms: u64,
    bytes: u64,
    taxa_kbps: u64,
    /// (instante de entrega, resultado)
    pendentes: Vec<(Us, ProbeResult)>,
    /// Total da última rajada emitida (0 = nenhuma ainda), para a função de banda.
    ultimo_total: u32,
}

impl Sim {
    fn new(taxa_kbps: u64) -> Self {
        Self {
            p: ProbePlanner::new(),
            t_ms: 0,
            bytes: 0,
            taxa_kbps,
            pendentes: Vec::new(),
            ultimo_total: 0,
        }
    }

    /// Avança até `ate_ms` (inclusive). `banda(now, total da última rajada)` dá o relatório;
    /// `resp(rajada)` dá o `kbps` medido, entregue em `on_result` 100 ms depois do fim dela.
    fn avanca(
        &mut self,
        ate_ms: u64,
        need: Option<ProbeNeed>,
        banda: &dyn Fn(Us, u32) -> u32,
        resp: &dyn Fn(&ProbeBurst) -> Option<u32>,
    ) -> Vec<(Us, ProbeBurst)> {
        let mut v = Vec::new();
        while self.t_ms <= ate_ms {
            let now = self.t_ms * 1000;
            let (prontos, resto): (Vec<_>, Vec<_>) = self.pendentes.drain(..).partition(|x| x.0 <= now);
            self.pendentes = resto;
            for (_, r) in prontos {
                self.p.on_result(now, &r);
            }
            if let Some(b) = self.p.poll(now, need, banda(now, self.ultimo_total), self.bytes) {
                self.ultimo_total = b.total_kbps;
                if let Some(kbps) = resp(&b) {
                    let r = ProbeResult {
                        cluster: b.cluster,
                        kbps,
                        probes: 10,
                        span_us: 100_000,
                    };
                    self.pendentes.push((now + b.duration_us + 100_000, r));
                }
                v.push((now, b));
            }
            // taxa kbps -> bytes por passo de 100 ms = taxa * 12,5
            self.bytes += self.taxa_kbps * 25 / 2;
            self.t_ms += 100;
        }
        v
    }
}

fn perto(a: Us, ms: u64) -> bool {
    a.abs_diff(ms * 1000) <= 100_000
}

fn perto_kbps(a: u32, b: u32) -> bool {
    a.abs_diff(b) <= 1
}

#[test]
fn degrau_sobe_3750_5625_8437_12000() {
    let mut s = Sim::new(400);
    // Banda: 3562 até o primeiro sucesso; depois 0,95 × total da última rajada.
    let banda = |_: Us, total: u32| {
        if total <= 3750 {
            3562
        } else {
            (0.95 * f64::from(total)) as u32
        }
    };
    let resp = |b: &ProbeBurst| Some(b.total_kbps);

    // Fase 1: necessidade 2500/0, enviado 400 kbps.
    let r1 = s.avanca(2900, Some(NEED_2500), &banda, &resp);
    assert_eq!(r1.len(), 1, "{:?}", r1.iter().map(|x| x.0).collect::<Vec<_>>());
    assert!(perto(r1[0].0, 1000), "t={}", r1[0].0);
    assert_eq!(r1[0].1.total_kbps, 3750);
    // Banda 3562 >= 3195: sucesso, intervalo de 5 s.
    assert_eq!(s.p.interval_us(), INTERVALO_MIN_US);
    assert_eq!(s.p.last_measure(2_900_000), Some((3750, false)));

    // Fase 2: o vídeo sobe para 2500 kbps; a necessidade muda para 8000 em ~4,5 s.
    s.taxa_kbps = 2500;
    let r2a = s.avanca(4400, Some(NEED_2500), &banda, &resp);
    assert!(r2a.is_empty());
    let r2 = s.avanca(10_000, Some(NEED_8000), &banda, &resp);
    let tempos: Vec<_> = r2.iter().map(|x| x.0).collect();
    assert!(r2.len() >= 3, "{tempos:?}");
    let esperado = [(6200u64, 5625u32), (7400, 8438), (8600, 12000)];
    for (i, (ms, total)) in esperado.iter().enumerate() {
        assert!(perto(r2[i].0, *ms), "rajada {i}: {tempos:?}");
        assert!(
            perto_kbps(r2[i].1.total_kbps, *total),
            "rajada {i}: {}",
            r2[i].1.total_kbps
        );
    }
    // Nenhuma rajada antes de fim + 1 s.
    for w in r2.windows(2) {
        assert!(w[1].0 >= w[0].0 + DURACAO_RAJADA_US + DEGRAU_US, "{tempos:?}");
    }
    // A banda 11400 passa de 10223 depois da última: sucesso.
    assert_eq!(s.p.interval_us(), 5_000_000);
}

#[test]
fn saturou_falha_e_teto_1_25() {
    let mut s = Sim::new(400);
    let banda = |_: Us, _: u32| 1000;
    // Primeira medida: 3750 (não saturou contra o alvo 3750).
    let r1 = s.avanca(2900, Some(NEED_2500), &banda, &|b| Some(b.total_kbps));
    assert_eq!(r1[0].1.total_kbps, 3750);
    // Necessidade 8000: o teto é 1,5 × 3750 = 5625; a medida de 4000 < 0,85 × 5625 satura.
    let r2 = s.avanca(8000, Some(NEED_8000), &banda, &|_| Some(4000));
    assert!(!r2.is_empty());
    assert!(perto(r2[0].0, 6200), "t={}", r2[0].0);
    assert_eq!(r2[0].1.total_kbps, 5625);
    // Falha na hora: intervalo 5 s -> 10 s, sem esperar a janela de resultado.
    assert_eq!(s.p.interval_us(), 10_000_000);
    assert_eq!(s.p.last_measure(8_000_000), Some((4000, true)));
    // Próxima rajada depois do intervalo, com teto ceil(1,25 × 4000) = 5000.
    let r3 = s.avanca(20_000, Some(NEED_8000), &banda, &|_| None);
    assert!(!r3.is_empty());
    assert!(
        r3[0].0 >= r2[0].0 + DURACAO_RAJADA_US + 10_000_000 - 100_000,
        "t={}",
        r3[0].0
    );
    assert_eq!(r3[0].1.total_kbps, 5000);
}

#[test]
fn sem_medida_vai_direto_ao_alvo() {
    let mut s = Sim::new(400);
    let r = s.avanca(2000, Some(NEED_2500), &|_, _| 1000, &|_| None);
    assert!(perto(r[0].0, 1000));
    assert_eq!(r[0].1.total_kbps, 3750);
    assert_eq!(s.p.last_measure(2_000_000), None);
}

#[test]
fn medida_expira_em_60s() {
    let mut s = Sim::new(400);
    let banda = |_: Us, _: u32| 1000;
    // Só a primeira rajada recebe medida: 4000 < 0,85 × 12000 (saturou), entregue em 1,3 s.
    let r = s.avanca(75_000, Some(NEED_8000), &banda, &|b| {
        if b.cluster == 1 {
            Some(4000)
        } else {
            None
        }
    });
    assert_eq!(r[0].1.total_kbps, 12000);
    let t_medida = 1_300_000;
    assert_eq!(s.p.last_measure(t_medida + 59_000_000), Some((4000, true)));
    assert_eq!(s.p.last_measure(t_medida + 61_000_000), None);
    // Enquanto a medida vale, o teto é 5000; depois dela, volta ao alvo cheio.
    let (com, sem): (Vec<&(Us, ProbeBurst)>, Vec<_>) = r.iter().skip(1).partition(|x| x.0 <= t_medida + 60_000_000);
    assert!(
        !com.is_empty() && !sem.is_empty(),
        "{:?}",
        r.iter().map(|x| x.0).collect::<Vec<_>>()
    );
    assert!(com.iter().all(|x| x.1.total_kbps == 5000));
    assert!(sem.iter().all(|x| x.1.total_kbps == 12000));
}

#[test]
fn medida_de_cluster_estranho_ignorada() {
    let mut p = ProbePlanner::new();
    let estranho = ProbeResult {
        cluster: 42,
        kbps: 3000,
        probes: 10,
        span_us: 100_000,
    };
    p.on_result(500_000, &estranho);
    assert_eq!(p.last_measure(500_000), None);
    // Também depois de haver rajadas: cluster que não foi emitido continua ignorado.
    let mut s = Sim::new(400);
    let r = s.avanca(1500, Some(NEED_2500), &|_, _| 1000, &|_| None);
    assert_eq!(r[0].1.cluster, 1);
    s.p.on_result(1_500_000, &estranho);
    assert_eq!(s.p.last_measure(1_500_000), None);
}

#[test]
fn need_muda_mantem_medida() {
    let mut s = Sim::new(400);
    let banda = |_: Us, _: u32| 1000;
    s.avanca(5000, Some(NEED_2500), &banda, &|b| Some(b.total_kbps));
    assert_eq!(s.p.interval_us(), 10_000_000);
    assert_eq!(s.p.last_measure(5_000_000), Some((3750, false)));
    // Troca a necessidade: a medida fica, o intervalo volta a 5 s.
    s.avanca(5100, Some(NEED_8000), &banda, &|_| None);
    assert_eq!(s.p.last_measure(5_100_000), Some((3750, false)));
    assert_eq!(s.p.interval_us(), INTERVALO_MIN_US);
}
