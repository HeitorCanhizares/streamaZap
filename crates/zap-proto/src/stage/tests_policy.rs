//! Testes de `stage/policy.rs` (mesma tarefa).
use super::*;
use crate::types::{AudioSource, TrackKind};

fn stage(on: bool, audience_music: bool) -> Stage {
    let mut s = Stage::new("d", on);
    s.hosts.push("a".to_string());
    s.audience_music = audience_music;
    s
}

const TRILHAS: [(TrackKind, AudioSource); 5] = [
    (TrackKind::Screen, AudioSource::Apps),
    (TrackKind::Window, AudioSource::Apps),
    (TrackKind::Camera, AudioSource::Apps),
    (TrackKind::Audio, AudioSource::Apps),
    (TrackKind::Audio, AudioSource::Mic),
];

fn eh_mic(k: TrackKind, s: AudioSource) -> bool {
    k == TrackKind::Audio && s == AudioSource::Mic
}

#[test]
fn matriz_publicar() {
    // Palco desligado: tudo ok, exceto microfone (para todos, inclusive o dono).
    let off = stage(false, false);
    for id in ["d", "a", "p"] {
        for (k, s) in TRILHAS {
            let esperado = if eh_mic(k, s) {
                Err(StageError::MicOutsideStage)
            } else {
                Ok(())
            };
            assert_eq!(off.can_publish(id, k, s), esperado, "{id} {k:?} {s:?}");
        }
    }
    // Palco ligado: anfitriões e dono tudo ok; plateia tudo negado (até microfone).
    let on = stage(true, false);
    for id in ["d", "a"] {
        for (k, s) in TRILHAS {
            assert_eq!(on.can_publish(id, k, s), Ok(()), "{id} {k:?} {s:?}");
        }
    }
    for (k, s) in TRILHAS {
        assert_eq!(
            on.can_publish("p", k, s),
            Err(StageError::AudienceCannotPublish),
            "{k:?} {s:?}"
        );
    }
}

#[test]
fn matriz_musica() {
    use MusicAction::*;
    let acoes = [Add, Remove, Vote, Pause, Seek, Info];
    let ok_todos = [
        ("palco desligado", stage(false, false), "p"),
        ("anfitriao", stage(true, false), "a"),
        ("dono", stage(true, false), "d"),
        ("plateia com chave", stage(true, true), "p"),
    ];
    for (nome, s, id) in &ok_todos {
        for a in acoes {
            assert_eq!(s.check_music(id, a), Ok(()), "{nome} {a:?}");
        }
    }
    let trancado = stage(true, false);
    for a in acoes {
        let esperado = if a == Info {
            Ok(())
        } else {
            Err(StageError::MusicLocked)
        };
        assert_eq!(trancado.check_music("p", a), esperado, "{a:?}");
    }
}

#[test]
fn moderador() {
    let on = stage(true, false);
    let off = stage(false, false);
    assert!(on.music_moderator("d"));
    assert!(off.music_moderator("d"));
    assert!(on.music_moderator("a"));
    assert!(!off.music_moderator("a"));
    assert!(!on.music_moderator("p"));
    assert!(!on.music_moderator("zzz"));
    assert!(!off.music_moderator("zzz"));
}

#[test]
fn votante() {
    let off = stage(false, false);
    for id in ["d", "a", "p"] {
        assert!(off.is_voter(id));
    }
    let trancado = stage(true, false);
    assert!(trancado.is_voter("d"));
    assert!(trancado.is_voter("a"));
    assert!(!trancado.is_voter("p"));
    let aberto = stage(true, true);
    for id in ["d", "a", "p"] {
        assert!(aberto.is_voter(id));
    }
}

#[test]
fn voters_palco_trancado() {
    let plateia: Vec<String> = (0..10).map(|i| format!("p{i}")).collect();
    let mut present: Vec<&str> = vec!["d", "a"];
    present.extend(plateia.iter().map(String::as_str));
    assert_eq!(stage(true, false).voters(present.iter().copied()), 2);
    assert_eq!(stage(true, true).voters(present.iter().copied()), 12);
    assert_eq!(stage(false, false).voters(present.iter().copied()), 12);
}
