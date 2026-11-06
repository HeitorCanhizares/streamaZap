//! Fontes de captura: listagem, interpretação de ids e miniaturas.

use zap_audio::AudioSelection;
use zap_media::{MonitorRef, VideoSourceSpec};

#[cfg(windows)]
use crate::types::AppSoundView;
use crate::types::{AudioChoice, Rgba, SourceItem, SourceKind, SourceRef, Sources};
use crate::CoreError;

/// Resolução da fonte sintética sempre oferecida (qualquer plataforma).
const SINTETICA_LARGURA: u32 = 1920;
const SINTETICA_ALTURA: u32 = 1080;

/// Lado máximo, em pixels, de uma miniatura.
const MINIATURA_MAX: u32 = 1920;

/// Lista monitores (`zap_capture::list_monitors`), janelas (`list_windows`), câmeras
/// (`list_cameras`), apps com som (`zap_audio::list_audio_apps`, só Windows) e codificadores
/// (`["auto","hardware","software"]`). Erros de listagem viram lista vazia (log). Em qualquer
/// plataforma acrescenta em `screens` a fonte `synthetic:1920x1080` ("Tela de teste (sintética)").
/// Ids: `screen:<index>`, `window:<hwnd>`, `camera:<id>`, `app:<pid>`.
pub fn list_sources() -> Sources {
    let mut screens = match zap_capture::list_monitors() {
        Ok(monitores) => monitores
            .into_iter()
            .map(|m| SourceItem {
                id: format!("screen:{}", m.index),
                kind: SourceKind::Screen,
                name: m.name,
                sound: None,
            })
            .collect(),
        Err(e) => {
            // Sem monitores listados a interface ainda tem a fonte sintética abaixo.
            tracing::warn!(erro = %e, "falha ao listar monitores");
            Vec::new()
        }
    };

    // Os processos com janela também entram na lista de som: dá para escolher um app antes de
    // ele tocar (a captura começa a pegar o som assim que ele abrir o áudio).
    #[cfg_attr(not(windows), allow(unused_mut, unused_variables))]
    let mut pids_com_janela: Vec<u32> = Vec::new();
    let windows = match zap_capture::list_windows() {
        Ok(janelas) => janelas
            .into_iter()
            .map(|w| {
                pids_com_janela.push(w.pid);
                SourceItem {
                    id: format!("window:{}", w.hwnd),
                    kind: SourceKind::Window,
                    name: w.title,
                    sound: None,
                }
            })
            .collect(),
        Err(e) => {
            tracing::warn!(erro = %e, "falha ao listar janelas");
            Vec::new()
        }
    };

    let cameras = match zap_capture::list_cameras() {
        Ok(cams) => cams
            .into_iter()
            .map(|c| SourceItem {
                id: format!("camera:{}", c.id),
                kind: SourceKind::Camera,
                name: c.name,
                sound: None,
            })
            .collect(),
        Err(e) => {
            tracing::warn!(erro = %e, "falha ao listar câmeras");
            Vec::new()
        }
    };

    // Apps com som dependem de WASAPI: só existem no Windows.
    #[cfg(windows)]
    let apps = match zap_audio::list_audio_apps(&pids_com_janela) {
        Ok(lista) => lista
            .into_iter()
            .map(|a| SourceItem {
                id: format!("app:{}", a.pid),
                kind: SourceKind::App,
                name: a.name,
                sound: Some(match a.sound {
                    zap_audio::AppSound::Playing => AppSoundView::Playing,
                    zap_audio::AppSound::Ready => AppSoundView::Ready,
                    zap_audio::AppSound::Silent => AppSoundView::Silent,
                }),
            })
            .collect(),
        Err(e) => {
            tracing::warn!(erro = %e, "falha ao listar apps com som");
            Vec::new()
        }
    };
    #[cfg(not(windows))]
    let apps: Vec<SourceItem> = Vec::new();

    // Fonte sintética no fim: permite testar o pipeline em qualquer máquina.
    screens.push(SourceItem {
        id: format!("synthetic:{SINTETICA_LARGURA}x{SINTETICA_ALTURA}"),
        kind: SourceKind::Screen,
        name: "Tela de teste (sintética)".to_string(),
        sound: None,
    });

    Sources {
        screens,
        windows,
        cameras,
        apps,
        encoders: vec!["auto".into(), "hardware".into(), "software".into()],
    }
}

/// `screen:N` → Monitor(N), `window:H` → Window(H), `camera:ID` → Camera(ID),
/// `synthetic:LxA` → Synthetic. Qualquer outra coisa: `CoreError::Invalid`.
pub fn parse_source(id: &str) -> Result<SourceRef, CoreError> {
    interpreta_fonte(id).ok_or_else(|| CoreError::Invalid(format!("fonte desconhecida: {id}")))
}

/// Versão sem o erro: `None` quando o id não é válido.
fn interpreta_fonte(id: &str) -> Option<SourceRef> {
    if let Some(resto) = id.strip_prefix("screen:") {
        return resto.parse::<u32>().ok().map(SourceRef::Monitor);
    }
    if let Some(resto) = id.strip_prefix("window:") {
        return resto.parse::<isize>().ok().map(SourceRef::Window);
    }
    if let Some(resto) = id.strip_prefix("camera:") {
        // Id de câmera é um symbolic link: qualquer texto não vazio serve.
        return (!resto.is_empty()).then(|| SourceRef::Camera(resto.to_string()));
    }
    if let Some(resto) = id.strip_prefix("synthetic:") {
        let (l, a) = resto.split_once('x')?;
        let width = l.parse::<u32>().ok()?;
        let height = a.parse::<u32>().ok()?;
        // Zero pixels não geram quadro; recusa já na interpretação.
        return (width > 0 && height > 0).then_some(SourceRef::Synthetic { width, height });
    }
    None
}

pub fn to_video_spec(src: &SourceRef) -> VideoSourceSpec {
    match src {
        SourceRef::Monitor(n) => VideoSourceSpec::Monitor(MonitorRef(*n)),
        SourceRef::Window(h) => VideoSourceSpec::Window(*h),
        SourceRef::Camera(id) => VideoSourceSpec::Camera(id.clone()),
        SourceRef::Synthetic { width, height } => VideoSourceSpec::Synthetic {
            width: *width,
            height: *height,
        },
    }
}

/// `"none"` → None, `"all"` → All, lista `app:<pid>` → Apps(pids). Outro texto: Invalid.
pub fn parse_audio(choice: &AudioChoice) -> Result<AudioSelection, CoreError> {
    match choice {
        AudioChoice::Keyword(k) => match k.as_str() {
            "none" => Ok(AudioSelection::None),
            "all" => Ok(AudioSelection::All),
            outro => Err(CoreError::Invalid(format!("áudio desconhecido: {outro}"))),
        },
        AudioChoice::Apps(ids) => {
            let pids = ids
                .iter()
                .map(|id| {
                    id.strip_prefix("app:")
                        .and_then(|pid| pid.parse::<u32>().ok())
                        .ok_or_else(|| CoreError::Invalid(format!("app de áudio desconhecido: {id}")))
                })
                .collect::<Result<Vec<u32>, CoreError>>()?;
            Ok(AudioSelection::Apps(pids))
        }
    }
}

/// Miniatura RGBA `w`×`h` da fonte. M3: placeholder determinístico (degradê cuja cor deriva do
/// hash do id da fonte); captura real de miniaturas fica para depois (nota do decompositor).
pub fn thumbnail(src: &SourceRef, w: u32, h: u32) -> Result<Rgba, CoreError> {
    check_thumbnail_size(w, h)?;

    // A cor base vem do hash da fonte: a mesma fonte sempre gera a mesma imagem.
    let base = fnv1a32(format!("{src:?}").as_bytes());
    let (r0, g0, b0) = ((base >> 16) as u8, (base >> 8) as u8, base as u8);

    // Diagonal: o canto superior esquerdo é a cor base; o inferior direito escurece até ~50%.
    let diagonal = (w + h - 2).max(1);
    let mut data = Vec::with_capacity(w as usize * h as usize * 4);
    for y in 0..h {
        for x in 0..w {
            let t = (x + y) * 255 / diagonal;
            let fator = 255 - t / 2;
            let canal = |c: u8| -> u8 { ((u32::from(c) * fator) / 255) as u8 };
            data.extend_from_slice(&[canal(r0), canal(g0), canal(b0), 255]);
        }
    }

    Ok(Rgba {
        width: w,
        height: h,
        data,
    })
}

/// Cada lado da miniatura entre 1 e `MINIATURA_MAX`.
pub(crate) fn check_thumbnail_size(w: u32, h: u32) -> Result<(), CoreError> {
    if !(1..=MINIATURA_MAX).contains(&w) || !(1..=MINIATURA_MAX).contains(&h) {
        return Err(CoreError::Invalid(format!(
            "dimensão de miniatura inválida: {w}x{h} (1 a {MINIATURA_MAX})"
        )));
    }
    Ok(())
}

/// FNV-1a de 32 bits: hash simples e estável entre execuções (ao contrário de `DefaultHasher`).
fn fnv1a32(bytes: &[u8]) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for &b in bytes {
        hash ^= u32::from(b);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_source_valido() {
        assert_eq!(parse_source("screen:2").unwrap(), SourceRef::Monitor(2));
        assert_eq!(parse_source("window:-65").unwrap(), SourceRef::Window(-65));
        assert_eq!(
            parse_source("camera:\\\\?\\usb#vid_1").unwrap(),
            SourceRef::Camera("\\\\?\\usb#vid_1".into())
        );
        assert_eq!(
            parse_source("synthetic:1280x720").unwrap(),
            SourceRef::Synthetic {
                width: 1280,
                height: 720
            }
        );
    }

    #[test]
    fn parse_source_invalido() {
        for id in ["", "screen:x", "synthetic:0x10", "foo:1", "camera:"] {
            match parse_source(id) {
                Err(CoreError::Invalid(msg)) => assert!(msg.contains(id), "mensagem: {msg}"),
                outro => panic!("{id:?} deveria ser inválido, veio {outro:?}"),
            }
        }
    }

    #[test]
    fn to_video_spec_mapeia() {
        assert!(matches!(
            to_video_spec(&SourceRef::Monitor(3)),
            VideoSourceSpec::Monitor(MonitorRef(3))
        ));
        assert!(matches!(
            to_video_spec(&SourceRef::Window(42)),
            VideoSourceSpec::Window(42)
        ));
        match to_video_spec(&SourceRef::Camera("cam".into())) {
            VideoSourceSpec::Camera(id) => assert_eq!(id, "cam"),
            outro => panic!("veio {outro:?}"),
        }
        assert!(matches!(
            to_video_spec(&SourceRef::Synthetic { width: 8, height: 4 }),
            VideoSourceSpec::Synthetic { width: 8, height: 4 }
        ));
    }

    #[test]
    fn parse_audio_casos() {
        let kw = |s: &str| AudioChoice::Keyword(s.into());
        assert_eq!(parse_audio(&kw("none")).unwrap(), AudioSelection::None);
        assert_eq!(parse_audio(&kw("all")).unwrap(), AudioSelection::All);
        assert_eq!(
            parse_audio(&AudioChoice::Apps(vec!["app:12".into(), "app:7".into()])).unwrap(),
            AudioSelection::Apps(vec![12, 7])
        );
        assert!(matches!(parse_audio(&kw("algum")), Err(CoreError::Invalid(_))));
        assert!(matches!(
            parse_audio(&AudioChoice::Apps(vec!["12".into()])),
            Err(CoreError::Invalid(_))
        ));
        assert!(matches!(
            parse_audio(&AudioChoice::Apps(vec!["app:x".into()])),
            Err(CoreError::Invalid(_))
        ));
    }

    #[test]
    fn list_sources_tem_sintetica_e_encoders() {
        let s = list_sources();
        let ultima = s.screens.last().expect("screens não pode estar vazio");
        assert_eq!(ultima.id, "synthetic:1920x1080");
        assert_eq!(ultima.name, "Tela de teste (sintética)");
        assert_eq!(s.encoders, vec!["auto", "hardware", "software"]);
    }

    #[test]
    fn thumbnail_tamanho_e_determinismo() {
        let fonte = SourceRef::Synthetic {
            width: 640,
            height: 480,
        };
        let a = thumbnail(&fonte, 8, 4).unwrap();
        let b = thumbnail(&fonte, 8, 4).unwrap();
        assert_eq!(a.width, 8);
        assert_eq!(a.height, 4);
        assert_eq!(a.data.len(), 8 * 4 * 4);
        assert_eq!(a, b);
        assert!(a.data.chunks_exact(4).all(|px| px[3] == 255));

        let outra = thumbnail(&SourceRef::Monitor(1), 8, 4).unwrap();
        assert_ne!(a.data, outra.data);
    }

    #[test]
    fn thumbnail_dimensao_invalida() {
        let fonte = SourceRef::Monitor(0);
        assert!(matches!(thumbnail(&fonte, 0, 10), Err(CoreError::Invalid(_))));
        assert!(matches!(thumbnail(&fonte, 10, 0), Err(CoreError::Invalid(_))));
        assert!(matches!(thumbnail(&fonte, 1921, 10), Err(CoreError::Invalid(_))));
        assert!(matches!(thumbnail(&fonte, 10, 1921), Err(CoreError::Invalid(_))));
        assert!(thumbnail(&fonte, 1920, 1).is_ok());
    }
}
