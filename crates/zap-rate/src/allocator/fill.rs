//! Passos 1 e 2 do algoritmo: rodízio max-min de subida de camadas (função pura).
#![allow(unused_variables, dead_code)]

use zap_proto::LayerSpec;

#[derive(Debug, Clone, Copy)]
pub struct FillViewer {
    /// `banda × 0,9 − reservado`, em kbps (pode ser negativo).
    pub available_kbps: f64,
    /// Teto (número maior = camada menor).
    pub max_layer: u8,
}

/// Camada alvo de cada espectador (mesma ordem de `viewers`). `layers` está em ordem de camada
/// (0 = melhor). `upload_budget_kbps`: `Some(upload × 0,9)` no modo direto.
pub fn fill(layers: &[LayerSpec], viewers: &[FillViewer], upload_budget_kbps: Option<f64>) -> Vec<u8> {
    if viewers.is_empty() {
        return Vec::new();
    }
    let Some(last) = layers.len().checked_sub(1) else {
        // Sem escada não há o que subir: devolve 0 para cada espectador.
        return vec![0; viewers.len()];
    };

    // O alvo é guardado como índice no slice; o número da camada sai de `layers[i].layer`.
    // Começa todo mundo na camada mais barata (a última do slice).
    let mut alvo: Vec<usize> = vec![last; viewers.len()];
    // Banda negativa vira 0: fica na camada mais barata.
    let disponivel: Vec<f64> = viewers.iter().map(|v| v.available_kbps.max(0.0)).collect();

    loop {
        // Rodízio: a cada passada, quem está na camada de menor qualidade (número maior) vem
        // primeiro; empate pelo índice do espectador no slice.
        let mut ordem: Vec<usize> = (0..viewers.len()).collect();
        ordem.sort_by(|&a, &b| layers[alvo[b]].layer.cmp(&layers[alvo[a]].layer).then(a.cmp(&b)));

        let mut subiu = false;
        for &e in &ordem {
            let atual = alvo[e];
            // Já está na melhor camada: não há próxima.
            if atual == 0 {
                continue;
            }
            let c = atual - 1;
            let cam = &layers[c];
            // Além do teto do espectador.
            if cam.layer < viewers[e].max_layer {
                continue;
            }
            // Banda não comporta a próxima camada com folga de 15%.
            if disponivel[e] < 1.15 * f64::from(cam.kbps) {
                continue;
            }
            // Modo direto: o upload do publicador é dividido entre todos os espectadores.
            if let Some(b) = upload_budget_kbps {
                let uso: f64 = alvo.iter().map(|&i| f64::from(layers[i].kbps)).sum();
                if uso + f64::from(cam.kbps) - f64::from(layers[atual].kbps) > b {
                    continue;
                }
            }
            alvo[e] = c;
            subiu = true;
        }

        if !subiu {
            break;
        }
    }

    alvo.iter().map(|&i| layers[i].layer).collect()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn camada(layer: u8, kbps: u32) -> LayerSpec {
        LayerSpec {
            layer,
            width: 1280,
            height: 720,
            fps: 30,
            kbps,
        }
    }

    /// Escada padrão: 8000 / 2500 / 400 kbps nas camadas 0, 1 e 2.
    fn escada_padrao() -> Vec<LayerSpec> {
        vec![camada(0, 8000), camada(1, 2500), camada(2, 400)]
    }

    fn espectador(available_kbps: f64, max_layer: u8) -> FillViewer {
        FillViewer {
            available_kbps,
            max_layer,
        }
    }

    #[test]
    fn modo_local_3() {
        let layers = escada_padrao();
        let viewers = [espectador(100_000.0, 0); 3];
        assert_eq!(fill(&layers, &viewers, Some(9000.0)), vec![1, 1, 1]);
    }

    #[test]
    fn modo_local_2() {
        let layers = escada_padrao();
        let viewers = [espectador(100_000.0, 0); 2];
        assert_eq!(fill(&layers, &viewers, Some(9000.0)), vec![1, 1]);
    }

    #[test]
    fn modo_local_1() {
        let layers = escada_padrao();
        let viewers = [espectador(100_000.0, 0)];
        assert_eq!(fill(&layers, &viewers, Some(9000.0)), vec![0]);
    }

    #[test]
    fn banda_limitada() {
        let layers = escada_padrao();
        let viewers = [espectador(30_000.0, 0), espectador(3_400.0, 0), espectador(20_000.0, 2)];
        assert_eq!(fill(&layers, &viewers, None), vec![0, 1, 2]);
    }

    #[test]
    fn teto_respeitado() {
        let layers = escada_padrao();
        let viewers = [espectador(100_000.0, 1)];
        assert_eq!(fill(&layers, &viewers, None), vec![1]);
    }

    #[test]
    fn camera_indices_0_e_2() {
        // Câmera: só as camadas 0 e 2 existem; o índice 1 não.
        let layers = vec![camada(0, 1500), camada(2, 300)];
        let viewers = [espectador(100_000.0, 0), espectador(400.0, 0)];
        assert_eq!(fill(&layers, &viewers, None), vec![0, 2]);
    }

    #[test]
    fn sem_espectadores_vazio() {
        let layers = escada_padrao();
        assert!(fill(&layers, &[], Some(9000.0)).is_empty());
    }

    #[test]
    fn sem_camadas_devolve_zeros() {
        let viewers = [espectador(100_000.0, 0), espectador(100_000.0, 0)];
        assert_eq!(fill(&[], &viewers, None), vec![0, 0]);
    }

    #[test]
    fn banda_negativa_fica_na_mais_barata() {
        let layers = escada_padrao();
        let viewers = [espectador(-500.0, 0)];
        assert_eq!(fill(&layers, &viewers, None), vec![2]);
    }
}
