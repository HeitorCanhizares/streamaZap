//! Plano da sessão Online dentro do `Shared` (`docs/planos/fase3-app-planos.md` §4.3):
//! o ponto único de conferência chamado por `ShareSession` e pelas salas Online.
use zap_proto::plan::PlanInfo;
use zap_proto::{LayerSpec, TrackKind};

use crate::engine::Shared;
use crate::plan_gate::{self, PlanRefusal};
use crate::room::ActiveRoom;
use crate::types::Preset;
use crate::CoreError;

// Os chamadores entram nas tarefas core-05 a core-09.
#[allow(dead_code)]
impl Shared {
    /// Plano que limita a sala atual: `Some` só com `ActiveRoom::Online` e sessão com plano.
    pub(crate) fn room_plan(&self) -> Option<PlanInfo> {
        // Pega o `Arc` e solta o lock de `room` antes de olhar a sessão.
        let sala = self.current_room()?;
        match sala.as_ref() {
            ActiveRoom::Online(_) => self.session_plan(),
            // Sala Local: nada é limitado por plano.
            _ => None,
        }
    }

    /// Plano da sessão Online (criar/entrar): `self.online.client()?.plan()`.
    pub(crate) fn session_plan(&self) -> Option<PlanInfo> {
        self.online.client().and_then(|c| c.plan())
    }

    /// Sem `room_plan`: devolve `ladder` como veio. Com plano: `fit_ladder` e `check_layers`;
    /// recusa → `CoreError::Plan`.
    pub(crate) fn plan_ladder(&self, kind: TrackKind, ladder: Vec<LayerSpec>) -> Result<Vec<LayerSpec>, CoreError> {
        let Some(plano) = self.room_plan() else {
            return Ok(ladder);
        };
        let ladder = plan_gate::fit_ladder(kind, ladder, &plano.limits);
        plan_gate::check_layers(kind, &ladder, &plano).map_err(CoreError::Plan)?;
        Ok(ladder)
    }

    /// Preset da câmera que vale agora: sem `room_plan`, o próprio; com plano,
    /// `best_allowed(Camera, …)`.
    pub(crate) fn camera_preset_for_plan(&self, preset: Preset) -> Preset {
        match self.room_plan() {
            Some(plano) => plan_gate::best_allowed(TrackKind::Camera, preset, &plano.limits),
            None => preset,
        }
    }

    /// `state.set_plan_notice(r)` e `notifier.touch()`.
    pub(crate) fn record_plan_refusal(&self, r: &PlanRefusal) {
        self.state.lock().set_plan_notice(r);
        self.notifier.touch();
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use zap_proto::plan::{PlanId, PlanInfo};
    use zap_proto::{RoomMode, TrackKind};
    use zap_test_server::Config;

    use crate::engine::{Core, CoreOptions};
    use crate::plan_gate::{nominal_ladder, PlanAction, PlanRefusal};
    use crate::testutil::TestSink;
    use crate::types::{CreateRoom, Preset};
    use crate::CoreError;

    fn conta_cfg() -> Config {
        Config {
            requires_account: true,
            account: Some(zap_net::online::AccountProvider {
                provider: "supabase".into(),
                url: "https://x.supabase.co".into(),
                key: "k".into(),
            }),
            ..Default::default()
        }
    }

    async fn ate(mut ok: impl FnMut() -> bool) -> bool {
        let fim = Instant::now() + Duration::from_secs(5);
        while Instant::now() < fim {
            if ok() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        ok()
    }

    /// Núcleo apontado para `url`, com a sessão ligada (e o token, se houver).
    async fn novo_core(url: String, token: Option<&str>) -> (Core, std::path::PathBuf) {
        let token = token.map(String::from);
        let (core, d) = tokio::task::spawn_blocking(move || {
            let d = std::env::temp_dir().join(format!("zapcore-plan-{}", uuid::Uuid::new_v4()));
            let opts = CoreOptions {
                discovery: false,
                broadcast: false,
                include_loopback: true,
                audio_out: false,
                online: false,
                ..Default::default()
            };
            (Core::start_with(d.clone(), TestSink::new(), opts).unwrap(), d)
        })
        .await
        .unwrap();
        core.update_settings(|s| {
            s.server.address = url;
            s.server.enabled = true;
            s.server.use_relay = false;
        });
        *core.shared.online.account_token.lock() = token;
        core.shared.online.start(&core.shared);
        assert!(ate(|| core.shared.online.ws().is_some()).await, "sem websocket");
        (core, d)
    }

    async fn encerrar(core: Core, d: std::path::PathBuf) {
        tokio::task::spawn_blocking(move || {
            drop(core);
            let _ = std::fs::remove_dir_all(d);
        })
        .await
        .unwrap();
    }

    fn pedido(modo: RoomMode) -> CreateRoom {
        CreateRoom {
            name: "Sala".into(),
            mode: modo,
            password: None,
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn session_plan_com_e_sem_conta() {
        let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
        let (core, d) = novo_core(srv.base_url.clone(), Some("teste:u1:hd")).await;
        assert_eq!(core.shared.session_plan(), Some(PlanInfo::of(PlanId::Hd)));
        encerrar(core, d).await;

        let srv = zap_test_server::spawn(Config::default()).await.unwrap();
        let (core, d) = novo_core(srv.base_url.clone(), None).await;
        assert_eq!(core.shared.session_plan(), None);
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn room_plan_so_em_sala_online() {
        let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
        let (core, d) = novo_core(srv.base_url.clone(), Some("teste:u1:hd")).await;
        assert_eq!(core.shared.room_plan(), None, "sem sala");
        core.create_room(pedido(RoomMode::Local)).await.unwrap();
        assert!(core.shared.session_plan().is_some());
        assert_eq!(core.shared.room_plan(), None, "sala Local");
        core.leave_room().await;
        core.create_room(pedido(RoomMode::Online)).await.unwrap();
        assert_eq!(core.shared.room_plan(), Some(PlanInfo::of(PlanId::Hd)));
        core.leave_room().await;
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn plan_ladder_confere_o_plano() {
        let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
        let (core, d) = novo_core(srv.base_url.clone(), Some("teste:u1:hd")).await;
        let sh = &core.shared;

        // Sem sala e em sala Local: igual.
        let acima = nominal_ladder(TrackKind::Screen, Preset::P1080p60);
        assert_eq!(sh.plan_ladder(TrackKind::Screen, acima.clone()).unwrap(), acima);
        core.create_room(pedido(RoomMode::Local)).await.unwrap();
        assert_eq!(sh.plan_ladder(TrackKind::Screen, acima.clone()).unwrap(), acima);
        core.leave_room().await;

        core.create_room(pedido(RoomMode::Online)).await.unwrap();
        let r = sh.plan_ladder(TrackKind::Screen, acima);
        assert!(
            matches!(r, Err(CoreError::Plan(ref p)) if p.field.as_deref() == Some("camadas")),
            "{r:?}"
        );
        let ok = sh
            .plan_ladder(TrackKind::Screen, nominal_ladder(TrackKind::Screen, Preset::P720p60))
            .unwrap();
        assert_eq!(ok[0].kbps, 4000);
        core.leave_room().await;
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn plan_ladder_espectador_recusa_audio() {
        let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
        let (core, d) = novo_core(srv.base_url.clone(), Some("teste:u1:espectador")).await;
        core.create_room(pedido(RoomMode::Online)).await.unwrap();
        let r = core.shared.plan_ladder(TrackKind::Audio, vec![]);
        assert!(
            matches!(r, Err(CoreError::Plan(ref p)) if p.field.as_deref() == Some("tipo")),
            "{r:?}"
        );
        core.leave_room().await;
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn camera_preset_respeita_o_plano_so_no_online() {
        let srv = zap_test_server::spawn(conta_cfg()).await.unwrap();
        let (core, d) = novo_core(srv.base_url.clone(), Some("teste:u1:hd")).await;
        let sh = &core.shared;
        core.create_room(pedido(RoomMode::Local)).await.unwrap();
        assert_eq!(sh.camera_preset_for_plan(Preset::P1080p60), Preset::P1080p60);
        core.leave_room().await;
        core.create_room(pedido(RoomMode::Online)).await.unwrap();
        assert_eq!(sh.camera_preset_for_plan(Preset::P1080p60), Preset::P720p60);
        core.leave_room().await;
        encerrar(core, d).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn record_plan_refusal_conta_seq_e_toca_o_notifier() {
        // Sem sessão Online: nenhum outro toque pode mascarar o do `record_plan_refusal`.
        let sink = TestSink::new();
        let sink2 = sink.clone();
        let (core, d) = tokio::task::spawn_blocking(move || {
            let d = std::env::temp_dir().join(format!("zapcore-plan-{}", uuid::Uuid::new_v4()));
            let opts = CoreOptions {
                discovery: false,
                broadcast: false,
                include_loopback: true,
                audio_out: false,
                online: false,
                ..Default::default()
            };
            (Core::start_with(d.clone(), sink2, opts).unwrap(), d)
        })
        .await
        .unwrap();
        let r = PlanRefusal {
            plan: PlanId::Hd,
            action: PlanAction::Publish,
            field: Some("camadas".into()),
            message: "Seu plano não permite essa qualidade".into(),
        };
        // O tick de 500 ms do motor também toca o notifier e mascararia o do aviso:
        // para o tick e espera ele sair antes de registrar a recusa.
        core.shared.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        tokio::time::sleep(Duration::from_millis(1200)).await;
        core.shared.record_plan_refusal(&r);
        core.shared.record_plan_refusal(&r);
        // A entrega só acontece pelo notifier (tick), não pelo `snapshot()` direto.
        let chegou = sink
            .wait_for(Duration::from_secs(3), |s| {
                s.snapshots
                    .lock()
                    .iter()
                    .any(|x| x.plan_notice.as_ref().map(|n| n.seq) == Some(2))
            })
            .await;
        assert!(chegou, "o notifier não entregou o aviso");
        let snap = sink.last_snapshot().unwrap();
        assert_eq!(
            snap.plan_notice.as_ref().map(|n| n.field.clone()),
            Some(r.field.clone())
        );
        encerrar(core, d).await;
    }
}
