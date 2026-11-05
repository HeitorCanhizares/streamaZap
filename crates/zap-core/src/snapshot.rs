//! Snapshot entregue à interface. Formato EXATO de `app/src/bridge/types.ts` (snake_case).
//! Campos `?` do TypeScript = `Option` com `skip_serializing_if`.

use serde::{Deserialize, Serialize};

use crate::types::{Preset, RoomMode, TileId};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SceneKind {
    Game,
    Race,
    Movie,
    Code,
    Desktop,
    Cama,
    Camb,
    Camc,
    Camd,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Me {
    pub name: String,
    pub color: String,
    pub install_id: String,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SharerKind {
    Tela,
    Webcam,
    Janela,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RoomSharer {
    pub name: String,
    pub kind: SharerKind,
    pub scene: SceneKind,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RoomPerson {
    pub name: String,
    pub role: String,
    pub ping_ms: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RoomListing {
    pub room_id: String,
    pub name: String,
    pub host: String,
    pub participants: u32,
    pub has_password: bool,
    pub mode: RoomMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene: Option<SceneKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unread: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typing: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ping_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewers: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_warn: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cam_scene: Option<SceneKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sharers: Option<Vec<RoomSharer>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub people: Option<Vec<RoomPerson>>,
}

impl RoomListing {
    /// Listagem mínima (só os campos obrigatórios).
    pub fn basic(
        room_id: String,
        name: String,
        host: String,
        participants: u32,
        has_password: bool,
        mode: RoomMode,
    ) -> Self {
        Self {
            room_id,
            name,
            host,
            participants,
            has_password,
            mode,
            live_title: None,
            scene: None,
            addr: None,
            last_message: None,
            last_time: None,
            unread: None,
            typing: None,
            ping_ms: None,
            quality: None,
            bitrate: None,
            viewers: None,
            version_warn: None,
            cam_scene: None,
            sharers: None,
            people: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PeerListing {
    pub name: String,
    pub version: String,
    pub ip: String,
    pub receives_you: bool,
}

// Sem `Eq`: `volume` é f32 (o TS usa number). Divergência corrigida: o TS tem `volume?`/`muted?`
// e estes campos faltavam aqui.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Participant {
    pub id: String,
    pub name: String,
    pub color: String,
    pub ping_ms: u32,
    pub sharing: bool,
    pub camera: bool,
    /// Volume 0..1 que EU escolhi para este participante; ausente = 0,8.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<f32>,
    /// Silenciado por mim; ausente = false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muted: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TrackViewKind {
    Screen,
    Window,
    Camera,
    /// Só o som (quem compartilha o áudio sem tela).
    Audio,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TrackView {
    pub id: String,
    pub owner_id: String,
    pub kind: TrackViewKind,
    pub title: String,
    pub number: u16,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RoomView {
    pub id: String,
    pub name: String,
    pub mode: RoomMode,
    /// Código para convite (ex.: `ZAP-7K2Q`); ausente na sala Local sem código.
    /// Divergência corrigida: o TS tem `code?` e o campo faltava aqui.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub participants: Vec<Participant>,
    pub tracks: Vec<TrackView>,
    pub chat: Vec<crate::types::ChatLine>,
    pub unread: u32,
    /// Meu id de participante nesta sala (no Online vem do servidor, diferente do `install_id`).
    /// Vazio (exemplos e testes) não sai no JSON.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub me_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TileView {
    pub tile: TileId,
    pub track_id: String,
    pub layer: u8,
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub kbps: u32,
    pub ping_ms: u32,
    pub smoothing_ms: u32,
    pub reason: String,
}

/// Onde a cor é convertida para NV12 numa camada que está codificando.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConverterView {
    /// Video Processor do D3D11.
    Gpu,
    /// Reserva na CPU (placa sem Video Processor, ou ele falhou).
    Cpu,
}

impl From<zap_media::ConverterKind> for ConverterView {
    fn from(k: zap_media::ConverterKind) -> Self {
        match k {
            zap_media::ConverterKind::Gpu => ConverterView::Gpu,
            zap_media::ConverterKind::Cpu => ConverterView::Cpu,
        }
    }
}

/// Conversor de uma camada que está codificando.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayerConversion {
    pub layer: u8,
    pub converter: ConverterView,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ShareView {
    pub source: String,
    pub audio: String,
    pub preset: Preset,
    pub encoder: String,
    pub layers: Vec<u8>,
    pub viewers_per_layer: Vec<u32>,
    pub encode_ms: f32,
    /// Conversor de cada camada codificando agora, em ordem de camada.
    #[serde(default)]
    pub conversion: Vec<LayerConversion>,
    /// O pedido em vigor (fonte, som, qualidade, codificador): a sala abre o seletor com ele.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<crate::types::ShareRequest>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct NetworkStatus {
    pub radmin_ip: Option<String>,
    pub interfaces: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub version: u64,
    pub me: Me,
    pub network: NetworkStatus,
    pub rooms: Vec<RoomListing>,
    pub peers: Vec<PeerListing>,
    pub room: Option<RoomView>,
    pub tiles: Vec<TileView>,
    pub share: Option<ShareView>,
    pub errors: Vec<String>,
    /// Protocolo maior visto na rede (amigo/servidor mais novo): a interface oferece atualizar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub newer_protocol: Option<u32>,
    /// Servidor Online (barra): `None` = sem endereço configurado.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<crate::online::ServerStatus>,
}

impl Snapshot {
    /// Snapshot de exemplo (dados do mock da interface) usado por `snapshot_exemplo`.
    pub fn example() -> Snapshot {
        Snapshot {
            version: 1,
            me: Me {
                name: "Você".into(),
                color: "#3BF08A".into(),
                install_id: "mock-install".into(),
            },
            network: NetworkStatus {
                radmin_ip: Some("26.14.7.88".into()),
                interfaces: vec!["Radmin VPN".into(), "Ethernet".into()],
            },
            rooms: vec![
                // Listagem com campos opcionais preenchidos.
                RoomListing {
                    room_id: "zap-7k2q".into(),
                    name: "Sexta do Rush".into(),
                    host: "Marcão".into(),
                    participants: 3,
                    has_password: false,
                    mode: RoomMode::Online,
                    live_title: None,
                    scene: Some(SceneKind::Game),
                    addr: Some("ZAP-7K2Q".into()),
                    last_message: Some("Rafa: bora, entra logo".into()),
                    last_time: Some("22:14".into()),
                    unread: Some(3),
                    typing: None,
                    ping_ms: Some(38),
                    quality: Some("1080p60".into()),
                    bitrate: Some("6,2 Mbps".into()),
                    viewers: Some(4),
                    version_warn: None,
                    cam_scene: Some(SceneKind::Cama),
                    sharers: Some(vec![
                        RoomSharer {
                            name: "Marcão".into(),
                            kind: SharerKind::Tela,
                            scene: SceneKind::Game,
                        },
                        RoomSharer {
                            name: "Ju".into(),
                            kind: SharerKind::Webcam,
                            scene: SceneKind::Camb,
                        },
                        RoomSharer {
                            name: "Dudu".into(),
                            kind: SharerKind::Janela,
                            scene: SceneKind::Desktop,
                        },
                    ]),
                    people: Some(vec![
                        RoomPerson {
                            name: "Marcão".into(),
                            role: "host · compartilhando".into(),
                            ping_ms: 12,
                        },
                        RoomPerson {
                            name: "Ju".into(),
                            role: "webcam ligada".into(),
                            ping_ms: 38,
                        },
                        RoomPerson {
                            name: "Rafa".into(),
                            role: "assistindo Marcão".into(),
                            ping_ms: 61,
                        },
                        RoomPerson {
                            name: "Dudu".into(),
                            role: "compartilhando janela".into(),
                            ping_ms: 45,
                        },
                    ]),
                },
                // Listagem mínima: só os campos obrigatórios.
                RoomListing::basic(
                    "zap-3m9x".into(),
                    "Filminho de domingo".into(),
                    "Ju".into(),
                    2,
                    true,
                    RoomMode::Online,
                ),
            ],
            peers: vec![
                PeerListing {
                    name: "Marcão".into(),
                    version: "3.0.0".into(),
                    ip: "26.201.4.12".into(),
                    receives_you: true,
                },
                PeerListing {
                    name: "Ju".into(),
                    version: "3.0.0".into(),
                    ip: "26.88.19.5".into(),
                    receives_you: true,
                },
                PeerListing {
                    name: "Rafa".into(),
                    version: "3.0.0".into(),
                    ip: "26.3.140.77".into(),
                    receives_you: false,
                },
                PeerListing {
                    name: "Dudu".into(),
                    version: "3.0.0".into(),
                    ip: "26.57.210.9".into(),
                    receives_you: true,
                },
            ],
            room: Some(RoomView {
                id: "zap-7k2q".into(),
                name: "Sexta do Rush".into(),
                mode: RoomMode::Online,
                code: Some("ZAP-7K2Q".into()),
                participants: vec![
                    Participant {
                        id: "p-marcao".into(),
                        name: "Marcão".into(),
                        color: "#FFB547".into(),
                        ping_ms: 24,
                        sharing: true,
                        camera: false,
                        volume: Some(0.8),
                        muted: Some(false),
                    },
                    Participant {
                        id: "p-ju".into(),
                        name: "Ju".into(),
                        color: "#7CF2C0".into(),
                        ping_ms: 58,
                        sharing: false,
                        camera: true,
                        volume: Some(0.6),
                        muted: Some(false),
                    },
                ],
                tracks: vec![TrackView {
                    id: "trk-1".into(),
                    owner_id: "p-marcao".into(),
                    kind: TrackViewKind::Screen,
                    title: "Tela inteira".into(),
                    number: 1,
                }],
                chat: vec![crate::types::ChatLine {
                    id: "c3".into(),
                    author_id: "p-marcao".into(),
                    author: "Marcão".into(),
                    color: "#FFB547".into(),
                    text: "tá sim, só o cs2.exe. o spot tá limpo".into(),
                    ts: 1767312720000,
                }],
                unread: 0,
                me_id: String::new(),
            }),
            tiles: vec![TileView {
                tile: 0,
                track_id: "trk-1".into(),
                layer: 2,
                width: 1280,
                height: 720,
                fps: 60.0,
                kbps: 5200,
                ping_ms: 24,
                smoothing_ms: 200,
                reason: "banda ok".into(),
            }],
            share: Some(ShareView {
                source: "screen:0".into(),
                audio: "all".into(),
                preset: Preset::P1080p60,
                encoder: "NVENC".into(),
                layers: vec![0, 1, 2],
                viewers_per_layer: vec![0, 1, 3],
                encode_ms: 6.5,
                conversion: vec![LayerConversion {
                    layer: 0,
                    converter: ConverterView::Gpu,
                }],
                request: None,
            }),
            errors: Vec::new(),
            newer_protocol: None,
            server: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Saída de referência do `snapshot_exemplo`, usada pela interface. Para regravar:
    /// `ZAP_UPDATE_SNAPSHOT=1 cargo test -p zap-core snapshot_exemplo`.
    const ARQUIVO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/snapshot_exemplo.json");

    #[test]
    fn snapshot_exemplo() {
        let json = serde_json::to_string_pretty(&Snapshot::example()).expect("serializa o exemplo");
        // `--nocapture` mostra o JSON para a interface usar como fonte.
        println!("{json}");

        if std::env::var("ZAP_UPDATE_SNAPSHOT").as_deref() == Ok("1") {
            std::fs::write(ARQUIVO, format!("{json}\n")).expect("grava snapshot_exemplo.json");
            return;
        }
        let esperado = std::fs::read_to_string(ARQUIVO)
            .expect("le snapshot_exemplo.json (crie com ZAP_UPDATE_SNAPSHOT=1)")
            .replace("\r\n", "\n");
        assert_eq!(
            json,
            esperado.trim_end(),
            "snapshot difere de tests/snapshot_exemplo.json; se a mudança for intencional, rode com ZAP_UPDATE_SNAPSHOT=1"
        );
    }

    #[test]
    fn round_trip_serde() {
        let original = Snapshot::example();
        let json = serde_json::to_string(&original).expect("serializa");
        let volta: Snapshot = serde_json::from_str(&json).expect("desserializa");
        assert_eq!(volta, original);
    }

    #[test]
    fn opcionais_ausentes_nao_saem() {
        let basic = RoomListing::basic("zap-x".into(), "Sala".into(), "Host".into(), 1, false, RoomMode::Local);
        let valor = serde_json::to_value(&basic).expect("serializa");
        let objeto = valor.as_object().expect("objeto JSON");
        assert!(!objeto.contains_key("live_title"));
        for chave in ["scene", "addr", "sharers", "people", "ping_ms", "version_warn"] {
            assert!(!objeto.contains_key(chave), "chave opcional {chave} não deveria sair");
        }
        assert_eq!(objeto.get("mode").and_then(|m| m.as_str()), Some("local"));
    }

    #[test]
    fn chaves_conforme_ts() {
        let valor = serde_json::to_value(Snapshot::example()).expect("serializa");
        // serde_json ordena as chaves do objeto; a comparação é feita como conjunto.
        let mut recebidas: Vec<&str> = valor
            .as_object()
            .expect("objeto JSON")
            .keys()
            .map(String::as_str)
            .collect();
        let mut esperadas = vec![
            "version", "me", "network", "rooms", "peers", "room", "tiles", "share", "errors",
        ];
        recebidas.sort_unstable();
        esperadas.sort_unstable();
        assert_eq!(recebidas, esperadas);
    }

    #[test]
    fn newer_protocol_omitido_quando_none() {
        let valor = serde_json::to_value(Snapshot::example()).expect("serializa");
        let objeto = valor.as_object().expect("objeto JSON");
        assert!(!objeto.contains_key("newer_protocol"));
    }

    #[test]
    fn newer_protocol_aparece_quando_some() {
        let mut snap = Snapshot::example();
        snap.newer_protocol = Some(6);
        let valor = serde_json::to_value(&snap).expect("serializa");
        assert_eq!(valor.get("newer_protocol").and_then(|v| v.as_u64()), Some(6));

        let json = serde_json::to_string(&snap).expect("serializa");
        let volta: Snapshot = serde_json::from_str(&json).expect("desserializa");
        assert_eq!(volta.newer_protocol, Some(6));
        assert_eq!(volta, snap);
    }

    #[test]
    fn json_antigo_sem_o_campo_desserializa() {
        let mut valor = serde_json::to_value(Snapshot::example()).expect("serializa");
        valor.as_object_mut().expect("objeto JSON").remove("newer_protocol");
        let snap: Snapshot = serde_json::from_value(valor).expect("desserializa JSON antigo");
        assert_eq!(snap.newer_protocol, None);
    }
}
