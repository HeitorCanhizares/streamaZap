//! Storage de autenticação do supabase-js (comandos `auth_store_*`): valores de qualquer
//! tamanho sobre credenciais pequenas. Nunca loga chaves de sessão nem valores.
//!
//! Layout: a credencial `<key>` guarda a CONTAGEM de pedaços (`"3"`); os pedaços ficam em
//! `<key>#0`, `<key>#1`... (ver [`crate::auth_chunks`]).

use std::collections::HashMap;
use std::sync::OnceLock;

use parking_lot::Mutex;

use crate::auth_chunks::{chunk_key, join_chunks, parse_count, split_value, MAX_UNITS};

/// Erro do storage: texto em português para a interface.
pub type StoreResult<T> = Result<T, String>;

/// Uma credencial por nome. `delete` de nome que não existe é `Ok`.
pub trait CredStore: Send + Sync {
    fn get(&self, name: &str) -> StoreResult<Option<String>>;
    fn set(&self, name: &str, value: &str) -> StoreResult<()>;
    fn delete(&self, name: &str) -> StoreResult<()>;
}

/// Storage em memória: usado nos testes e fora do Windows.
#[derive(Default)]
pub struct MemoryStore {
    pub map: Mutex<HashMap<String, String>>,
}

impl CredStore for MemoryStore {
    fn get(&self, name: &str) -> StoreResult<Option<String>> {
        Ok(self.map.lock().get(name).cloned())
    }
    fn set(&self, name: &str, value: &str) -> StoreResult<()> {
        self.map.lock().insert(name.to_string(), value.to_string());
        Ok(())
    }
    fn delete(&self, name: &str) -> StoreResult<()> {
        self.map.lock().remove(name);
        Ok(())
    }
}

/// Storage do processo: Gerenciador de Credenciais no Windows, memória nos outros sistemas.
pub fn default_store() -> &'static dyn CredStore {
    static STORE: OnceLock<Box<dyn CredStore>> = OnceLock::new();
    STORE
        .get_or_init(|| {
            #[cfg(windows)]
            {
                Box::new(crate::auth_keyring::KeyringStore::new())
            }
            #[cfg(not(windows))]
            {
                Box::new(MemoryStore::default())
            }
        })
        .as_ref()
}

/// Lê a contagem de pedaços de `key` (0 se ausente ou inválida).
fn read_count(s: &dyn CredStore, key: &str) -> StoreResult<usize> {
    Ok(s.get(key)?.as_deref().and_then(parse_count).unwrap_or(0))
}

/// Lê o valor completo de `key`. Sem contagem → `None`. Contagem inválida ou pedaço faltando
/// (credencial corrompida) → `None`, sem erro.
pub fn store_get(s: &dyn CredStore, key: &str) -> StoreResult<Option<String>> {
    let Some(texto) = s.get(key)? else {
        return Ok(None);
    };
    let Some(n) = parse_count(&texto) else {
        return Ok(None);
    };
    let mut pedacos = Vec::with_capacity(n.min(1024));
    for i in 0..n {
        match s.get(&chunk_key(key, i))? {
            Some(p) => pedacos.push(p),
            None => return Ok(None),
        }
    }
    Ok(Some(join_chunks(&pedacos)))
}

/// Grava `value` em `key`: grava os pedaços novos, depois a contagem, depois apaga os pedaços
/// antigos que sobraram (contagem antiga maior que a nova).
pub fn store_set(s: &dyn CredStore, key: &str, value: &str) -> StoreResult<()> {
    let antiga = read_count(s, key)?;
    let pedacos = split_value(value, MAX_UNITS);
    for (i, p) in pedacos.iter().enumerate() {
        s.set(&chunk_key(key, i), p)?;
    }
    s.set(key, &pedacos.len().to_string())?;
    for i in pedacos.len()..antiga {
        s.delete(&chunk_key(key, i))?;
    }
    Ok(())
}

/// Remove a contagem e todos os pedaços de `key`. Chave que não existe é `Ok`.
pub fn store_remove(s: &dyn CredStore, key: &str) -> StoreResult<()> {
    let n = read_count(s, key)?;
    for i in 0..n {
        s.delete(&chunk_key(key, i))?;
    }
    s.delete(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grande() -> String {
        (0..6144).map(|i| char::from(b'a' + (i * 7 % 26) as u8)).collect()
    }

    fn chaves(m: &MemoryStore) -> Vec<String> {
        let mut v: Vec<String> = m.map.lock().keys().cloned().collect();
        v.sort();
        v
    }

    #[test]
    fn roundtrip_curto() {
        let m = MemoryStore::default();
        store_set(&m, "k", "abc").unwrap();
        assert_eq!(store_get(&m, "k").unwrap(), Some("abc".to_string()));
        assert_eq!(m.map.lock().get("k").unwrap(), "1");
        assert!(m.map.lock().contains_key("k#0"));
    }

    #[test]
    fn roundtrip_6kb() {
        let m = MemoryStore::default();
        let v = grande();
        store_set(&m, "k", &v).unwrap();
        assert_eq!(store_get(&m, "k").unwrap(), Some(v));
        assert_eq!(m.map.lock().get("k").unwrap(), "7");
        for i in 0..7 {
            assert!(m.map.lock().contains_key(&format!("k#{i}")));
        }
        assert!(m.map.lock().values().all(|x| x.encode_utf16().count() <= 1000));
    }

    #[test]
    fn valor_menor_apaga_sobras() {
        let m = MemoryStore::default();
        store_set(&m, "k", &grande()).unwrap();
        store_set(&m, "k", "x").unwrap();
        assert_eq!(store_get(&m, "k").unwrap(), Some("x".to_string()));
        assert_eq!(chaves(&m), vec!["k".to_string(), "k#0".to_string()]);
    }

    #[test]
    fn valor_vazio() {
        let m = MemoryStore::default();
        store_set(&m, "k", "").unwrap();
        assert_eq!(store_get(&m, "k").unwrap(), Some(String::new()));
        assert_eq!(chaves(&m), vec!["k".to_string()]);
        assert_eq!(m.map.lock().get("k").unwrap(), "0");
    }

    #[test]
    fn remove_apaga_tudo() {
        let m = MemoryStore::default();
        store_set(&m, "k", &grande()).unwrap();
        store_remove(&m, "k").unwrap();
        assert!(m.map.lock().is_empty());
        assert_eq!(store_get(&m, "k").unwrap(), None);
        store_remove(&m, "k").unwrap();
        store_remove(&m, "inexistente").unwrap();
    }

    #[test]
    fn get_sem_chave_none() {
        let m = MemoryStore::default();
        assert_eq!(store_get(&m, "k").unwrap(), None);
    }

    #[test]
    fn pedaco_faltando_da_none() {
        let m = MemoryStore::default();
        store_set(&m, "k", &grande()).unwrap();
        m.delete("k#2").unwrap();
        assert_eq!(store_get(&m, "k").unwrap(), None);
    }

    #[test]
    fn contagem_invalida_da_none() {
        let m = MemoryStore::default();
        m.set("k", "abc").unwrap();
        assert_eq!(store_get(&m, "k").unwrap(), None);
    }

    #[test]
    fn chaves_independentes() {
        let m = MemoryStore::default();
        store_set(&m, "a", &grande()).unwrap();
        store_set(&m, "b", "bbb").unwrap();
        store_remove(&m, "a").unwrap();
        assert_eq!(store_get(&m, "a").unwrap(), None);
        assert_eq!(store_get(&m, "b").unwrap(), Some("bbb".to_string()));
    }

    #[test]
    fn unicode_ida_e_volta() {
        let m = MemoryStore::default();
        let v = "😀".repeat(700);
        store_set(&m, "k", &v).unwrap();
        assert_eq!(store_get(&m, "k").unwrap(), Some(v));
    }
}
