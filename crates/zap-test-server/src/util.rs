//! Utilitários puros: data ISO, tokens, ids e hash de senha (só para teste).

use rand::Rng;
use sha2::{Digest, Sha256};

/// Alfabeto para ids e tokens: letras e dígitos (62 caracteres).
const ALFANUMERICO: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

/// Data/hora UTC atual no formato `2026-11-04T21:30:00Z` (segundos, sem fração).
pub fn now_iso() -> String {
    iso_from_unix_ms(now_unix_ms())
}

/// `ms` desde a época Unix para ISO-8601 UTC (`...Z`). Separado para ser testável.
pub fn iso_from_unix_ms(ms: u64) -> String {
    // Ignoramos a fração de segundo: o formato pedido tem só segundos.
    let secs = ms / 1000;
    let dias = secs / 86_400;
    let sod = secs % 86_400;
    let (ano, mes, dia) = civil_de_dias(dias);
    let (h, mi, s) = (sod / 3600, (sod % 3600) / 60, sod % 60);
    format!("{ano:04}-{mes:02}-{dia:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Converte dias desde 1970-01-01 em (ano, mês, dia) do calendário gregoriano.
/// Algoritmo `civil_from_days` de Howard Hinnant, sem negativos (datas >= 1970).
fn civil_de_dias(dias: u64) -> (u64, u64, u64) {
    // Desloca a época para 0000-03-01, o que deixa o ciclo de 400 anos alinhado.
    let z = dias + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097; // dia do ciclo de 400 anos, em [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // ano do ciclo
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // dia dentro do ano (março = 0)
    let mp = (5 * doy + 2) / 153; // mês com março = 0
    let dia = doy - (153 * mp + 2) / 5 + 1;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 };
    let ano = yoe + era * 400 + u64::from(mes <= 2);
    (ano, mes, dia)
}

/// Milissegundos desde a época Unix agora.
pub fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|d| u64::try_from(d.as_millis()).ok())
        .unwrap_or(0)
}

/// Id aleatório com prefixo: `new_id("p")` → `p_8Kq2mZ0aL1` (10 caracteres alfanuméricos).
pub fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", caracteres_aleatorios(10))
}

/// Token/ingresso opaco: 32 caracteres alfanuméricos aleatórios.
pub fn new_token() -> String {
    caracteres_aleatorios(32)
}

/// `ZAP-XXXX` aleatório (`zap_proto::RoomCode::from_entropy`).
pub fn new_room_code() -> zap_proto::RoomCode {
    let mut rng = rand::rng();
    let bytes: [u8; 4] = std::array::from_fn(|_| rng.random());
    zap_proto::RoomCode::from_entropy(bytes)
}

/// Hash de senha de sala: `hex(sha256(salt || senha))` com sal aleatório, no formato
/// `"<salt>$<hex>"`. Só para teste: o servidor de produção usa Argon2id ou bcrypt.
pub fn hash_password(password: &str) -> String {
    let sal = caracteres_aleatorios(8);
    let hex = hash_com_sal(&sal, password);
    format!("{sal}${hex}")
}

/// Confere `password` contra o texto produzido por `hash_password`.
pub fn verify_password(password: &str, stored: &str) -> bool {
    // Sem '$' o texto não tem o formato `<salt>$<hex>`, então nunca confere.
    let Some((sal, hex)) = stored.split_once('$') else {
        return false;
    };
    hash_com_sal(sal, password) == hex
}

/// `hex(sha256(sal || senha))` em minúsculas, 64 caracteres.
fn hash_com_sal(sal: &str, password: &str) -> String {
    let mut h = Sha256::new();
    h.update(sal.as_bytes());
    h.update(password.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// `n` caracteres escolhidos de `ALFANUMERICO` com o gerador do thread.
fn caracteres_aleatorios(n: usize) -> String {
    let mut rng = rand::rng();
    (0..n)
        .map(|_| char::from(ALFANUMERICO[rng.random_range(0..ALFANUMERICO.len())]))
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn iso_zero() {
        assert_eq!(iso_from_unix_ms(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn iso_bissexto() {
        assert_eq!(iso_from_unix_ms(951_782_400_000), "2000-02-29T00:00:00Z");
        assert_eq!(iso_from_unix_ms(1_709_164_800_000), "2024-02-29T00:00:00Z");
    }

    #[test]
    fn iso_formato() {
        let s = now_iso();
        assert_eq!(s.len(), 20);
        assert!(s.ends_with('Z'));
        assert_eq!(s.as_bytes()[10], b'T');
    }

    #[test]
    fn ids_formato() {
        let a = new_id("p");
        assert!(a.starts_with("p_"));
        assert_eq!(a.len(), 12);
        let b = new_id("p");
        assert_ne!(a, b);
    }

    #[test]
    fn token_32() {
        let t = new_token();
        assert_eq!(t.len(), 32);
        assert!(t.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn codigo_valido() {
        assert!(zap_proto::RoomCode::parse(new_room_code().as_str()).is_ok());
    }

    #[test]
    fn senha_ok() {
        let h = hash_password("abc");
        assert!(verify_password("abc", &h));
        assert!(!verify_password("abd", &h));
        // Mesma senha, sal diferente: o texto armazenado muda.
        assert_ne!(h, hash_password("abc"));
    }

    #[test]
    fn senha_sem_cifrao() {
        assert!(!verify_password("x", "semcifrao"));
    }
}
