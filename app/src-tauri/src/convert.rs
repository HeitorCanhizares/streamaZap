//! Conversões puras entre o formato da interface e o do núcleo (testáveis sem janela).

use zap_core::{CoreError, Rgba, TileSize};

/// Tamanho do tile em pixels (`[largura, altura]` do `api.ts`) para o `TileSize` do núcleo:
/// largura < 640 → `Thumbnail`; < 1600 → `Main`; senão `Full`.
pub fn tile_size_from_px(size: [u32; 2]) -> TileSize {
    let largura = size[0];
    if largura < 640 {
        TileSize::Thumbnail
    } else if largura < 1600 {
        TileSize::Main
    } else {
        TileSize::Full
    }
}

/// Formato da miniatura (lido por `decodeThumbnail`): `u32 largura LE, u32 altura LE` + RGBA.
pub fn encode_thumbnail(img: &Rgba) -> Vec<u8> {
    // Cabeçalho de 8 bytes em little-endian, igual ao que o decodificador da interface espera.
    let mut out = Vec::with_capacity(8 + img.data.len());
    out.extend_from_slice(&img.width.to_le_bytes());
    out.extend_from_slice(&img.height.to_le_bytes());
    out.extend_from_slice(&img.data);
    out
}

/// Mensagem para a interface (`Display` do erro em português).
pub fn err_string(e: CoreError) -> String {
    e.to_string()
}

/// `chat_typing(typing)`: só `true` gera aviso (o núcleo só tem `send_typing`).
pub fn should_send_typing(typing: bool) -> bool {
    typing
}

#[cfg(test)]
mod tests {
    use super::*;
    use zap_core::online::{AccountProviderView, AccountView};
    use zap_core::snapshot::{TrackView, TrackViewKind};
    use zap_core::{ServerTestResult, Snapshot, SourceItem, SourceKind};
    use zap_proto::plan::{PlanId, PlanInfo};

    #[test]
    fn tile_size_limites() {
        assert_eq!(tile_size_from_px([639, 360]), TileSize::Thumbnail);
        assert_eq!(tile_size_from_px([640, 360]), TileSize::Main);
        assert_eq!(tile_size_from_px([1599, 900]), TileSize::Main);
        assert_eq!(tile_size_from_px([1600, 900]), TileSize::Full);
    }

    #[test]
    fn miniatura_formato() {
        let img = Rgba {
            width: 2,
            height: 1,
            data: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        let bytes = encode_thumbnail(&img);
        assert_eq!(bytes.len(), 16);
        assert_eq!(&bytes[0..4], &2u32.to_le_bytes());
        assert_eq!(&bytes[4..8], &1u32.to_le_bytes());
        assert_eq!(&bytes[8..], img.data.as_slice());
    }

    #[test]
    fn miniatura_confere_com_decodificador() {
        // Mesma conta de `decodeThumbnail`: len == 8 + w*h*4.
        let img = Rgba {
            width: 3,
            height: 2,
            data: vec![0; 3 * 2 * 4],
        };
        let bytes = encode_thumbnail(&img);
        assert_eq!(bytes.len(), 8 + 3 * 2 * 4);
    }

    #[test]
    fn erro_em_portugues() {
        assert_eq!(err_string(CoreError::NoRoom), "Você não está em uma sala");
    }

    #[test]
    fn typing_so_true() {
        assert!(should_send_typing(true));
        assert!(!should_send_typing(false));
    }

    // Sources e Snapshot cruzam a ponte por serde direto: estes testes seguram o JSON que a
    // interface lê (sem conversão manual no app).

    #[test]
    fn fonte_de_camera_serializa_modo_maximo() {
        let item = SourceItem {
            id: "camera:x".into(),
            kind: SourceKind::Camera,
            name: "Cam".into(),
            sound: None,
            max_height: Some(1080),
            max_fps: Some(60),
        };
        let v = serde_json::to_value(&item).expect("json");
        assert_eq!(v["max_height"].as_u64(), Some(1080));
        assert_eq!(v["max_fps"].as_u64(), Some(60));
    }

    #[test]
    fn fonte_sem_modo_omite_os_campos() {
        let item = SourceItem {
            id: "camera:x".into(),
            kind: SourceKind::Camera,
            name: "Cam".into(),
            sound: None,
            max_height: None,
            max_fps: None,
        };
        let v = serde_json::to_value(&item).expect("json");
        assert!(v.get("max_height").is_none());
        assert!(v.get("max_fps").is_none());
    }

    #[test]
    fn trilha_de_camera_leva_o_jeito() {
        let base = |layout| TrackView {
            id: "t1".into(),
            owner_id: "p1".into(),
            kind: TrackViewKind::Camera,
            title: "Câmera".into(),
            number: 1,
            camera_layout: layout,
        };

        let solta = serde_json::to_value(base(Some(zap_proto::CameraLayout::Free))).expect("json");
        assert_eq!(solta["camera_layout"], serde_json::json!({"modo": "solta"}));

        let canto = serde_json::to_value(base(Some(zap_proto::CameraLayout::Corner {
            corner: zap_proto::Corner::TopLeft,
        })))
        .expect("json");
        assert_eq!(
            canto["camera_layout"],
            serde_json::json!({"modo": "canto", "canto": "sup_esq"})
        );

        let sem = serde_json::to_value(base(None)).expect("json");
        assert!(sem.get("camera_layout").is_none());
    }

    #[test]
    fn snapshot_de_exemplo_vai_e_volta() {
        let original = Snapshot::example();
        let v = serde_json::to_value(&original).expect("json");
        let volta: Snapshot = serde_json::from_value(v).expect("de volta");
        assert_eq!(volta, original);
    }

    // Campos de conta (fase 1 de contas/planos): o JSON que a interface lê.

    fn provedor() -> AccountProviderView {
        AccountProviderView {
            provider: "supabase".into(),
            url: "https://x.supabase.co".into(),
            key: "anon".into(),
        }
    }

    fn conta_completa() -> AccountView {
        AccountView {
            account_provider: Some(provedor()),
            requires_account: true,
            requires_password: false,
            account_id: Some("u1".into()),
            plan: Some(PlanInfo::of(PlanId::Hd)),
            login_error: None,
        }
    }

    #[test]
    fn account_view_completo_serializa() {
        let v = serde_json::to_value(conta_completa()).expect("json");
        assert_eq!(v["account_provider"]["provider"], "supabase");
        assert_eq!(v["requires_account"], true);
        assert_eq!(v["requires_password"], false);
        assert_eq!(v["account_id"], "u1");
        assert_eq!(v["plan"]["id"], "hd");
        assert_eq!(v["plan"]["transmitir"]["altura_max"], 720);
        assert!(v.get("login_error").is_none());
    }

    #[test]
    fn account_view_vazio_omite_opcionais() {
        let v = serde_json::to_value(AccountView::default()).expect("json");
        let mut chaves: Vec<&str> = v.as_object().expect("objeto").keys().map(|k| k.as_str()).collect();
        chaves.sort_unstable();
        assert_eq!(chaves, vec!["requires_account", "requires_password"]);
    }

    #[test]
    fn login_error_serializa() {
        for erro in ["conta_necessaria", "senha_servidor"] {
            let conta = AccountView {
                login_error: Some(erro.into()),
                ..Default::default()
            };
            let v = serde_json::to_value(conta).expect("json");
            assert_eq!(v["login_error"], erro);
        }
    }

    #[test]
    fn snapshot_com_conta_vai_e_volta() {
        let mut original = Snapshot::example();
        original.account = Some(conta_completa());
        let v = serde_json::to_value(&original).expect("json");
        assert!(v.get("account").is_some());
        let volta: Snapshot = serde_json::from_value(v).expect("de volta");
        assert_eq!(volta, original);
    }

    #[test]
    fn snapshot_sem_conta_omite_chave() {
        let v = serde_json::to_value(Snapshot::example()).expect("json");
        assert!(v.get("account").is_none());
    }

    #[test]
    fn server_test_result_leva_conta() {
        let com = ServerTestResult {
            ok: true,
            requires_account: true,
            account_provider: Some(provedor()),
            ..Default::default()
        };
        let v = serde_json::to_value(&com).expect("json");
        assert_eq!(v["requires_account"], true);
        assert_eq!(v["account_provider"]["provider"], "supabase");

        let sem = ServerTestResult {
            ok: true,
            requires_account: true,
            ..Default::default()
        };
        let v = serde_json::to_value(&sem).expect("json");
        assert_eq!(v["requires_account"], true);
        assert!(v.get("account_provider").is_none());
    }

    #[test]
    fn snapshot_antigo_sem_account_desserializa() {
        let mut v = serde_json::to_value(Snapshot::example()).expect("json");
        v.as_object_mut().expect("objeto").remove("account");
        let s: Snapshot = serde_json::from_value(v).expect("de volta");
        assert!(s.account.is_none());
    }
}
