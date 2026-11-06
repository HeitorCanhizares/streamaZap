//! Tarefa periódica (100 ms) que roda o alocador de cada trilha e age sobre o resultado.
//!
//! Tudo o que sai daqui é mensagem da mídia, então vai pelo stream de controle QUIC
//! (`Inner::send_quic`), como no servidor de produção (`docs/api/mensagens.md`, "Canal de cada
//! mensagem").

use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use tokio::time::{interval, MissedTickBehavior};
use zap_proto::{control::LayerRequestMsg, ControlMsg, LayerSpec, TrackKind};

use crate::{relay::RelayEvent, state::Hub, util::now_unix_ms};

/// Período do alocador.
const PERIODO: Duration = Duration::from_millis(100);
/// Salas vazias somem depois de 2 min.
const SALA_VAZIA_MS: u64 = 120_000;
/// A limpeza de salas vazias roda no máximo 1 vez por segundo.
const INTERVALO_LIMPEZA_MS: u64 = 1_000;

/// Eventos de uma trilha, coletados com o empréstimo mutável da sala e enviados depois.
struct Coleta {
    track_id: String,
    publisher: String,
    layers: Vec<LayerSpec>,
    events: Vec<RelayEvent>,
}

impl Hub {
    /// Um passo: para cada trilha de cada sala, `TrackRelay::tick(clock.now_us())` e então
    /// - `LayerChanged` → `camada.atual {trilha_id, camada, largura, altura, fps, motivo}` ao
    ///   espectador (largura/altura/fps da `LayerSpec` da camada);
    /// - `LayersRequested` → `camadas.pedidas {trilha_id, camadas:[{camada,kbps}]}` ao publicador;
    /// - `KeyframeNeeded(c)` → `keyframe.pedir {trilha_id, camada: c}` ao publicador.
    ///
    /// Também chama `sweep(120_000)` uma vez por segundo.
    ///
    /// Depois disso alimenta o `Prober` de cada participante com a necessidade de sondagem
    /// (`probe::need_for`) e, se o planejador mandar, dispara a rajada numa tarefa tokio,
    /// fora do lock.
    pub fn tick_all(&self) {
        let mut rajadas = Vec::new();
        {
            let mut guard = self.inner.lock();
            let inner = &mut *guard;
            let now_us = self.clock.now_us();
            let mut coletas = Vec::new();
            for sala in inner.rooms.values_mut() {
                for trilha in sala.tracks.iter_mut().filter(|t| t.kind != TrackKind::Audio) {
                    let events = trilha.relay.tick(now_us);
                    if events.is_empty() {
                        continue;
                    }
                    coletas.push(Coleta {
                        track_id: trilha.id.clone(),
                        publisher: trilha.participant_id.clone(),
                        layers: trilha.layers.clone(),
                        events,
                    });
                }
            }
            for c in coletas {
                enviar_eventos(inner, c);
            }
            // Sondagem: `participants` e `rooms` são campos disjuntos, então o empréstimo vale.
            for p in inner.participants.values() {
                let (Some(prober), Some(room_id)) = (&p.prober, &p.room_id) else {
                    continue;
                };
                let Some(sala) = inner.rooms.get(room_id) else {
                    continue;
                };
                for t in sala.tracks.iter().filter(|t| t.kind != TrackKind::Audio) {
                    prober.set_need(t.number, crate::probe::need_for(t, p.num));
                }
                if let Some(b) = prober.poll(now_us, crate::probe::bandwidth_of(sala, p.num)) {
                    rajadas.push((prober.clone(), b));
                }
            }
        }

        // Fora do lock: a rajada é assíncrona. Sem prober não há rajada, então `spawn` nunca
        // roda nos testes síncronos.
        for (prober, b) in rajadas {
            tokio::spawn(async move {
                let _ = prober.run_burst(b).await;
            });
        }

        // Fora do lock: `sweep` trava sozinho.
        static ULTIMA_LIMPEZA_MS: AtomicU64 = AtomicU64::new(0);
        let agora = now_unix_ms();
        let ultima = ULTIMA_LIMPEZA_MS.load(Ordering::Relaxed);
        if agora.saturating_sub(ultima) >= INTERVALO_LIMPEZA_MS
            && ULTIMA_LIMPEZA_MS
                .compare_exchange(ultima, agora, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
        {
            self.sweep(SALA_VAZIA_MS);
            self.tick_music();
        }
    }
}

/// Transforma os eventos de uma trilha em mensagens pelo QUIC de quem deve recebê-las.
fn enviar_eventos(inner: &crate::state::Inner, c: Coleta) {
    for e in c.events {
        match e {
            RelayEvent::LayerChanged { viewer, layer, reason } => {
                let Some(espectador) = inner.participants.values().find(|p| p.num == viewer) else {
                    continue;
                };
                let Some(spec) = c.layers.iter().find(|l| l.layer == layer) else {
                    continue;
                };
                inner.send_quic(
                    &espectador.id,
                    ControlMsg::LayerCurrent {
                        track_id: c.track_id.clone(),
                        layer,
                        width: spec.width,
                        height: spec.height,
                        fps: spec.fps,
                        reason: reason.to_string(),
                    },
                );
            }
            RelayEvent::LayersRequested(pedidos) => inner.send_quic(
                &c.publisher,
                ControlMsg::LayersRequested {
                    track_id: c.track_id.clone(),
                    layers: pedidos
                        .iter()
                        .map(|r| LayerRequestMsg {
                            layer: r.layer,
                            kbps: r.kbps,
                        })
                        .collect(),
                },
            ),
            RelayEvent::KeyframeNeeded(camada) => inner.send_quic(
                &c.publisher,
                ControlMsg::KeyframeRequest {
                    track_id: c.track_id.clone(),
                    layer: Some(camada),
                },
            ),
        }
    }
}

/// Laço: `tokio::time::interval(100 ms)`, chamando `hub.tick_all()` até ser abortado.
pub async fn run(hub: Arc<Hub>) {
    let mut periodo = interval(PERIODO);
    periodo.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        periodo.tick().await;
        hub.tick_all();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{
        relay::{TrackRelay, ViewerEntry},
        state::TrackData,
        Config,
    };
    use tokio::sync::mpsc;
    use zap_net::online::{CreateRoomRequest, LoginRequest};
    use zap_proto::{Access, DelayState, TileSize};

    type Rx = mpsc::UnboundedReceiver<ControlMsg>;

    fn entrar(hub: &Hub, nome: &str) -> String {
        let pedido = LoginRequest {
            name: nome.into(),
            color: "#3366ff".into(),
            install_id: format!("inst-{nome}"),
            protocol: 4,
            app_version: "3.0.0".into(),
            server_password: None,
            account_token: None,
        };
        hub.login(pedido, "203.0.113.7").unwrap().participant_id
    }

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
                kbps: 2500,
            },
            LayerSpec {
                layer: 2,
                width: 640,
                height: 360,
                fps: 15,
                kbps: 400,
            },
        ]
    }

    fn drenar(rx: &mut Rx) -> Vec<ControlMsg> {
        let mut v = Vec::new();
        while let Ok(m) = rx.try_recv() {
            v.push(m);
        }
        v
    }

    /// Ana publica uma tela de 3 camadas; Beto assiste. Devolve os canais QUIC e o WS de Beto.
    fn cenario() -> (Hub, Rx, Rx, Rx) {
        let hub = Hub::new(Config::default());
        let ana = entrar(&hub, "Ana");
        let beto = entrar(&hub, "Beto");
        let sala = hub
            .create_room(
                &ana,
                CreateRoomRequest {
                    name: "Sala".into(),
                    access: Access::Open,
                    password: None,
                },
            )
            .unwrap()
            .id;
        let (qa, rqa) = mpsc::unbounded_channel();
        let (qb, rqb) = mpsc::unbounded_channel();
        let (wb, rwb) = mpsc::unbounded_channel();
        {
            let mut inner = hub.inner.lock();
            inner.participants.get_mut(&ana).unwrap().quic_ctl = Some(qa);
            let b = inner.participants.get_mut(&beto).unwrap();
            b.quic_ctl = Some(qb);
            b.ws = Some(wb);
            let num = b.num;
            let mut relay = TrackRelay::new(escada());
            relay.subscribe(ViewerEntry {
                participant_id: beto.clone(),
                num,
                size: TileSize::Main,
                max_layer_req: None,
                audio: false,
                preview: false,
                bandwidth_kbps: 0,
                state: DelayState::Normal,
                current: 0,
                pending: None,
                announced: None,
            });
            inner.rooms.get_mut(&sala).unwrap().tracks.push(TrackData {
                id: "t_tela".into(),
                number: 1,
                participant_id: ana.clone(),
                kind: TrackKind::Screen,
                title: "Tela".into(),
                layers: escada(),
                audio: None,
                relay,
                camera_layout: None,
            });
        }
        (hub, rqa, rqb, rwb)
    }

    #[test]
    fn tick_anuncia_camada_inicial() {
        let (hub, _rqa, mut rqb, mut rwb) = cenario();
        hub.tick_all();
        let msgs = drenar(&mut rqb);
        assert!(
            msgs.iter().any(|m| matches!(
                m,
                ControlMsg::LayerCurrent { layer: 2, reason, width: 640, height: 360, .. } if reason == "inicio"
            )),
            "{msgs:?}"
        );
        assert!(drenar(&mut rwb).is_empty(), "mensagem da mídia não vai pelo WebSocket");
    }

    #[test]
    fn tick_pede_camadas_ao_publicador() {
        let (hub, mut rqa, _rqb, _rwb) = cenario();
        hub.tick_all();
        let msgs = drenar(&mut rqa);
        assert!(
            msgs.iter().any(|m| matches!(
                m,
                ControlMsg::LayersRequested { track_id, layers } if track_id == "t_tela" && layers.iter().any(|l| l.layer == 2)
            )),
            "{msgs:?}"
        );
        // O mesmo pedido não se repete no tick seguinte.
        hub.tick_all();
        assert!(!drenar(&mut rqa)
            .iter()
            .any(|m| matches!(m, ControlMsg::LayersRequested { .. })));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tick_com_espectador_acima_do_teto_sem_prober_nao_entra_em_panico() {
        let (hub, _rqa, _rqb, _rwb) = cenario();
        {
            let mut inner = hub.inner.lock();
            for sala in inner.rooms.values_mut() {
                for t in sala.tracks.iter_mut() {
                    for v in t.relay.viewers.values_mut() {
                        v.current = 2;
                        v.size = TileSize::Main;
                    }
                }
            }
        }
        for _ in 0..5 {
            hub.tick_all();
        }
    }

    #[test]
    fn sala_vazia_e_sem_trilhas_nao_envia_nada() {
        let hub = Hub::new(Config::default());
        hub.tick_all();
        let ana = entrar(&hub, "Ana");
        hub.create_room(
            &ana,
            CreateRoomRequest {
                name: "Sala".into(),
                access: Access::Open,
                password: None,
            },
        )
        .unwrap();
        let (qa, mut rqa) = mpsc::unbounded_channel();
        hub.inner.lock().participants.get_mut(&ana).unwrap().quic_ctl = Some(qa);
        hub.tick_all();
        assert!(drenar(&mut rqa).is_empty());
    }
}
