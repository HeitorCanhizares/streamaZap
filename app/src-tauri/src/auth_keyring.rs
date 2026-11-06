//! Credenciais no Gerenciador de Credenciais do Windows (crate `keyring`). Só Windows.

#![cfg(windows)]

use crate::auth_store::{CredStore, StoreResult};

/// Serviço usado nas credenciais.
pub const SERVICE: &str = "streamaZap";

/// `CredStore` sobre o `keyring`: alvo `streamaZap/<nome>`, serviço e usuário `streamaZap`.
pub struct KeyringStore;

impl KeyringStore {
    pub fn new() -> Self {
        Self
    }
}

impl Default for KeyringStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Entrada do `keyring` para um nome: alvo `streamaZap/<nome>`.
fn entry(name: &str) -> Result<keyring::Entry, keyring::Error> {
    keyring::Entry::new_with_target(&format!("{SERVICE}/{name}"), SERVICE, SERVICE)
}

impl CredStore for KeyringStore {
    fn get(&self, name: &str) -> StoreResult<Option<String>> {
        let erro = || "Não foi possível ler as credenciais do Windows".to_string();
        let e = entry(name).map_err(|err| {
            tracing::warn!("keyring: entrada inválida: {err}");
            erro()
        })?;
        match e.get_password() {
            Ok(s) => Ok(Some(s)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => {
                tracing::warn!("keyring: falha ao ler: {err}");
                Err(erro())
            }
        }
    }

    fn set(&self, name: &str, value: &str) -> StoreResult<()> {
        let erro = || "Não foi possível gravar nas credenciais do Windows".to_string();
        let e = entry(name).map_err(|err| {
            tracing::warn!("keyring: entrada inválida: {err}");
            erro()
        })?;
        e.set_password(value).map_err(|err| {
            tracing::warn!("keyring: falha ao gravar: {err}");
            erro()
        })
    }

    fn delete(&self, name: &str) -> StoreResult<()> {
        let erro = || "Não foi possível apagar das credenciais do Windows".to_string();
        let e = entry(name).map_err(|err| {
            tracing::warn!("keyring: entrada inválida: {err}");
            erro()
        })?;
        match e.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => {
                tracing::warn!("keyring: falha ao apagar: {err}");
                Err(erro())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth_store::{store_get, store_remove, store_set};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static N: AtomicUsize = AtomicUsize::new(0);

    fn nome(tag: &str) -> String {
        format!(
            "teste-keyring-{}-{}-{tag}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        )
    }

    #[test]
    fn roundtrip_real() {
        let s = KeyringStore::new();
        let n = nome("rt");
        let r = (|| -> StoreResult<(Option<String>, Option<String>)> {
            s.set(&n, "valor ç 😀")?;
            let a = s.get(&n)?;
            s.delete(&n)?;
            Ok((a, s.get(&n)?))
        })();
        let _ = s.delete(&n);
        let (a, b) = r.expect("keyring");
        assert_eq!(a.as_deref(), Some("valor ç 😀"));
        assert_eq!(b, None);
    }

    #[test]
    fn get_inexistente_none() {
        let s = KeyringStore::new();
        assert_eq!(s.get(&nome("inex")).expect("get"), None);
    }

    #[test]
    fn delete_inexistente_ok() {
        let s = KeyringStore::new();
        assert!(s.delete(&nome("delinex")).is_ok());
    }

    #[test]
    fn regrava_sobrescreve() {
        let s = KeyringStore::new();
        let n = nome("rg");
        let r = (|| -> StoreResult<Option<String>> {
            s.set(&n, "a")?;
            s.set(&n, "b")?;
            s.get(&n)
        })();
        let _ = s.delete(&n);
        assert_eq!(r.expect("keyring").as_deref(), Some("b"));
    }

    #[test]
    fn pedaco_de_1000_unidades_cabe() {
        let s = KeyringStore::new();
        let n = nome("1000");
        let v = "x".repeat(1000);
        let r = (|| -> StoreResult<Option<String>> {
            s.set(&n, &v)?;
            s.get(&n)
        })();
        let _ = s.delete(&n);
        assert_eq!(r.expect("keyring"), Some(v));
    }

    #[test]
    fn integracao_com_store_6kb() {
        let s = KeyringStore::new();
        let k = nome("6kb");
        let v: String = (0..6144).map(|i| char::from(b'a' + (i * 7 % 26) as u8)).collect();
        let r = (|| -> StoreResult<Option<String>> {
            store_set(&s, &k, &v)?;
            store_get(&s, &k)
        })();
        let limpo = store_remove(&s, &k);
        let depois = store_get(&s, &k);
        assert_eq!(r.expect("keyring"), Some(v));
        limpo.expect("remove");
        assert_eq!(depois.expect("get"), None);
    }
}
