//! A cada 100 ms: decide camadas por espectador e o que o pipeline codifica.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use zap_proto::DelayState;
use zap_rate::allocator::{Budget, LayerRequest};

use super::video::VideoShared;
use super::viewer::LayerInfo;

/// Estimativa de upload (kbps): se algum espectador está em `DelayState::Overuse`, a soma das
/// larguras de banda entregues (`bandwidth_kbps`) de todos os espectadores; senão 50_000.
/// Mínimo 500.
pub fn estimate_upload_kbps(viewers: &[zap_rate::allocator::ViewerInput]) -> u32 {
    let sobrecarga = viewers.iter().any(|v| v.state == DelayState::Overuse);
    let bruto = if sobrecarga {
        // Soma saturada: banda declarada pelo cliente não é confiável para estourar u32.
        viewers.iter().fold(0u32, |acc, v| acc.saturating_add(v.bandwidth_kbps))
    } else {
        50_000
    };
    bruto.max(500)
}

/// União de dois pedidos: para a mesma camada vale o maior kbps; ordenado por camada.
pub(crate) fn merge_requests(a: &[LayerRequest], b: &[LayerRequest]) -> Vec<LayerRequest> {
    let mut out: Vec<LayerRequest> = Vec::new();
    for r in a.iter().chain(b.iter()) {
        match out.iter_mut().find(|o| o.layer == r.layer) {
            Some(o) => o.kbps = o.kbps.max(r.kbps),
            None => out.push(r.clone()),
        }
    }
    out.sort_by_key(|r| r.layer);
    out
}

/// Um tick: monta `ViewerInput` de cada `ViewerLink` (`state.input(id, 0)`), chama
/// `allocator.update(now_us, &inputs, Budget::Direct{upload_kbps})`, aplica
/// `pipeline.set_requests(&out.requests)` e `request_keyframe` para cada camada de
/// `out.keyframes`; cada espectador recebe `apply_decision` e, se a camada atual mudou, uma
/// tarefa `announce_layer(.., "banda")` (via `rt.spawn`). Guarda o upload em `shared.upload_kbps`.
pub fn run_alloc_tick(shared: &Arc<VideoShared>, rt: &tokio::runtime::Handle) {
    // Cópia curta: não segura o lock da lista durante a decisão.
    // Links de ponte ficam fora do alocador e da estimativa de upload.
    let viewers: Vec<_> = shared
        .viewers
        .lock()
        .iter()
        .filter(|v| !v.state.lock().bridge)
        .cloned()
        .collect();
    let inputs: Vec<_> = viewers.iter().map(|v| v.state.lock().input(v.id, 0)).collect();

    let upload = estimate_upload_kbps(&inputs);
    shared.upload_kbps.store(upload, Ordering::Relaxed);

    let now = shared.clock.now_us();
    let out = shared
        .allocator
        .lock()
        .update(now, &inputs, Budget::Direct { upload_kbps: upload });

    // O pipeline ignora pedidos iguais; sem espectadores `requests` vem vazio e desliga tudo.
    let ponte = shared.bridge_requests.lock().clone();
    shared.pipeline.set_requests(&merge_requests(&out.requests, &ponte));
    for l in &out.keyframes {
        shared.pipeline.request_keyframe(*l);
    }

    for d in &out.viewers {
        let Some(v) = viewers.iter().find(|v| v.id == d.id) else {
            continue;
        };
        let mudou = v.state.lock().apply_decision(d);
        if !mudou {
            continue;
        }
        let info = {
            let escada = shared.ladder.lock();
            match escada.iter().find(|l| l.layer == d.current) {
                Some(l) => LayerInfo::from_spec(l),
                None => LayerInfo {
                    layer: d.current,
                    width: 0,
                    height: 0,
                    fps: 0,
                },
            }
        };
        let v = v.clone();
        rt.spawn(async move {
            v.announce_layer(&info, "banda").await;
        });
    }
}

/// Tarefa: `interval(100 ms)` chamando `run_alloc_tick` até `shared.stop`.
pub(crate) async fn run(shared: Arc<VideoShared>, rt: tokio::runtime::Handle) {
    let mut ticker = tokio::time::interval(Duration::from_millis(100));
    // Tick atrasado não deve disparar rajada de recuperação.
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    while !shared.stop.load(Ordering::Relaxed) {
        ticker.tick().await;
        if shared.stop.load(Ordering::Relaxed) {
            break;
        }
        run_alloc_tick(&shared, &rt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicU32};
    use std::time::Instant;

    use parking_lot::Mutex;
    use zap_media::{EncoderChoiceSpec, VideoPipeline, VideoPipelineConfig, VideoSourceSpec};
    use zap_net::{
        control::{ControlRx, ControlTx},
        endpoint,
        media_tx::MediaSender,
        tls::{self, Identity},
        Clock,
    };
    use zap_proto::{ControlMsg, LayerSpec};
    use zap_rate::allocator::{Allocator, ViewerInput};

    use crate::publish::viewer::{ViewerLink, ViewerState};

    type Res = Result<(), Box<dyn std::error::Error>>;

    fn vi(id: u64, banda: u32, state: DelayState) -> ViewerInput {
        ViewerInput {
            id,
            bandwidth_kbps: banda,
            state,
            max_layer: 0,
            reserved_kbps: 0,
        }
    }

    #[test]
    fn upload_sem_sobrecarga_50mbps() {
        let v = [vi(1, 1000, DelayState::Normal), vi(2, 300, DelayState::Underuse)];
        assert_eq!(estimate_upload_kbps(&v), 50_000);
        assert_eq!(estimate_upload_kbps(&[]), 50_000);
    }

    #[test]
    fn upload_com_overuse_soma_bandas() {
        let v = [vi(1, 1000, DelayState::Overuse), vi(2, 3000, DelayState::Normal)];
        assert_eq!(estimate_upload_kbps(&v), 4000);
    }

    #[test]
    fn upload_minimo_500() {
        let v = [vi(1, 100, DelayState::Overuse)];
        assert_eq!(estimate_upload_kbps(&v), 500);
    }

    fn escada() -> Vec<LayerSpec> {
        vec![
            LayerSpec {
                layer: 0,
                width: 1280,
                height: 720,
                fps: 30,
                kbps: 2500,
            },
            LayerSpec {
                layer: 1,
                width: 960,
                height: 540,
                fps: 30,
                kbps: 1200,
            },
            LayerSpec {
                layer: 2,
                width: 640,
                height: 360,
                fps: 15,
                kbps: 500,
            },
        ]
    }

    fn shared() -> Result<Arc<VideoShared>, Box<dyn std::error::Error>> {
        let (tx, rx) = crossbeam_channel::bounded(64);
        // Escoa os quadros para o pipeline nunca travar na fila.
        std::thread::spawn(move || while rx.recv().is_ok() {});
        let cfg = VideoPipelineConfig {
            track: 1,
            source: VideoSourceSpec::Synthetic {
                width: 640,
                height: 360,
            },
            ladder: escada(),
            encoder: EncoderChoiceSpec::Auto,
        };
        let pipeline = VideoPipeline::start(cfg, tx)?;
        Ok(Arc::new(VideoShared {
            track: 1,
            pipeline,
            allocator: Mutex::new(Allocator::new(escada())),
            viewers: Mutex::new(Vec::new()),
            ladder: Mutex::new(escada()),
            stop: AtomicBool::new(false),
            upload_kbps: AtomicU32::new(0),
            clock: Clock::new(),
            bridge_requests: Mutex::new(Vec::new()),
        }))
    }

    struct Par {
        cli: quinn::Connection,
        srv: quinn::Connection,
        _eps: (quinn::Endpoint, quinn::Endpoint),
    }

    async fn par() -> Result<Par, Box<dyn std::error::Error>> {
        let id = Identity::generate("zap")?;
        let (servidor, porta) = endpoint::bind_server(&id, 0).await?;
        let addr: std::net::SocketAddr = format!("127.0.0.1:{porta}").parse()?;
        let cliente = endpoint::client_endpoint()?;
        let conectando = cliente.connect_with(tls::client_config(Some(&id.fingerprint()))?, addr, "zap")?;
        let inc = servidor.accept().await.ok_or("sem conexão")?;
        let srv = inc.await?;
        let cli = conectando.await?;
        Ok(Par {
            cli,
            srv,
            _eps: (servidor, cliente),
        })
    }

    async fn espectador(p: &Par) -> Result<(Arc<ViewerLink>, ControlRx), Box<dyn std::error::Error>> {
        let (send, _recv_a) = p.cli.open_bi().await?;
        let link = ViewerLink::new(
            1,
            "x".into(),
            "t_1".into(),
            Arc::new(MediaSender::new(p.cli.clone(), Clock::new())),
            ControlTx::new(send),
            2,
        );
        // O lado B só enxerga o stream depois de chegarem dados.
        link.announce_layer(
            &LayerInfo {
                layer: 2,
                width: 640,
                height: 360,
                fps: 15,
            },
            "inicio",
        )
        .await;
        let (_s, recv_b) = p.srv.accept_bi().await?;
        let mut rx = ControlRx::new(recv_b);
        rx.recv().await?.ok_or("stream fechou")?;
        Ok((link, rx))
    }

    /// Adianta o alocador com tempos sintéticos: a histerese exige 3 s para subir de camada,
    /// mais do que a janela dos testes. Devolve a camada pendente (alvo da subida).
    fn adiantar(sh: &VideoShared, link: &ViewerLink) -> Option<u8> {
        let ins = vec![link.state.lock().input(link.id, 0)];
        let b = Budget::Direct { upload_kbps: 50_000 };
        let mut al = sh.allocator.lock();
        al.update(0, &ins, b);
        let out = al.update(3_100_000, &ins, b);
        out.viewers.first().and_then(|d| d.pending)
    }

    fn rq(layer: u8, kbps: u32) -> LayerRequest {
        LayerRequest { layer, kbps }
    }

    #[test]
    fn merge_requests_pega_maior_kbps_e_ordena() {
        let m = merge_requests(&[rq(2, 100), rq(0, 900)], &[rq(2, 300), rq(1, 50)]);
        assert_eq!(m, vec![rq(0, 900), rq(1, 50), rq(2, 300)]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tick_ignora_viewers_de_ponte() -> Res {
        let rt = tokio::runtime::Handle::current();
        let p = par().await?;
        // Referência: só o espectador direto.
        let sh_ref = shared()?;
        let (direto_ref, _rx_ref) = espectador(&p).await?;
        direto_ref.state.lock().bandwidth_kbps = 20_000;
        sh_ref.viewers.lock().push(direto_ref.clone());

        // Mesmo espectador direto + um link de ponte com banda mínima em sobrecarga.
        let sh = shared()?;
        let (direto, _rx) = espectador(&p).await?;
        direto.state.lock().bandwidth_kbps = 20_000;
        let (send, _recv) = p.cli.open_bi().await?;
        let ponte = ViewerLink::new(
            7,
            "ponte".into(),
            "t_1".into(),
            Arc::new(MediaSender::new(p.cli.clone(), Clock::new())),
            ControlTx::new(send),
            2,
        );
        {
            let mut st = ponte.state.lock();
            *st = ViewerState::new_bridge();
            st.set_bridge_layers(vec![0, 1, 2]);
            st.bandwidth_kbps = 1;
            st.delay = DelayState::Overuse;
        }
        sh.viewers.lock().push(direto.clone());
        sh.viewers.lock().push(ponte.clone());

        run_alloc_tick(&sh_ref, &rt);
        run_alloc_tick(&sh, &rt);

        // Contado, o link de ponte faria o upload cair para ~20 Mbps (soma em sobrecarga).
        assert_eq!(sh.upload_kbps.load(Ordering::Relaxed), 50_000);
        assert_eq!(
            sh.upload_kbps.load(Ordering::Relaxed),
            sh_ref.upload_kbps.load(Ordering::Relaxed)
        );
        // A decisão do espectador direto é a mesma de quando a ponte não existe.
        {
            let a = direto.state.lock();
            let b = direto_ref.state.lock();
            assert_eq!((a.current, a.pending), (b.current, b.pending));
        }
        // O link de ponte não recebe `apply_decision`.
        let st = ponte.state.lock();
        assert_eq!((st.current, st.pending), (0, None));
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tick_une_pedidos_da_ponte() -> Res {
        let sh = shared()?;
        let rt = tokio::runtime::Handle::current();
        *sh.bridge_requests.lock() = vec![rq(2, 300)];
        // As estatísticas do pipeline saem em janelas de 1 s: espera a camada ligar.
        let ini = Instant::now();
        let mut ligadas = Vec::new();
        while ini.elapsed() < Duration::from_secs(3) {
            run_alloc_tick(&sh, &rt);
            tokio::time::sleep(Duration::from_millis(100)).await;
            ligadas = sh
                .pipeline
                .stats()
                .layers
                .iter()
                .filter(|l| l.enabled)
                .map(|l| l.layer)
                .collect::<Vec<_>>();
            if !ligadas.is_empty() {
                break;
            }
        }
        assert_eq!(ligadas, vec![2], "só a camada pedida pela ponte deveria ligar");
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tick_sem_espectadores_nao_liga_camadas() -> Res {
        let sh = shared()?;
        let rt = tokio::runtime::Handle::current();
        for _ in 0..3 {
            run_alloc_tick(&sh, &rt);
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(sh.pipeline.stats().layers.iter().all(|l| !l.enabled));
        assert_eq!(sh.upload_kbps.load(Ordering::Relaxed), 50_000);
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tick_com_espectador_pede_camada_e_pipeline_liga() -> Res {
        let sh = shared()?;
        let rt = tokio::runtime::Handle::current();
        let p = par().await?;
        let (link, _rx) = espectador(&p).await?;
        link.state.lock().bandwidth_kbps = 20_000;
        let pendente = adiantar(&sh, &link);
        assert!(matches!(pendente, Some(0 | 1)), "alvo de subida esperado");
        sh.viewers.lock().push(link);

        let ini = Instant::now();
        let mut ligou = false;
        while ini.elapsed() < Duration::from_secs(1) {
            run_alloc_tick(&sh, &rt);
            tokio::time::sleep(Duration::from_millis(100)).await;
            if sh.pipeline.stats().layers.iter().any(|l| l.enabled && l.layer <= 1) {
                ligou = true;
                break;
            }
        }
        assert!(ligou, "camada 0 ou 1 deveria estar ligada");
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tick_muda_current_e_anuncia() -> Res {
        let sh = shared()?;
        let rt = tokio::runtime::Handle::current();
        let p = par().await?;
        let (link, mut rx) = espectador(&p).await?;
        link.state.lock().bandwidth_kbps = 20_000;
        let pendente = adiantar(&sh, &link).ok_or("sem camada pendente")?;
        // O keyframe da camada pendente chegou: o alocador passa a `current = pendente`.
        sh.allocator.lock().on_keyframe(pendente);
        sh.viewers.lock().push(link.clone());

        // Roda ticks até o estado do espectador trocar a camada atual; o anúncio chega ao par.
        let anuncio = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                run_alloc_tick(&sh, &rt);
                if link.state.lock().current != 2 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            rx.recv().await
        })
        .await??
        .ok_or("stream fechou")?;
        match anuncio {
            ControlMsg::LayerCurrent {
                track_id,
                layer,
                reason,
                ..
            } => {
                assert_eq!(track_id, "t_1");
                assert_eq!(layer, link.state.lock().current);
                assert_eq!(reason, "banda");
            }
            outra => return Err(format!("mensagem inesperada: {outra:?}").into()),
        }
        Ok(())
    }
}
