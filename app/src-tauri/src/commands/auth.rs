//! Storage do supabase-js no Gerenciador de Credenciais (`auth_store_get/set/remove`).
//! Erro = `String` em português. Nunca logar chave nem valor.

use crate::auth_store::{default_store, store_get, store_remove, store_set, CredStore};

/// Tamanho máximo da chave, em bytes.
const MAX_KEY_BYTES: usize = 200;

const ERRO_ACESSO: &str = "Falha ao acessar as credenciais";

/// Recusa chave vazia ou grande demais, antes de qualquer I/O.
fn validar_chave(key: &str) -> Result<(), String> {
    if key.trim().is_empty() {
        return Err("Chave de credencial vazia".into());
    }
    if key.len() > MAX_KEY_BYTES {
        return Err("Chave de credencial grande demais".into());
    }
    Ok(())
}

fn get_com(s: &dyn CredStore, key: &str) -> Result<Option<String>, String> {
    validar_chave(key)?;
    store_get(s, key)
}

fn set_com(s: &dyn CredStore, key: &str, value: &str) -> Result<(), String> {
    validar_chave(key)?;
    store_set(s, key, value)
}

fn remove_com(s: &dyn CredStore, key: &str) -> Result<(), String> {
    validar_chave(key)?;
    store_remove(s, key)
}

/// `auth_store_get(key) -> Option<String>`.
#[tauri::command]
pub async fn auth_store_get(key: String) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || get_com(default_store(), &key))
        .await
        .map_err(|_| String::from(ERRO_ACESSO))?
}

/// `auth_store_set(key, value)`.
#[tauri::command]
pub async fn auth_store_set(key: String, value: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || set_com(default_store(), &key, &value))
        .await
        .map_err(|_| String::from(ERRO_ACESSO))?
}

/// `auth_store_remove(key)`.
#[tauri::command]
pub async fn auth_store_remove(key: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || remove_com(default_store(), &key))
        .await
        .map_err(|_| String::from(ERRO_ACESSO))?
}

/// `deep_link_pendentes() -> Vec<String>`: callbacks do login que chegaram com a interface ainda
/// fechada (o app aberto do zero pela URL). Esvazia a fila.
#[tauri::command]
pub fn deep_link_pendentes(pendentes: tauri::State<'_, crate::deep_link_setup::DeepLinkPendentes>) -> Vec<String> {
    pendentes.tirar()
}

/// `convites_pendentes() -> Vec<String>`: alvos de convite de sala (`ZAP-7K2Q` ou `ip:porta`) que
/// chegaram com a interface ainda fechada (o app aberto do zero pelo link). Esvazia a fila.
#[tauri::command]
pub fn convites_pendentes(convites: tauri::State<'_, crate::deep_link_setup::ConvitesPendentes>) -> Vec<String> {
    convites.tirar()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth_store::MemoryStore;

    #[test]
    fn set_get_remove_pelas_funcoes_internas() {
        let s = MemoryStore::default();
        set_com(&s, "sb-auth", "{\"a\":1}").unwrap();
        assert_eq!(get_com(&s, "sb-auth").unwrap().as_deref(), Some("{\"a\":1}"));
        remove_com(&s, "sb-auth").unwrap();
        assert_eq!(get_com(&s, "sb-auth").unwrap(), None);
    }

    #[test]
    fn valor_6kb_ida_e_volta() {
        let s = MemoryStore::default();
        let v: String = (0..6144).map(|i| char::from(b'a' + (i % 26) as u8)).collect();
        set_com(&s, "sb-grande", &v).unwrap();
        assert_eq!(get_com(&s, "sb-grande").unwrap(), Some(v));
    }

    #[test]
    fn chave_vazia_e_recusada() {
        let s = MemoryStore::default();
        for k in ["", "   "] {
            assert_eq!(get_com(&s, k).unwrap_err(), "Chave de credencial vazia");
            assert_eq!(set_com(&s, k, "x").unwrap_err(), "Chave de credencial vazia");
            assert_eq!(remove_com(&s, k).unwrap_err(), "Chave de credencial vazia");
        }
    }

    #[test]
    fn chave_grande_demais() {
        let s = MemoryStore::default();
        let ok = "k".repeat(200);
        let grande = "k".repeat(201);
        assert!(set_com(&s, &ok, "v").is_ok());
        assert_eq!(
            set_com(&s, &grande, "v").unwrap_err(),
            "Chave de credencial grande demais"
        );
        assert!(get_com(&s, &grande).is_err());
        assert!(remove_com(&s, &grande).is_err());
    }

    #[test]
    fn comandos_registrados_no_generate_handler() {
        let lib = include_str!("../lib.rs");
        for c in ["get", "set", "remove"] {
            let t = format!("commands::auth::auth_store_{c},");
            assert!(lib.contains(&t), "faltando {t}");
        }
        assert!(lib.contains("commands::auth::deep_link_pendentes,"));
        assert!(lib.contains("commands::auth::convites_pendentes,"));
    }

    #[test]
    fn nada_e_logado() {
        let src = include_str!("auth.rs");
        let fora_dos_testes = src.split("#[cfg(test)]").next().unwrap();
        assert!(!fora_dos_testes.contains("tracing::"));
    }
}
