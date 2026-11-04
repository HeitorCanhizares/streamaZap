//! Modelo do que a interface vê. Quem muda o estado chama o `set_*` e depois `Notifier::touch`.

use std::sync::Arc;

use crate::snapshot::*;
use crate::types::TileId;

/// Quantos erros recentes ficam guardados; os mais antigos são descartados.
const MAX_ERROS: usize = 5;

pub struct UiState {
    version: u64,
    me: Me,
    network: NetworkStatus,
    rooms: Vec<RoomListing>,
    peers: Vec<PeerListing>,
    room: Option<RoomView>,
    tiles: Vec<TileView>,
    share: Option<ShareView>,
    errors: Vec<String>,
    newer_protocol: Option<u32>,
}

impl UiState {
    pub fn new(me: Me) -> Self {
        Self {
            version: 0,
            me,
            network: NetworkStatus::default(),
            rooms: vec![],
            peers: vec![],
            room: None,
            tiles: vec![],
            share: None,
            errors: vec![],
            newer_protocol: None,
        }
    }
    pub fn set_me(&mut self, me: Me) {
        self.me = me;
    }
    pub fn set_network(&mut self, n: NetworkStatus) {
        self.network = n;
    }
    pub fn set_rooms(&mut self, r: Vec<RoomListing>) {
        self.rooms = r;
    }
    pub fn set_peers(&mut self, p: Vec<PeerListing>) {
        self.peers = p;
    }
    pub fn set_room(&mut self, r: Option<RoomView>) {
        self.room = r;
    }
    /// Troca ou insere o tile (chave: `tile`).
    pub fn upsert_tile(&mut self, t: TileView) {
        match self.tiles.iter_mut().find(|x| x.tile == t.tile) {
            Some(existente) => *existente = t,
            None => self.tiles.push(t),
        }
    }
    pub fn remove_tile(&mut self, tile: TileId) {
        self.tiles.retain(|t| t.tile != tile);
    }
    pub fn tiles(&self) -> &[TileView] {
        &self.tiles
    }
    pub fn set_share(&mut self, s: Option<ShareView>) {
        self.share = s;
    }
    /// Guarda no máximo os 5 últimos erros.
    pub fn push_error(&mut self, e: String) {
        self.errors.push(e);
        if self.errors.len() > MAX_ERROS {
            let excesso = self.errors.len() - MAX_ERROS;
            self.errors.drain(..excesso);
        }
    }
    pub fn set_newer_protocol(&mut self, p: Option<u32>) {
        self.newer_protocol = p;
    }
    pub fn clear_errors(&mut self) {
        self.errors.clear();
    }
    /// Incrementa `version` e devolve uma cópia imutável.
    pub fn snapshot(&mut self) -> Arc<Snapshot> {
        self.version += 1;
        Arc::new(Snapshot {
            version: self.version,
            me: self.me.clone(),
            network: self.network.clone(),
            rooms: self.rooms.clone(),
            peers: self.peers.clone(),
            room: self.room.clone(),
            tiles: self.tiles.clone(),
            share: self.share.clone(),
            errors: self.errors.clone(),
            newer_protocol: self.newer_protocol,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RoomMode;

    fn me(nome: &str) -> Me {
        Me {
            name: nome.into(),
            color: "#3BF08A".into(),
            install_id: "teste-install".into(),
        }
    }

    fn tile(id: TileId, largura: u32) -> TileView {
        TileView {
            tile: id,
            track_id: format!("trk-{id}"),
            layer: 0,
            width: largura,
            height: 720,
            fps: 30.0,
            kbps: 1000,
            ping_ms: 20,
            smoothing_ms: 200,
            reason: "teste".into(),
        }
    }

    fn sala(id: &str) -> RoomView {
        RoomView {
            id: id.into(),
            name: format!("Sala {id}"),
            mode: RoomMode::Local,
            code: None,
            participants: vec![],
            tracks: vec![],
            chat: vec![],
            unread: 0,
        }
    }

    #[test]
    fn version_incrementa_a_cada_snapshot() {
        let mut st = UiState::new(me("Ana"));
        assert_eq!(st.snapshot().version, 1);
        assert_eq!(st.snapshot().version, 2);
        assert_eq!(st.snapshot().version, 3);
    }

    #[test]
    fn upsert_substitui_e_insere_tile() {
        let mut st = UiState::new(me("Ana"));
        st.upsert_tile(tile(0, 1280));
        st.upsert_tile(tile(1, 640));
        // Mesmo `tile` substitui a entrada existente, sem duplicar nem mudar a ordem.
        st.upsert_tile(tile(0, 1920));
        let tiles = st.tiles();
        assert_eq!(tiles.len(), 2);
        assert_eq!(tiles[0].tile, 0);
        assert_eq!(tiles[0].width, 1920);
        assert_eq!(tiles[1].tile, 1);
    }

    #[test]
    fn remove_tile() {
        let mut st = UiState::new(me("Ana"));
        st.upsert_tile(tile(0, 1280));
        st.upsert_tile(tile(1, 640));
        st.remove_tile(0);
        assert_eq!(st.tiles().len(), 1);
        assert_eq!(st.tiles()[0].tile, 1);
        // Remover um tile inexistente não faz nada.
        st.remove_tile(42);
        assert_eq!(st.tiles().len(), 1);
    }

    #[test]
    fn erros_maximo_5_mantem_ultimos() {
        let mut st = UiState::new(me("Ana"));
        for i in 1..=7 {
            st.push_error(format!("erro {i}"));
        }
        let snap = st.snapshot();
        assert_eq!(snap.errors, vec!["erro 3", "erro 4", "erro 5", "erro 6", "erro 7"]);
        st.clear_errors();
        assert!(st.snapshot().errors.is_empty());
    }

    #[test]
    fn snapshot_e_copia_imutavel() {
        let mut st = UiState::new(me("Ana"));
        st.set_room(Some(sala("a")));
        st.upsert_tile(tile(0, 1280));
        let antigo = st.snapshot();

        st.set_me(me("Bia"));
        st.set_room(None);
        st.remove_tile(0);
        st.push_error("depois".into());

        // O Arc já devolvido continua com o estado do momento.
        assert_eq!(antigo.me.name, "Ana");
        assert_eq!(antigo.room.as_ref().map(|r| r.id.as_str()), Some("a"));
        assert_eq!(antigo.tiles.len(), 1);
        assert!(antigo.errors.is_empty());

        let novo = st.snapshot();
        assert_eq!(novo.version, 2);
        assert_eq!(novo.me.name, "Bia");
        assert!(novo.room.is_none());
    }

    #[test]
    fn set_room_none_limpa() {
        let mut st = UiState::new(me("Ana"));
        st.set_room(Some(sala("a")));
        assert!(st.snapshot().room.is_some());
        st.set_room(None);
        assert!(st.snapshot().room.is_none());
    }

    #[test]
    fn newer_protocol_comeca_none_e_propaga() {
        let mut st = UiState::new(me("Ana"));
        assert_eq!(st.snapshot().newer_protocol, None);

        st.set_newer_protocol(Some(5));
        assert_eq!(st.snapshot().newer_protocol, Some(5));

        st.set_newer_protocol(None);
        assert_eq!(st.snapshot().newer_protocol, None);
    }
}
