//! Necessidade de sondagem de cada espectador (`docs/api/balanceamento.md` §4).

use zap_rate::probe::ProbeNeed;

use crate::state::{RoomData, TrackData};

/// Necessidade de sondagem do espectador `num` numa trilha: `current > teto`
/// (`relay::max_layer_for`) e sem `pending` -> camada existente logo acima de `current`;
/// `reserve_kbps = 0` (o alocador do servidor de teste não reserva áudio).
pub fn need_for(track: &TrackData, num: u64) -> Option<ProbeNeed> {
    let v = track.relay.viewers.get(&num)?;
    // Troca já em andamento: esperar o keyframe antes de sondar outra camada.
    if v.pending.is_some() {
        return None;
    }
    let teto = crate::relay::max_layer_for(v, &track.layers);
    // Número menor = camada melhor: `current <= teto` já está no teto ou acima dele.
    if v.current <= teto {
        return None;
    }
    // Alvo = a camada existente logo acima de `current` (maior número abaixo dele).
    // A câmera pode não ter a camada 1; nesse caso o alvo é a 0.
    let alvo = track
        .layers
        .iter()
        .filter(|l| l.layer < v.current)
        .max_by_key(|l| l.layer)?;
    Some(ProbeNeed {
        next_kbps: alvo.kbps,
        reserve_kbps: 0,
    })
}

/// Banda do último relatório do espectador nessa sala (de qualquer `ViewerEntry` dele; 0 se nenhum).
pub fn bandwidth_of(room: &RoomData, num: u64) -> u32 {
    // Cada trilha guarda sua própria cópia do relatório; a maior vale como a banda da sala.
    room.tracks
        .iter()
        .filter_map(|t| t.relay.viewers.get(&num))
        .map(|v| v.bandwidth_kbps)
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::collections::VecDeque;

    use super::*;
    use crate::relay::{TrackRelay, ViewerEntry};
    use zap_proto::{Access, DelayState, LayerSpec, RoomCode, TileSize, TrackKind};

    fn camada(layer: u8, kbps: u32) -> LayerSpec {
        LayerSpec {
            layer,
            width: 0,
            height: 0,
            fps: 0,
            kbps,
        }
    }

    /// Escada de teste: 8000 / 2500 / 400 kbps nas camadas 0 / 1 / 2.
    fn escada() -> Vec<LayerSpec> {
        vec![camada(0, 8000), camada(1, 2500), camada(2, 400)]
    }

    fn espectador(num: u64, size: TileSize, current: u8) -> ViewerEntry {
        ViewerEntry {
            participant_id: format!("p{num}"),
            num,
            size,
            max_layer_req: None,
            audio: false,
            preview: false,
            bandwidth_kbps: 0,
            state: DelayState::Normal,
            current,
            pending: None,
            announced: None,
        }
    }

    fn trilha(id: &str, layers: Vec<LayerSpec>, viewers: Vec<ViewerEntry>) -> TrackData {
        let mut relay = TrackRelay::new(layers.clone());
        for v in viewers {
            relay.viewers.insert(v.num, v);
        }
        TrackData {
            id: id.into(),
            number: 1,
            participant_id: "dona".into(),
            kind: TrackKind::Screen,
            title: "Tela".into(),
            layers,
            audio: None,
            relay,
        }
    }

    fn sala(tracks: Vec<TrackData>) -> RoomData {
        RoomData {
            id: "sala".into(),
            code: RoomCode::from_entropy([1, 2, 3, 4]),
            name: "Teste".into(),
            owner_id: "dona".into(),
            access: Access::Open,
            password_hash: None,
            created_at: "2026-01-01T00:00:00Z".into(),
            members: vec!["dona".into()],
            tracks,
            chat: VecDeque::new(),
            music: Default::default(),
            next_track_number: 2,
            empty_since_ms: None,
        }
    }

    #[test]
    fn need_ate_camada_logo_acima() {
        // Principal tem teto 0; está na 2 (400 kbps) -> sobe para a 1 (2500 kbps).
        let t = trilha("t", escada(), vec![espectador(7, TileSize::Main, 2)]);
        assert_eq!(
            need_for(&t, 7),
            Some(ProbeNeed {
                next_kbps: 2500,
                reserve_kbps: 0
            })
        );
    }

    #[test]
    fn sem_need_quando_no_teto() {
        let t = trilha(
            "t",
            escada(),
            vec![espectador(1, TileSize::Main, 0), espectador(2, TileSize::Thumbnail, 2)],
        );
        assert_eq!(need_for(&t, 1), None);
        assert_eq!(need_for(&t, 2), None);
    }

    #[test]
    fn sem_need_com_pending() {
        let mut v = espectador(7, TileSize::Main, 2);
        v.pending = Some(1);
        let t = trilha("t", escada(), vec![v]);
        assert_eq!(need_for(&t, 7), None);
    }

    #[test]
    fn camera_sem_camada_1() {
        // Só existem as camadas 0 e 2: a logo acima da 2 é a 0 (4000 kbps).
        let layers = vec![camada(0, 4000), camada(2, 400)];
        let t = trilha("t", layers, vec![espectador(7, TileSize::Main, 2)]);
        assert_eq!(
            need_for(&t, 7),
            Some(ProbeNeed {
                next_kbps: 4000,
                reserve_kbps: 0
            })
        );
    }

    #[test]
    fn sem_need_para_quem_nao_assiste() {
        let t = trilha("t", escada(), vec![espectador(7, TileSize::Main, 2)]);
        assert_eq!(need_for(&t, 42), None);
    }

    #[test]
    fn sem_need_na_previa() {
        // Prévia: teto fixo 2, então a camada 2 já é o teto.
        let mut v = espectador(7, TileSize::Full, 2);
        v.preview = true;
        let t = trilha("t", escada(), vec![v]);
        assert_eq!(need_for(&t, 7), None);
    }

    #[test]
    fn bandwidth_of_pega_o_maior_relatorio() {
        let mut a = espectador(7, TileSize::Main, 0);
        a.bandwidth_kbps = 1500;
        let mut b = espectador(7, TileSize::Thumbnail, 2);
        b.bandwidth_kbps = 900;
        let r = sala(vec![trilha("a", escada(), vec![a]), trilha("b", escada(), vec![b])]);
        assert_eq!(bandwidth_of(&r, 7), 1500);
        assert_eq!(bandwidth_of(&r, 99), 0);
    }
}
