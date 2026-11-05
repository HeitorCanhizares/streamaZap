//! Prévia da câmera nas configurações: abre a câmera escolhida só para a interface (sem sala nem
//! rede) e entrega os quadros ao tile, como a prévia local da sala. Se a câmera já está ligada na
//! sala, usa a prévia local dela em vez de abrir o dispositivo de novo (a câmera é exclusiva).

use std::sync::Arc;

use zap_media::{EncodedFrame, VideoPipeline, VideoPipelineConfig};
use zap_proto::LayerSpec;
use zap_rate::allocator::LayerRequest;

use crate::engine::{Core, Shared};
use crate::share::camera_source;
use crate::sources::to_video_spec;
use crate::types::{TileId, VideoPacket};
use crate::CoreError;

/// Camada única da prévia: leve, mas dá para ver o enquadramento.
const CAMADA: LayerSpec = LayerSpec {
    layer: 0,
    width: 640,
    height: 360,
    fps: 30,
    kbps: 1200,
};

/// Prévia aberta num tile.
pub(crate) enum CameraTest {
    /// Câmera aberta só para a prévia; o leitor termina quando o pipeline cai.
    Own {
        pipeline: VideoPipeline,
        reader: std::thread::JoinHandle<()>,
    },
    /// A câmera da sala já estava ligada: a prévia local dela entrega no tile.
    Room,
}

fn pacote(f: &EncodedFrame) -> VideoPacket {
    VideoPacket {
        track: f.track,
        layer: f.layer,
        keyframe: f.keyframe,
        width: f.width,
        height: f.height,
        seq: f.seq,
        capture_us: f.capture_us,
        data: f.data.clone(),
    }
}

/// Encerra uma prévia. `Own` espera o pipeline parar (bloqueia: chame fora do runtime).
fn encerrar(shared: &Arc<Shared>, tile: TileId, previa: CameraTest) {
    match previa {
        CameraTest::Own { pipeline, reader } => {
            // O pipeline fecha o canal ao cair; o leitor sai do `recv`.
            drop(pipeline);
            if reader.join().is_err() {
                tracing::warn!("leitor da prévia da câmera terminou com pânico");
            }
        }
        CameraTest::Room => {
            if let Ok(sessao) = shared.share.try_lock() {
                if let Some(c) = sessao.camera.as_ref() {
                    c.publisher.clear_local_preview(tile);
                }
            }
        }
    }
    shared.playout.remove_tile(tile);
}

impl Core {
    /// Abre a prévia da câmera `camera_id` (o id da lista de fontes; `None` = a padrão) no `tile`.
    /// Já havia prévia nesse tile: troca. Câmera ligada na sala: mostra a dela.
    pub async fn camera_preview(&self, camera_id: Option<String>, tile: TileId) -> Result<(), CoreError> {
        let shared = self.shared.clone();
        let rt = shared.rt.clone();
        rt.spawn(async move { abrir(&shared, camera_id, tile).await })
            .await
            .map_err(|_| CoreError::Stopped)?
    }

    /// Fecha a prévia da câmera do `tile` (sem prévia: nada).
    pub fn camera_preview_stop(&self, tile: TileId) {
        let shared = self.shared.clone();
        let anterior = shared.camera_tests.lock().remove(&tile);
        if let Some(p) = anterior {
            // Parar o pipeline espera a thread dele: fora do runtime.
            std::thread::spawn(move || encerrar(&shared, tile, p));
        }
    }
}

async fn abrir(shared: &Arc<Shared>, camera_id: Option<String>, tile: TileId) -> Result<(), CoreError> {
    // Solta a trava antes do `await`.
    let anterior = shared.camera_tests.lock().remove(&tile);
    if let Some(p) = anterior {
        let sh = shared.clone();
        let _ = tokio::task::spawn_blocking(move || encerrar(&sh, tile, p)).await;
    }
    // Câmera já ligada na sala: a prévia local dela (o dispositivo não abre duas vezes).
    {
        let sessao = shared.share.lock().await;
        if let Some(c) = sessao.camera.as_ref() {
            c.publisher.set_local_preview(tile, shared.playout.clone());
            shared.camera_tests.lock().insert(tile, CameraTest::Room);
            return Ok(());
        }
    }
    let src = camera_source(camera_id)?;
    let encoder = shared.settings.read().video.encoder.to_spec();
    let (tx, rx) = crossbeam_channel::bounded::<EncodedFrame>(16);
    let cfg = VideoPipelineConfig {
        track: 0,
        source: to_video_spec(&src),
        ladder: vec![CAMADA],
        encoder,
    };
    // Abrir a câmera pode demorar (o driver liga o sensor): fora do runtime.
    let pipeline = tokio::task::spawn_blocking(move || VideoPipeline::start(cfg, tx))
        .await
        .map_err(|_| CoreError::Stopped)??;
    pipeline.set_requests(&[LayerRequest {
        layer: CAMADA.layer,
        kbps: CAMADA.kbps,
    }]);
    let saida = shared.playout.clone();
    let reader = std::thread::Builder::new()
        .name("zap-previa-camera".into())
        .spawn(move || {
            while let Ok(f) = rx.recv() {
                saida.push_now(tile, pacote(&f));
            }
        })
        .map_err(|e| CoreError::Unsupported(format!("não deu para abrir a prévia da câmera: {e}")))?;
    let anterior = shared
        .camera_tests
        .lock()
        .insert(tile, CameraTest::Own { pipeline, reader });
    if let Some(p) = anterior {
        // Outra chamada abriu no mesmo tile enquanto esta abria: fica a mais nova.
        let sh = shared.clone();
        std::thread::spawn(move || encerrar(&sh, tile, p));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::CoreOptions;
    use crate::testutil::TestSink;
    use std::time::Duration;

    #[tokio::test(flavor = "multi_thread")]
    async fn previa_da_camera_sintetica_entrega_e_para() {
        let d = std::env::temp_dir().join(format!("zapcore-camtest-{}", uuid::Uuid::new_v4()));
        let sink = TestSink::new();
        let opts = CoreOptions {
            discovery: false,
            broadcast: false,
            include_loopback: true,
            audio_out: false,
            ..Default::default()
        };
        let core = Core::start_with(d.clone(), sink.clone(), opts).expect("core");
        const TILE: TileId = 9100;
        core.camera_preview(Some("synthetic:640x360".into()), TILE)
            .await
            .expect("prévia");
        let chegou = sink
            .wait_for(Duration::from_secs(5), |s| s.video_count(TILE) >= 3)
            .await;
        assert!(chegou, "a prévia da câmera não entregou quadros");
        {
            let videos = sink.videos.lock();
            let primeiro = videos.iter().find(|(_, t, _)| *t == TILE).expect("quadro");
            assert!(primeiro.2.keyframe, "a prévia tem que começar num keyframe");
        }
        core.camera_preview_stop(TILE);
        tokio::time::sleep(Duration::from_millis(500)).await;
        let antes = sink.video_count(TILE);
        tokio::time::sleep(Duration::from_millis(400)).await;
        assert_eq!(sink.video_count(TILE), antes, "parar devia cortar a prévia");
        assert!(core.shared.camera_tests.lock().is_empty());
        let _ = tokio::task::spawn_blocking(move || drop(core)).await;
        let _ = std::fs::remove_dir_all(d);
    }
}
