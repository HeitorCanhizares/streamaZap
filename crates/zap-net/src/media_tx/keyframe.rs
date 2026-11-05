//! Estado de keyframes em voo de uma conexão (prazo estendido e admissão). Ver `docs/api/midia.md`
//! ("Prazo") e `docs/planos/m4c-estabilizacao.md` §5.2.
#![allow(dead_code)]

use parking_lot::Mutex;
use std::time::Duration;
use tokio::{sync::Notify, time::Instant};

/// Máximo de fins de keyframe lembrados.
pub(crate) const MAX_FINS: usize = 8;
/// Fins mais velhos que isso são esquecidos.
pub(crate) const VALIDADE_FIM: Duration = Duration::from_secs(5);

/// Instantâneo do estado, para `vence_em`.
#[derive(Debug, Clone, Default)]
pub(crate) struct Instantaneo {
    /// Entrada de cada keyframe ainda em voo.
    pub em_voo: Vec<Instant>,
    /// (entrada, fim) dos últimos keyframes terminados.
    pub fins: Vec<(Instant, Instant)>,
}

#[derive(Default)]
struct Dentro {
    proximo_id: u64,
    /// id -> (entrada, bytes)
    em_voo: Vec<(u64, Instant, u64)>,
    fins: Vec<(Instant, Instant)>,
}

/// Keyframes de uma conexão: em voo, terminados e um `Notify` acordado a cada fim.
#[derive(Default)]
pub(crate) struct KeyframeState {
    dentro: Mutex<Dentro>,
    /// Acordado (`notify_waiters`) quando um keyframe termina (ACK completo ou reset).
    pub(crate) fim: Notify,
}

impl KeyframeState {
    /// Registra um keyframe que entrou em `entrada` com `bytes` (cabeçalho + payload). Devolve o id.
    pub(crate) fn begin(&self, entrada: Instant, bytes: u64) -> u64 {
        let mut d = self.dentro.lock();
        let id = d.proximo_id;
        d.proximo_id += 1;
        d.em_voo.push((id, entrada, bytes));
        id
    }
    /// Keyframe `id` terminou em `fim` (entregue ou resetado): sai de "em voo", entra em `fins`
    /// (até `MAX_FINS`, descartando os mais velhos que `VALIDADE_FIM`) e acorda `fim`.
    pub(crate) fn end(&self, id: u64, fim: Instant) {
        {
            let mut d = self.dentro.lock();
            if let Some(pos) = d.em_voo.iter().position(|(i, _, _)| *i == id) {
                let (_, entrada, _) = d.em_voo.remove(pos);
                d.fins.push((entrada, fim));
            }
            // Esquece fins velhos em relação a este fim e limita o tamanho (jogando os mais antigos).
            d.fins
                .retain(|(_, f)| fim.saturating_duration_since(*f) <= VALIDADE_FIM);
            if d.fins.len() > MAX_FINS {
                let sobra = d.fins.len() - MAX_FINS;
                d.fins.drain(..sobra);
            }
        }
        self.fim.notify_waiters();
    }
    /// Soma dos bytes dos keyframes em voo.
    pub(crate) fn bytes_em_voo(&self) -> u64 {
        self.dentro.lock().em_voo.iter().map(|(_, _, b)| *b).sum()
    }
    /// Cópia do estado atual.
    pub(crate) fn snapshot(&self) -> Instantaneo {
        let d = self.dentro.lock();
        Instantaneo {
            em_voo: d.em_voo.iter().map(|(_, e, _)| *e).collect(),
            fins: d.fins.clone(),
        }
    }
}

/// Instante em que o prazo de um quadro comum que entrou em `entrada` vence, dado o que se sabe
/// dos keyframes; `None` = ainda depende de um keyframe em voo que entrou antes dele.
pub(crate) fn vence_em(
    entrada: Instant,
    prazo: Duration,
    em_voo: &[Instant],
    fins: &[(Instant, Instant)],
) -> Option<Instant> {
    // Keyframe que entrou depois do quadro nunca conta (comparação estrita).
    if em_voo.iter().any(|e| *e < entrada) {
        return None;
    }
    let base = fins
        .iter()
        .filter(|(e, _)| *e < entrada)
        .map(|(_, f)| *f)
        .fold(entrada, Instant::max);
    Some(base + prazo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    const PRAZO: Duration = Duration::from_millis(500);

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn vence_em_sem_keyframe() {
        let t = Instant::now();
        assert_eq!(vence_em(t, PRAZO, &[], &[]), Some(t + PRAZO));
    }

    #[test]
    fn vence_em_keyframe_em_voo_antes_da_none() {
        let t = Instant::now();
        assert_eq!(vence_em(t + ms(100), PRAZO, &[t], &[]), None);
    }

    #[test]
    fn vence_em_keyframe_terminado_depois_da_entrada_usa_fim() {
        let t = Instant::now();
        let fins = [(t, t + ms(300))];
        assert_eq!(vence_em(t + ms(100), PRAZO, &[], &fins), Some(t + ms(300) + PRAZO));
    }

    #[test]
    fn vence_em_keyframe_terminado_antes_da_entrada_usa_entrada() {
        let t = Instant::now();
        let fins = [(t, t + ms(50))];
        assert_eq!(vence_em(t + ms(100), PRAZO, &[], &fins), Some(t + ms(100) + PRAZO));
    }

    #[test]
    fn vence_em_keyframe_que_entrou_depois_nao_conta() {
        let t = Instant::now();
        let entrada = t + ms(100);
        // Em voo, entrou depois.
        assert_eq!(vence_em(entrada, PRAZO, &[t + ms(200)], &[]), Some(entrada + PRAZO));
        // Terminado, entrou depois.
        let fins = [(t + ms(200), t + ms(900))];
        assert_eq!(vence_em(entrada, PRAZO, &[], &fins), Some(entrada + PRAZO));
        // Entrou no mesmo instante: também não conta.
        assert_eq!(vence_em(entrada, PRAZO, &[entrada], &[]), Some(entrada + PRAZO));
    }

    #[test]
    fn vence_em_usa_o_maior_fim() {
        let t = Instant::now();
        let fins = [(t, t + ms(300)), (t + ms(10), t + ms(700)), (t + ms(20), t + ms(400))];
        assert_eq!(vence_em(t + ms(100), PRAZO, &[], &fins), Some(t + ms(700) + PRAZO));
    }

    #[test]
    fn begin_end_bytes_em_voo() {
        let k = KeyframeState::default();
        let t = Instant::now();
        let a = k.begin(t, 100);
        let b = k.begin(t, 50);
        assert_ne!(a, b);
        assert_eq!(k.bytes_em_voo(), 150);
        assert_eq!(k.snapshot().em_voo.len(), 2);
        k.end(a, t + ms(10));
        assert_eq!(k.bytes_em_voo(), 50);
        let s = k.snapshot();
        assert_eq!(s.em_voo, vec![t]);
        assert_eq!(s.fins, vec![(t, t + ms(10))]);
        k.end(b, t + ms(20));
        assert_eq!(k.bytes_em_voo(), 0);
    }

    #[test]
    fn end_id_desconhecido_nao_entra_em_panico() {
        let k = KeyframeState::default();
        let t = Instant::now();
        k.end(42, t);
        let id = k.begin(t, 10);
        k.end(id, t);
        k.end(id, t);
        assert_eq!(k.snapshot().fins.len(), 1);
    }

    #[test]
    fn fins_limitados_a_8_e_5s() {
        let k = KeyframeState::default();
        let t = Instant::now();
        for i in 0..12u64 {
            let id = k.begin(t + ms(i), 1);
            k.end(id, t + ms(100 + i));
        }
        let s = k.snapshot();
        assert_eq!(s.fins.len(), MAX_FINS);
        assert_eq!(s.fins[0].0, t + ms(4));
        // Um fim bem depois esquece os com mais de 5 s.
        let id = k.begin(t, 1);
        k.end(id, t + Duration::from_secs(10));
        assert_eq!(k.snapshot().fins, vec![(t, t + Duration::from_secs(10))]);
    }

    #[tokio::test]
    async fn end_acorda_notify() {
        let k = Arc::new(KeyframeState::default());
        let t = Instant::now();
        let id = k.begin(t, 1);
        let k2 = k.clone();
        let (pronto_tx, pronto_rx) = tokio::sync::oneshot::channel::<()>();
        let tarefa = tokio::spawn(async move {
            let n = k2.fim.notified();
            tokio::pin!(n);
            n.as_mut().enable();
            pronto_tx.send(()).ok();
            n.await;
        });
        pronto_rx.await.expect("tarefa não ficou pronta");
        k.end(id, Instant::now());
        tokio::time::timeout(Duration::from_secs(1), tarefa)
            .await
            .expect("não acordou")
            .expect("tarefa falhou");
    }
}
