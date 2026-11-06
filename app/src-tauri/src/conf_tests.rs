//! Testes de configuração: garantem que `tauri.conf.json` e a capability do updater
//! continuam batendo com as constantes do `updater.rs`. Só testes, sem código de produção.

#![cfg(test)]

use crate::updater::{pubkey_configurada, ENDPOINT_ESTAVEL, PUBKEY_PLACEHOLDER};

const TAURI_CONF: &str = include_str!("../tauri.conf.json");
const CAPABILITY: &str = include_str!("../capabilities/default.json");

fn conf() -> serde_json::Value {
    serde_json::from_str(TAURI_CONF).expect("tauri.conf.json é JSON válido")
}

fn capability() -> serde_json::Value {
    serde_json::from_str(CAPABILITY).expect("capabilities/default.json é JSON válido")
}

#[test]
fn conf_updater_artifacts_e_nsis_usuario() {
    let c = conf();
    // Sem os artefatos do updater o latest.json não sai no build.
    assert_eq!(c["bundle"]["createUpdaterArtifacts"], true);
    // Instalação por usuário: a atualização não pede admin.
    assert_eq!(c["bundle"]["windows"]["nsis"]["installMode"], "currentUser");
}

#[test]
fn conf_updater_instala_em_modo_passivo() {
    let c = conf();
    // Atualização passiva: mostra só o progresso, sem pedir interação do usuário.
    assert_eq!(c["plugins"]["updater"]["windows"]["installMode"], "passive");
    // Coexiste com a instalação por usuário do NSIS (são configurações diferentes).
    assert_eq!(c["bundle"]["windows"]["nsis"]["installMode"], "currentUser");
}

#[test]
fn conf_endpoint_estavel_igual_constante() {
    let c = conf();
    let endpoints = c["plugins"]["updater"]["endpoints"]
        .as_array()
        .expect("endpoints é uma lista");
    assert_eq!(endpoints[0], ENDPOINT_ESTAVEL);
}

#[test]
fn conf_pubkey_placeholder_ou_real() {
    let c = conf();
    let pubkey = c["plugins"]["updater"]["pubkey"].as_str().expect("pubkey é texto");
    // Nunca vazia: vazia quebraria a verificação de assinatura.
    assert!(!pubkey.trim().is_empty(), "pubkey do updater não pode ser vazia");
    // Consistência: só é "configurada" quando não é o placeholder.
    assert_eq!(pubkey_configurada(pubkey), pubkey != PUBKEY_PLACEHOLDER);
}

#[test]
fn capability_tem_updater_e_restart() {
    let cap = capability();
    let permissions = cap["permissions"].as_array().expect("permissions é uma lista");
    let tem = |nome: &str| permissions.iter().any(|p| p == nome);
    assert!(tem("updater:default"), "falta updater:default na capability");
    assert!(
        tem("process:allow-restart"),
        "falta process:allow-restart na capability"
    );
}

const CARGO_RAIZ: &str = include_str!("../../../Cargo.toml");

fn diretivas_csp() -> Vec<String> {
    let c = conf();
    let csp = c["app"]["security"]["csp"].as_str().expect("csp é texto");
    csp.split(';')
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty())
        .collect()
}

#[test]
fn conf_csp_permite_supabase() {
    let dirs = diretivas_csp();
    let connect = dirs
        .iter()
        .find(|d| d.starts_with("connect-src"))
        .expect("existe connect-src");
    for esperado in [
        "https://*.supabase.co",
        "wss://*.supabase.co",
        "'self'",
        "ipc:",
        "http://ipc.localhost",
    ] {
        assert!(connect.contains(esperado), "connect-src sem {esperado}");
    }
}

#[test]
fn conf_csp_nao_abre_demais() {
    let dirs = diretivas_csp();
    for d in &dirs {
        assert!(
            !d.split_whitespace().any(|t| t == "*"),
            "diretiva com curinga solto: {d}"
        );
    }
    assert!(dirs.iter().any(|d| d == "script-src 'self'"));
}

#[test]
fn conf_deep_link_esquema_streamazap() {
    let c = conf();
    let schemes = &c["plugins"]["deep-link"]["desktop"]["schemes"];
    assert_eq!(schemes, &serde_json::json!(["streamazap"]));
}

#[test]
fn capability_tem_deep_link_e_opener_so_https() {
    let cap = capability();
    let permissions = cap["permissions"].as_array().expect("permissions é uma lista");
    assert!(permissions.iter().any(|p| p == "deep-link:default"));
    let opener = permissions
        .iter()
        .find(|p| p["identifier"] == "opener:allow-open-url")
        .expect("falta opener:allow-open-url com escopo");
    let allow = opener["allow"].as_array().expect("allow é uma lista");
    assert!(!allow.is_empty());
    for item in allow {
        let url = item["url"].as_str().expect("url é texto");
        assert!(url.starts_with("https://"), "url não https: {url}");
    }
    assert!(!CAPABILITY.contains("opener:default"));
    assert!(!CAPABILITY.contains("opener:allow-open-path"));
}

#[test]
fn capability_mantem_as_antigas() {
    let cap = capability();
    let permissions = cap["permissions"].as_array().expect("permissions é uma lista");
    let tem = |nome: &str| permissions.iter().any(|p| p == nome);
    assert!(tem("updater:default"));
    assert!(tem("process:allow-restart"));
    assert!(tem("notification:default"));
}

#[test]
fn conf_single_instance_tem_deep_link_no_manifesto() {
    let linha = CARGO_RAIZ
        .lines()
        .find(|l| l.trim_start().starts_with("tauri-plugin-single-instance"))
        .expect("single-instance no Cargo.toml da raiz");
    assert!(linha.contains("deep-link"));
}
