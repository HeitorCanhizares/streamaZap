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
