//! Testes de ponta a ponta do backend sintético, só pela API pública de `zap-media`.
//! Portáveis: rodam em qualquer plataforma (o pipeline real só existe no Windows).
//! As tolerâncias são folgadas de propósito para CI lento.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use crossbeam_channel::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};
use zap_audio::OpusDec;
use zap_media::{
    AudioPipeline, EncodedAudio, EncodedFrame, EncoderChoiceSpec, VideoPipeline, VideoPipelineConfig, VideoSourceSpec,
};
use zap_proto::LayerSpec;
use zap_rate::allocator::LayerRequest;

const TRACK: u16 = 7;

/// Escada de 3 camadas: 1080p60, 720p30 e 360p30.
fn escada() -> Vec<LayerSpec> {
    vec![
        LayerSpec {
            layer: 0,
            width: 1920,
            height: 1080,
            fps: 60,
            kbps: 8000,
        },
        LayerSpec {
            layer: 1,
            width: 1280,
            height: 720,
            fps: 30,
            kbps: 3000,
        },
        LayerSpec {
            layer: 2,
            width: 640,
            height: 360,
            fps: 30,
            kbps: 800,
        },
    ]
}

fn config(ladder: Vec<LayerSpec>) -> VideoPipelineConfig {
    VideoPipelineConfig {
        track: TRACK,
        source: VideoSourceSpec::Synthetic {
            width: 1920,
            height: 1080,
        },
        ladder,
        encoder: EncoderChoiceSpec::Auto,
    }
}

fn pipeline() -> (VideoPipeline, Receiver<EncodedFrame>) {
    let (tx, rx) = crossbeam_channel::bounded(4096);
    let p = VideoPipeline::start(config(escada()), tx).expect("start do pipeline sintético");
    (p, rx)
}

fn pedido(layer: u8, kbps: u32) -> LayerRequest {
    LayerRequest { layer, kbps }
}

/// Recebe itens até `dur` passar (ou o canal fechar).
fn coletar<T>(rx: &Receiver<T>, dur: Duration) -> Vec<T> {
    let fim = Instant::now() + dur;
    let mut v = Vec::new();
    loop {
        let agora = Instant::now();
        if agora >= fim {
            break;
        }
        match rx.recv_timeout(fim - agora) {
            Ok(x) => v.push(x),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    v
}

/// Descarta o que já está no canal.
fn drenar<T>(rx: &Receiver<T>) {
    while rx.try_recv().is_ok() {}
}

/// `true` se o dado traz o start code seguido do cabeçalho NAL `byte` (0x67 SPS, 0x68 PPS, 0x65 IDR).
fn tem_nal(data: &[u8], byte: u8) -> bool {
    data.windows(5).any(|w| w == [0, 0, 0, 1, byte])
}

#[test]
fn video_so_camadas_pedidas_emitem() {
    let (p, rx) = pipeline();
    p.set_requests(&[pedido(1, 3000)]);
    let quadros = coletar(&rx, Duration::from_secs(1));
    assert!(quadros.len() >= 10, "poucos quadros em 1 s: {}", quadros.len());
    assert_eq!(quadros[0].seq, 0, "contador por camada começa em 0");
    for (i, q) in quadros.iter().enumerate() {
        assert_eq!(q.layer, 1, "quadro de camada não pedida");
        assert_eq!(q.track, TRACK);
        assert_eq!((q.width, q.height), (1280, 720));
        if i > 0 {
            assert_eq!(q.seq, quadros[i - 1].seq + 1, "seq quebrado no quadro {i}");
        }
    }
}

#[test]
fn video_primeiro_quadro_e_keyframe_com_sps_pps() {
    let (p, rx) = pipeline();
    p.set_requests(&[pedido(1, 3000)]);
    let quadros = coletar(&rx, Duration::from_millis(300));
    let primeiro = quadros.first().expect("ao menos um quadro em 300 ms");
    assert!(primeiro.keyframe, "primeiro quadro após ligar a camada deve ser chave");
    assert!(primeiro.has_params);
    assert!(primeiro.data.starts_with(&[0, 0, 0, 1]), "Annex B sem start code");
    assert!(tem_nal(&primeiro.data, 0x67), "falta SPS (NAL 7)");
    assert!(tem_nal(&primeiro.data, 0x68), "falta PPS (NAL 8)");
    assert!(tem_nal(&primeiro.data, 0x65), "falta IDR (NAL 5)");
}

#[test]
fn video_tamanho_segue_bitrate() {
    let (p, rx) = pipeline();
    p.set_requests(&[pedido(2, 800)]);
    let quadros = coletar(&rx, Duration::from_secs(2));
    assert!(!quadros.is_empty());
    let bytes: u64 = quadros.iter().map(|q| q.data.len() as u64).sum();
    let kbps = bytes as f64 * 8.0 / 1000.0 / 2.0;
    assert!(
        (400.0..=1600.0).contains(&kbps),
        "bitrate medido {kbps:.0} kbps fora de 400..1600 (pedido 800)"
    );
}

#[test]
fn video_keyframe_8x_maior() {
    let (p, rx) = pipeline();
    p.set_requests(&[pedido(2, 800)]);
    let quadros = coletar(&rx, Duration::from_secs(1));
    let (chaves, normais): (Vec<&EncodedFrame>, Vec<&EncodedFrame>) = quadros.iter().partition(|q| q.keyframe);
    assert!(!chaves.is_empty(), "sem keyframe");
    assert!(!normais.is_empty(), "sem quadros não-chave");
    let media: f64 = normais.iter().map(|q| q.data.len() as f64).sum::<f64>() / normais.len() as f64;
    for k in &chaves {
        assert!(
            k.data.len() as f64 >= 6.0 * media,
            "keyframe de {} bytes, média dos não-chave {media:.0}",
            k.data.len()
        );
    }
}

#[test]
fn video_request_keyframe_gera_chave() {
    let (p, rx) = pipeline();
    p.set_requests(&[pedido(1, 3000)]);
    // Descarta o início (inclui o keyframe de ligação). O periódico só vem em 10 s.
    let _ = coletar(&rx, Duration::from_millis(300));
    p.request_keyframe(1);
    let depois = coletar(&rx, Duration::from_millis(500));
    assert!(
        depois.iter().any(|q| q.keyframe && q.layer == 1),
        "nenhum keyframe em 500 ms após request_keyframe"
    );
}

#[test]
fn video_desligar_para() {
    let (p, rx) = pipeline();
    p.set_requests(&[pedido(1, 3000)]);
    let antes = coletar(&rx, Duration::from_millis(300));
    assert!(!antes.is_empty(), "camada não emitiu antes de desligar");
    p.set_requests(&[]);
    std::thread::sleep(Duration::from_millis(300));
    // Quadros já em trânsito antes do desligamento ficam para trás.
    drenar(&rx);
    let depois = coletar(&rx, Duration::from_millis(300));
    assert!(depois.is_empty(), "{} quadros chegaram após desligar", depois.len());
}

#[test]
fn video_set_ladder_muda_resolucao() {
    let (p, rx) = pipeline();
    p.set_requests(&[pedido(1, 3000)]);
    let _ = coletar(&rx, Duration::from_millis(300));
    p.set_ladder(vec![LayerSpec {
        layer: 1,
        width: 480,
        height: 270,
        fps: 30,
        kbps: 3000,
    }]);
    // Dá tempo para a thread reconstruir as camadas; o que já saiu com o tamanho antigo some.
    std::thread::sleep(Duration::from_millis(200));
    drenar(&rx);
    let depois = coletar(&rx, Duration::from_millis(500));
    assert!(!depois.is_empty(), "nenhum quadro da camada 1 após set_ladder");
    for q in &depois {
        assert_eq!(q.layer, 1);
        assert_eq!((q.width, q.height), (480, 270), "resolução antiga após set_ladder");
    }
}

#[test]
fn video_stats_mostra_fps() {
    let (p, rx) = pipeline();
    p.set_requests(&[pedido(1, 3000)]);
    // Mantém o canal drenado enquanto espera a janela de estatísticas (~1 s).
    let limite = Instant::now() + Duration::from_secs(4);
    let camada = loop {
        drenar(&rx);
        let st = p.stats();
        if let Some(l) = st.layers.iter().find(|l| l.layer == 1 && l.fps > 0.0) {
            assert!(l.enabled, "camada ligada aparece como desligada");
            assert!(
                (20.0..=40.0).contains(&l.fps),
                "fps medido {} fora de 20..40 (escada 30)",
                l.fps
            );
            assert_eq!(st.encoder_name, "synthetic");
            break l.clone();
        }
        assert!(Instant::now() < limite, "estatísticas da camada 1 não apareceram");
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(camada.frames > 0);
}

#[test]
fn audio_sintetico_50_blocos_decodificaveis() {
    let (tx, rx) = crossbeam_channel::bounded::<EncodedAudio>(256);
    let p = AudioPipeline::start_synthetic(TRACK, tx).expect("start do áudio sintético");
    let blocos = coletar(&rx, Duration::from_secs(1));
    drop(p);
    assert!(
        (40..=60).contains(&blocos.len()),
        "{} blocos em 1 s (esperado perto de 50)",
        blocos.len()
    );
    assert_eq!(blocos[0].seq, 0);
    let mut dec = OpusDec::new().expect("decodificador Opus");
    let mut pcm = [0f32; zap_audio::FRAME_SAMPLES];
    for (i, b) in blocos.iter().enumerate() {
        assert_eq!(b.track, TRACK);
        if i > 0 {
            assert_eq!(b.seq, blocos[i - 1].seq + 1, "seq quebrado no bloco {i}");
        }
        dec.decode(Some(&b.data), None, &mut pcm)
            .unwrap_or_else(|e| panic!("bloco {i} não decodifica: {e}"));
    }
}

#[test]
fn audio_drop_encerra() {
    let (tx, rx) = crossbeam_channel::bounded::<EncodedAudio>(256);
    let p = AudioPipeline::start_synthetic(TRACK, tx).expect("start do áudio sintético");
    let _ = coletar(&rx, Duration::from_millis(100));
    let inicio = Instant::now();
    drop(p);
    assert!(inicio.elapsed() < Duration::from_millis(500), "drop demorou demais");
    // Com a thread encerrada, o canal esvazia e fecha.
    let limite = Instant::now() + Duration::from_secs(1);
    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(_) => {}
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {
                assert!(Instant::now() < limite, "canal de áudio não fechou após drop");
            }
        }
    }
}
