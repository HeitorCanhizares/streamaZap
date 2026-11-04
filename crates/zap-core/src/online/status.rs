//! Estado do servidor Online mostrado na barra da interface.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ServerState {
    /// Sem endereço configurado.
    #[default]
    Off,
    /// Fazendo login / reconectando.
    Connecting,
    Online,
    /// Falhou (mensagem em `ServerStatus::message`).
    Error,
}

/// `Snapshot.server`. Campos opcionais não saem no JSON quando ausentes.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ServerStatus {
    pub state: ServerState,
    /// `nome` de `GET /v1/info`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Ping até o servidor, em ms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ping_ms: Option<u32>,
    /// Texto de erro para a interface (em português).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl ServerStatus {
    pub fn connecting(name: Option<String>) -> Self {
        Self {
            state: ServerState::Connecting,
            name,
            ping_ms: None,
            message: None,
        }
    }
    pub fn online(name: String, ping_ms: Option<u32>) -> Self {
        Self {
            state: ServerState::Online,
            name: Some(name),
            ping_ms,
            message: None,
        }
    }
    pub fn error(message: String) -> Self {
        Self {
            state: ServerState::Error,
            name: None,
            ping_ms: None,
            message: Some(message),
        }
    }
    /// Texto da barra: `Online` com ping → `"Servidor online · 42 ms"`; `Online` sem ping →
    /// `"Servidor online"`; `Connecting` → `"Servidor · conectando…"`; `Error` →
    /// `"Servidor · sem conexão"`; `Off` → `"Servidor · não configurado"`.
    pub fn label(&self) -> String {
        match (self.state, self.ping_ms) {
            (ServerState::Online, Some(ms)) => format!("Servidor online · {ms} ms"),
            (ServerState::Online, None) => "Servidor online".to_string(),
            (ServerState::Connecting, _) => "Servidor · conectando…".to_string(),
            (ServerState::Error, _) => "Servidor · sem conexão".to_string(),
            (ServerState::Off, _) => "Servidor · não configurado".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_online_com_ping() {
        assert_eq!(
            ServerStatus::online("srv".into(), Some(42)).label(),
            "Servidor online · 42 ms"
        );
    }

    #[test]
    fn label_online_sem_ping() {
        assert_eq!(ServerStatus::online("srv".into(), None).label(), "Servidor online");
    }

    #[test]
    fn label_connecting_error_off() {
        assert_eq!(ServerStatus::connecting(None).label(), "Servidor · conectando…");
        assert_eq!(ServerStatus::error("falhou".into()).label(), "Servidor · sem conexão");
        let default = ServerStatus::default();
        assert_eq!(default.state, ServerState::Off);
        assert_eq!(default.label(), "Servidor · não configurado");
        let off = ServerStatus {
            state: ServerState::Off,
            ..Default::default()
        };
        assert_eq!(off.label(), "Servidor · não configurado");
    }

    #[test]
    fn serde_omite_opcionais() {
        let v = serde_json::to_value(ServerStatus::connecting(None)).unwrap();
        assert_eq!(v["state"], "connecting");
        let obj = v.as_object().unwrap();
        assert!(!obj.contains_key("name"));
        assert!(!obj.contains_key("ping_ms"));
        assert!(!obj.contains_key("message"));
    }

    #[test]
    fn serde_roundtrip_online() {
        let s = ServerStatus::online("srv".into(), Some(7));
        let json = serde_json::to_string(&s).unwrap();
        let back: ServerStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
        assert_eq!(back.name.as_deref(), Some("srv"));
        assert_eq!(back.ping_ms, Some(7));
    }

    #[test]
    fn construtores_preenchem_campos() {
        let e = ServerStatus::error("sem rede".into());
        assert_eq!(e.state, ServerState::Error);
        assert_eq!(e.message.as_deref(), Some("sem rede"));
        assert!(e.name.is_none() && e.ping_ms.is_none());

        let o = ServerStatus::online("meu".into(), None);
        assert_eq!(o.state, ServerState::Online);
        assert_eq!(o.name.as_deref(), Some("meu"));
        assert!(o.message.is_none());
    }
}
