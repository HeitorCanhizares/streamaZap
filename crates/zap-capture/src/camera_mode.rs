//! Escolha do modo nativo da câmera (pura).

/// Um modo nativo anunciado pela câmera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CameraMode {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// Índice do tipo de mídia no Source Reader (`GetNativeMediaType`).
    pub type_index: u32,
}

/// Escolhe o índice (em `modes`) do melhor modo para o pedido.
/// Regras, em ordem: (1) só modos com `fps <= 30` se existir algum (senão todos); (2) preferir
/// modos com `width >= want_w` e `height >= want_h`; entre eles o de menor área; se nenhum, o de
/// maior área; (3) empate: `fps` mais perto de `want_fps` (preferindo o menor se igualmente
/// perto); (4) empate final: menor índice. `None` se `modes` vazio.
pub fn pick_camera_mode(modes: &[CameraMode], want_w: u32, want_h: u32, want_fps: u32) -> Option<usize> {
    if modes.is_empty() {
        return None;
    }

    // Até 30 fps é o que a captura aguenta com folga; acima só se não houver outra opção.
    let ate_30: Vec<usize> = (0..modes.len()).filter(|&i| modes[i].fps <= 30).collect();
    let candidatos: Vec<usize> = if ate_30.is_empty() {
        (0..modes.len()).collect()
    } else {
        ate_30
    };

    // Modos que cobrem o pedido: prefere o menor deles (menos dados); senão, o maior disponível.
    let grandes: Vec<usize> = candidatos
        .iter()
        .copied()
        .filter(|&i| modes[i].width >= want_w && modes[i].height >= want_h)
        .collect();
    let menor_area = !grandes.is_empty();
    let alvo = if menor_area { grandes } else { candidatos };

    let area = |i: usize| modes[i].width as u64 * modes[i].height as u64;
    let distancia_fps = |i: usize| modes[i].fps.abs_diff(want_fps);

    alvo.into_iter().min_by(|&a, &b| {
        let ordem_area = if menor_area {
            area(a).cmp(&area(b))
        } else {
            area(b).cmp(&area(a))
        };
        ordem_area
            .then(distancia_fps(a).cmp(&distancia_fps(b)))
            .then(modes[a].fps.cmp(&modes[b].fps))
            .then(a.cmp(&b))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modo(width: u32, height: u32, fps: u32) -> CameraMode {
        CameraMode {
            width,
            height,
            fps,
            type_index: 0,
        }
    }

    #[test]
    fn vazio_none() {
        assert_eq!(pick_camera_mode(&[], 1280, 720, 30), None);
    }

    #[test]
    fn prefere_menor_area_que_cobre_pedido() {
        let modes = [modo(640, 480, 30), modo(1280, 720, 30), modo(1920, 1080, 30)];
        assert_eq!(pick_camera_mode(&modes, 1280, 720, 30), Some(1));
    }

    #[test]
    fn sem_grande_pega_maior_area() {
        let modes = [modo(640, 480, 30), modo(1280, 720, 30), modo(800, 600, 30)];
        assert_eq!(pick_camera_mode(&modes, 3840, 2160, 30), Some(1));
    }

    #[test]
    fn ignora_acima_de_30fps_quando_ha_alternativa() {
        let modes = [modo(1920, 1080, 60), modo(1920, 1080, 30)];
        assert_eq!(pick_camera_mode(&modes, 1920, 1080, 60), Some(1));
    }

    #[test]
    fn so_60fps_usa_todos() {
        let modes = [modo(1280, 720, 60), modo(1920, 1080, 60)];
        assert_eq!(pick_camera_mode(&modes, 1280, 720, 60), Some(0));
    }

    #[test]
    fn empate_pelo_fps_mais_perto() {
        let modes = [modo(1280, 720, 24), modo(1280, 720, 30)];
        assert_eq!(pick_camera_mode(&modes, 1280, 720, 25), Some(0));
        assert_eq!(pick_camera_mode(&modes, 1280, 720, 28), Some(1));
    }

    #[test]
    fn empate_total_menor_indice() {
        let modes = [modo(1280, 720, 30), modo(1280, 720, 30)];
        assert_eq!(pick_camera_mode(&modes, 1280, 720, 30), Some(0));
    }
}
