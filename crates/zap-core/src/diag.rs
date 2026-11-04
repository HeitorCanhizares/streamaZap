//! Relatório de diagnóstico (M5): tipos serializáveis entregues à interface (comando
//! `diagnostics_run`) e ao "Copiar relatório". A montagem pura fica em `diag_build`, o texto em
//! `diag_text` e a coleta no sistema/descoberta em `diag_collect`.
//!
//! Chaves JSON em snake_case (como o `Snapshot`). O bloco `firewall` reaproveita
//! `zap_sys::FirewallStatus` (chaves em português: `suportado`, `bloqueado`, `liberado`, ...).

use std::net::IpAddr;

use serde::{Deserialize, Serialize};
use zap_sys::FirewallStatus;

use crate::snapshot::{PeerListing, RoomListing, RoomView};
use crate::types::RoomMode;

/// Gravidade de um problema encontrado.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warn,
    Error,
}

/// Um achado do diagnóstico, com código estável (a interface pode escolher o texto/ação).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DiagProblem {
    pub severity: Severity,
    /// Ver a lista de códigos em `diag_build::encontrar_problemas`.
    pub code: String,
    /// Frase em português, pronta para mostrar.
    pub message: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DiagInterface {
    pub name: String,
    pub ip: String,
    /// Broadcast dirigido (ausente em /31 e /32).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub broadcast: Option<String>,
    pub radmin: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct DiagNetwork {
    pub interfaces: Vec<DiagInterface>,
    /// IP da primeira interface Radmin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radmin_ip: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DiagFriend {
    pub name: String,
    pub version: String,
    pub ip: String,
    /// Ele me ouve (lista a minha instalação na presença dele).
    pub hears_you: bool,
    /// `version` igual à versão deste app.
    pub same_version: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DiagRoom {
    pub id: String,
    pub name: String,
    pub host: String,
    pub participants: u32,
    pub has_password: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_warn: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DiagCurrentRoom {
    pub name: String,
    pub mode: RoomMode,
    pub participants: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// Outra versão de protocolo vista na rede (descoberta `other_versions`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DiagOtherVersion {
    pub ip: String,
    pub protocol: u32,
    /// Protocolo maior que o deste app: é preciso atualizar.
    pub newer: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DiagReport {
    pub app_version: String,
    pub protocol: u32,
    pub network: DiagNetwork,
    /// Rótulos de outras VPNs ativas (sem Radmin).
    pub vpns: Vec<String>,
    /// Antivírus com firewall próprio.
    pub antivirus: Vec<String>,
    pub firewall: FirewallStatus,
    pub friends: Vec<DiagFriend>,
    pub rooms: Vec<DiagRoom>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_room: Option<DiagCurrentRoom>,
    pub other_versions: Vec<DiagOtherVersion>,
    /// Maior protocolo visto na rede, se for maior que o meu.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub newer_protocol: Option<u32>,
    pub problems: Vec<DiagProblem>,
}

/// Dados brutos já coletados; `diag_build::montar_relatorio` os transforma em `DiagReport`
/// sem tocar no sistema (testável em qualquer SO).
#[derive(Clone, Debug)]
pub struct DiagInput {
    pub app_version: String,
    pub protocol: u32,
    pub interfaces: Vec<zap_sys::Interface>,
    pub vpns: Vec<String>,
    pub antivirus: Vec<String>,
    pub firewall: FirewallStatus,
    pub peers: Vec<PeerListing>,
    pub rooms: Vec<RoomListing>,
    pub room: Option<RoomView>,
    pub other_versions: Vec<(IpAddr, u32)>,
}
