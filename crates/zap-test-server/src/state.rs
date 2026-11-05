//! Estado em memória do servidor (`Hub`) e suas sessões.
//!
//! Um único `Mutex<Inner>`: simples e suficiente para testes. Nada aqui faz I/O; quem precisa
//! enviar mensagens usa os canais registrados (`ParticipantData::ws`).

use std::{
    collections::{HashMap, VecDeque},
    sync::OnceLock,
};

use parking_lot::Mutex;
use tokio::sync::mpsc;
use zap_net::online::{LoginRequest, SessionResponse};
use zap_proto::{Access, AudioSpec, ControlMsg, LayerSpec, RoomCode, TrackKind};

use crate::{
    relay::TrackRelay,
    util::{iso_from_unix_ms, new_id, new_token, now_iso, now_unix_ms},
    Config,
};

/// Erro de regra de negócio, já com status HTTP e código de `docs/api/README.md#erros`.
/// Vira resposta REST (`rest.rs`) ou mensagem `erro` (WebSocket).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HubError {
    pub status: u16,
    pub code: &'static str,
    pub message: String,
}

impl HubError {
    pub fn new(status: u16, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
}

/// Canal por onde chegou um pedido C→S. A resposta direta (`erro`, `midia.rota`…) volta pelo
/// mesmo canal (`docs/api/mensagens.md`, "Canal de cada mensagem").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    Ws,
    Quic,
}

/// Token de sessão (vigente ou antigo ainda tolerado por 5 min depois da renovação).
#[derive(Debug, Clone)]
pub struct TokenEntry {
    pub participant_id: String,
    pub expires_at_ms: u64,
}

/// Protocolo de rede que este servidor fala (`protocolo` em `POST /v1/sessoes`).
const PROTOCOLO_ATUAL: u32 = 4;
/// Validade de um token novo: 24 h.
const VALIDADE_TOKEN_MS: u64 = 86_400_000;
/// Quanto tempo o token antigo ainda vale depois de uma renovação: 5 min.
const TOLERANCIA_RENOVACAO_MS: u64 = 300_000;

/// Quem está logado.
pub struct ParticipantData {
    pub id: String,
    /// Número u64 usado como id do espectador no `Allocator`.
    pub num: u64,
    pub name: String,
    pub color: String,
    pub install_id: String,
    pub app_version: String,
    pub protocol: u32,
    /// Sala em que está (após `POST …/entrar`).
    pub room_id: Option<String>,
    /// Sala cujos eventos o WebSocket recebe (após `sala.entrar`).
    pub ws_room: Option<String>,
    pub joined_at: String,
    /// Canal de saída do WebSocket atual (um por participante; o mais novo vence).
    pub ws: Option<mpsc::UnboundedSender<ControlMsg>>,
    /// Trilha de vídeo que o participante assiste em tamanho principal/cheia (`None` = nenhuma
    /// ou só miniaturas). Vai em `Participant::watching`.
    pub watching: Option<String>,
    /// Conexão QUIC da ponte atual, se houver (uma por participante: a nova substitui a antiga).
    pub quic: Option<quinn::Connection>,
    /// Canal de saída do stream de controle dessa conexão. As mensagens da mídia
    /// (`camadas.pedidas`, `keyframe.pedir`, `camada.atual`, `relatorio`) só saem por aqui.
    pub quic_ctl: Option<mpsc::UnboundedSender<ControlMsg>>,
    /// Sala do ingresso dessa conexão: o `ola` com ingresso vale como `sala.entrar`, então
    /// assinaturas feitas pelo QUIC (é assim que a prévia assina) usam esta sala.
    pub quic_room: Option<String>,
    /// Porta UDP de origem vista na conexão QUIC (`GET /v1/diagnostico`).
    pub quic_port: Option<u16>,
    pub public_ip: String,
    /// Sondagem de banda da conexão QUIC atual (`None` sem conexão ou na prévia).
    pub prober: Option<std::sync::Arc<zap_net::probe::Prober>>,
}

/// Ingresso emitido por `entrar`/`previa`.
#[derive(Debug, Clone)]
pub struct TicketData {
    pub participant_id: String,
    pub room_id: String,
    pub preview: bool,
    pub expires_at_ms: u64,
}

/// Mensagem de chat guardada (últimas 200).
#[derive(Debug, Clone)]
pub struct ChatEntry {
    pub id: String,
    pub participant_id: String,
    pub name: String,
    pub color: String,
    pub text: String,
    pub at: String,
}

/// Trilha publicada numa sala.
pub struct TrackData {
    pub id: String,
    /// `u16` único na sala (`Trilha.numero`).
    pub number: u16,
    pub participant_id: String,
    pub kind: TrackKind,
    pub title: String,
    pub layers: Vec<LayerSpec>,
    pub audio: Option<AudioSpec>,
    /// Alocador e assinantes desta trilha.
    pub relay: TrackRelay,
}

/// Sala.
pub struct RoomData {
    pub id: String,
    pub code: RoomCode,
    pub name: String,
    pub owner_id: String,
    pub access: Access,
    pub password_hash: Option<String>,
    pub created_at: String,
    /// Quem entrou por `POST …/entrar` (ordem de entrada).
    pub members: Vec<String>,
    pub tracks: Vec<TrackData>,
    pub chat: VecDeque<ChatEntry>,
    pub next_track_number: u16,
    /// Quando ficou vazia (a sala some 2 min depois; no teste, `Hub::sweep`).
    pub empty_since_ms: Option<u64>,
}

/// Tudo o que o `Mutex` protege.
#[derive(Default)]
pub struct Inner {
    pub tokens: HashMap<String, TokenEntry>,
    pub participants: HashMap<String, ParticipantData>,
    pub rooms: HashMap<String, RoomData>,
    pub tickets: HashMap<String, TicketData>,
    pub next_num: u64,
}

/// Estado compartilhado entre REST, WebSocket e ponte.
pub struct Hub {
    pub config: Config,
    pub inner: Mutex<Inner>,
    /// Endereço da ponte (`host:porta`), preenchido por `spawn` depois do bind QUIC.
    pub bridge_addr: OnceLock<String>,
    /// Relógio monotônico para o alocador (`now_us`).
    pub clock: zap_net::Clock,
}

impl Hub {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            inner: Mutex::new(Inner::default()),
            bridge_addr: OnceLock::new(),
            clock: zap_net::Clock::new(),
        }
    }

    /// `POST /v1/sessoes`: confere protocolo (4) e senha do servidor, cria participante
    /// (reaproveita o de mesmo `instalacao_id`) e token de 24 h.
    pub fn login(&self, req: LoginRequest, remote_ip: &str) -> Result<SessionResponse, HubError> {
        if req.protocol != PROTOCOLO_ATUAL {
            return Err(HubError::new(
                426,
                "protocolo_incompativel",
                "Atualize o StreamaZap: o servidor fala o protocolo 4",
            ));
        }
        if let Some(senha) = &self.config.server_password {
            if req.server_password.as_deref() != Some(senha.as_str()) {
                return Err(HubError::new(401, "senha_servidor", "Senha do servidor incorreta"));
            }
        }
        let tamanho_nome = req.name.chars().count();
        if tamanho_nome == 0 || tamanho_nome > 32 {
            return Err(HubError::new(400, "invalido", "nome inválido"));
        }

        let agora = now_unix_ms();
        let mut inner = self.inner.lock();

        // Mesmo `instalacao_id` = mesmo participante, mesmo com token novo.
        let existente = inner
            .participants
            .values_mut()
            .find(|p| p.install_id == req.install_id)
            .map(|p| {
                p.name = req.name.clone();
                p.color = req.color.clone();
                p.app_version = req.app_version.clone();
                p.protocol = req.protocol;
                p.public_ip = remote_ip.to_string();
                p.id.clone()
            });

        let participant_id = match existente {
            Some(id) => id,
            None => {
                let id = new_id("p");
                let num = inner.next_num;
                inner.next_num += 1;
                inner.participants.insert(
                    id.clone(),
                    ParticipantData {
                        id: id.clone(),
                        num,
                        name: req.name.clone(),
                        color: req.color.clone(),
                        install_id: req.install_id.clone(),
                        app_version: req.app_version.clone(),
                        protocol: req.protocol,
                        room_id: None,
                        ws_room: None,
                        joined_at: now_iso(),
                        ws: None,
                        watching: None,
                        quic: None,
                        quic_ctl: None,
                        quic_room: None,
                        quic_port: None,
                        public_ip: remote_ip.to_string(),
                        prober: None,
                    },
                );
                id
            }
        };

        let token = new_token();
        let expira_em = agora + VALIDADE_TOKEN_MS;
        inner.tokens.insert(
            token.clone(),
            TokenEntry {
                participant_id: participant_id.clone(),
                expires_at_ms: expira_em,
            },
        );
        Ok(SessionResponse {
            token,
            expires_at: iso_from_unix_ms(expira_em),
            participant_id,
            public_ip: Some(remote_ip.to_string()),
        })
    }

    /// `POST /v1/sessoes/renovar`: token novo; o antigo vale mais 5 min.
    pub fn renew(&self, token: &str) -> Result<SessionResponse, HubError> {
        let agora = now_unix_ms();
        let mut inner = self.inner.lock();
        // Confere e troca tudo sob o mesmo lock, para não haver corrida com outra renovação.
        let participant_id = dono_do_token(&inner, token, agora)?;

        if let Some(antigo) = inner.tokens.get_mut(token) {
            antigo.expires_at_ms = antigo.expires_at_ms.min(agora + TOLERANCIA_RENOVACAO_MS);
        }

        let novo = new_token();
        let expira_em = agora + VALIDADE_TOKEN_MS;
        inner.tokens.insert(
            novo.clone(),
            TokenEntry {
                participant_id: participant_id.clone(),
                expires_at_ms: expira_em,
            },
        );
        let public_ip = inner.participants.get(&participant_id).map(|p| p.public_ip.clone());
        Ok(SessionResponse {
            token: novo,
            expires_at: iso_from_unix_ms(expira_em),
            participant_id,
            public_ip,
        })
    }

    /// Id do participante dono do token, se válido e não expirado.
    pub fn authenticate(&self, token: &str) -> Result<String, HubError> {
        let agora = now_unix_ms();
        let inner = self.inner.lock();
        dono_do_token(&inner, token, agora)
    }
}

/// Participante dono de `token`, se ele existe e ainda não expirou em `agora`.
fn dono_do_token(inner: &Inner, token: &str, agora: u64) -> Result<String, HubError> {
    match inner.tokens.get(token) {
        Some(entrada) if entrada.expires_at_ms > agora => Ok(entrada.participant_id.clone()),
        _ => Err(HubError::new(401, "nao_autenticado", "Sessão inválida ou expirada")),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::Config;

    fn pedido(install_id: &str, nome: &str, protocolo: u32, senha: Option<&str>) -> LoginRequest {
        LoginRequest {
            name: nome.into(),
            color: "#3366ff".into(),
            install_id: install_id.into(),
            protocol: protocolo,
            app_version: "3.0.0".into(),
            server_password: senha.map(String::from),
        }
    }

    fn hub_com_senha(senha: Option<&str>) -> Hub {
        Hub::new(Config {
            server_password: senha.map(String::from),
            ..Config::default()
        })
    }

    #[test]
    fn login_ok() {
        let hub = Hub::new(Config::default());
        let resp = hub.login(pedido("inst-1", "Ana", 4, None), "203.0.113.7").unwrap();
        assert_eq!(resp.token.len(), 32);
        assert_eq!(resp.public_ip.as_deref(), Some("203.0.113.7"));
        assert!(resp.participant_id.starts_with("p_"));
        assert_eq!(hub.authenticate(&resp.token).unwrap(), resp.participant_id);
        let inner = hub.inner.lock();
        assert_eq!(inner.participants[&resp.participant_id].name, "Ana");
    }

    #[test]
    fn login_mesmo_install_id() {
        let hub = Hub::new(Config::default());
        let a = hub.login(pedido("inst-1", "Ana", 4, None), "203.0.113.7").unwrap();
        let b = hub.login(pedido("inst-1", "Ana B", 4, None), "203.0.113.9").unwrap();
        assert_eq!(a.participant_id, b.participant_id);
        assert_ne!(a.token, b.token);
        let inner = hub.inner.lock();
        assert_eq!(inner.participants.len(), 1);
        assert_eq!(inner.participants[&a.participant_id].name, "Ana B");
        assert_eq!(inner.participants[&a.participant_id].public_ip, "203.0.113.9");
    }

    #[test]
    fn protocolo_errado() {
        let hub = Hub::new(Config::default());
        let erro = hub.login(pedido("inst-1", "Ana", 3, None), "203.0.113.7").unwrap_err();
        assert_eq!(erro.status, 426);
        assert_eq!(erro.code, "protocolo_incompativel");
    }

    #[test]
    fn senha_servidor() {
        let hub = hub_com_senha(Some("segredo"));
        let sem = hub.login(pedido("inst-1", "Ana", 4, None), "ip").unwrap_err();
        assert_eq!((sem.status, sem.code), (401, "senha_servidor"));
        let errada = hub.login(pedido("inst-1", "Ana", 4, Some("outra")), "ip").unwrap_err();
        assert_eq!((errada.status, errada.code), (401, "senha_servidor"));
        assert!(hub.login(pedido("inst-1", "Ana", 4, Some("segredo")), "ip").is_ok());
    }

    #[test]
    fn nome_invalido() {
        let hub = Hub::new(Config::default());
        let vazio = hub.login(pedido("inst-1", "", 4, None), "ip").unwrap_err();
        assert_eq!((vazio.status, vazio.code), (400, "invalido"));
        let longo = "x".repeat(33);
        let grande = hub.login(pedido("inst-1", &longo, 4, None), "ip").unwrap_err();
        assert_eq!((grande.status, grande.code), (400, "invalido"));
        let limite = "x".repeat(32);
        assert!(hub.login(pedido("inst-1", &limite, 4, None), "ip").is_ok());
    }

    #[test]
    fn renovar() {
        let hub = Hub::new(Config::default());
        let antigo = hub.login(pedido("inst-1", "Ana", 4, None), "ip").unwrap();
        let novo = hub.renew(&antigo.token).unwrap();
        assert_ne!(novo.token, antigo.token);
        assert_eq!(novo.participant_id, antigo.participant_id);
        assert_eq!(hub.authenticate(&novo.token).unwrap(), antigo.participant_id);
        // Token antigo ainda vale (tolerância de 5 min).
        assert_eq!(hub.authenticate(&antigo.token).unwrap(), antigo.participant_id);
        // Forçando a expiração do antigo, ele deixa de valer.
        hub.inner.lock().tokens.get_mut(&antigo.token).unwrap().expires_at_ms = 0;
        let erro = hub.authenticate(&antigo.token).unwrap_err();
        assert_eq!(erro.code, "nao_autenticado");
    }

    #[test]
    fn token_desconhecido() {
        let hub = Hub::new(Config::default());
        let erro = hub.authenticate("nao-existe").unwrap_err();
        assert_eq!((erro.status, erro.code), (401, "nao_autenticado"));
        let erro = hub.renew("nao-existe").unwrap_err();
        assert_eq!((erro.status, erro.code), (401, "nao_autenticado"));
    }
}
