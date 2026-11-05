//! Renovação automática do token de sessão (1 h antes de expirar).
use std::{future::Future, pin::Pin, time::Duration};

/// Antecedência da renovação.
pub(crate) const RENEW_BEFORE: Duration = Duration::from_secs(3600);

/// Relógio da renovação do token: hora de parede e espera. Produção: `SystemTime` +
/// `tokio::time::sleep` (`SystemClock`); os testes injetam um relógio controlado.
pub(crate) trait RenewClock: Send + Sync + 'static {
    /// Segundos desde 1970-01-01 UTC.
    fn now_unix(&self) -> u64;
    /// Espera `d`.
    fn sleep(&self, d: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>>;
}

/// Relógio de produção.
pub(crate) struct SystemClock;

impl RenewClock for SystemClock {
    fn now_unix(&self) -> u64 {
        // Relógio do sistema antes da época: erro vira 0, como `agora_unix` do cliente.
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
    fn sleep(&self, d: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(tokio::time::sleep(d))
    }
}

/// Converte `2026-11-05T21:30:00Z` (ISO 8601 UTC, com ou sem fração de segundo) em segundos
/// desde 1970-01-01 UTC. Formato inválido = `None`. Sem dependência de crate de datas.
pub(crate) fn parse_iso_utc(text: &str) -> Option<u64> {
    let b = text.as_bytes();
    // Base fixa `AAAA-MM-DDTHH:MM:SS` (19 bytes) seguida de `Z` ou `.fração Z`.
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let ano = digitos(&b[0..4])? as i64;
    let mes = digitos(&b[5..7])?;
    let dia = digitos(&b[8..10])?;
    let hora = digitos(&b[11..13])?;
    let min = digitos(&b[14..16])?;
    let seg = digitos(&b[17..19])?;
    match &b[19..] {
        [b'Z'] => {}
        [b'.', resto @ ..] => match resto.split_last() {
            Some((b'Z', frac)) if !frac.is_empty() && frac.iter().all(u8::is_ascii_digit) => {}
            _ => return None,
        },
        _ => return None,
    }
    if !(1..=12).contains(&mes) || !(1..=31).contains(&dia) || hora >= 24 || min >= 60 || seg > 60 {
        return None;
    }
    // Dias desde 1970-01-01 pelo algoritmo days_from_civil (Howard Hinnant).
    // A fração de segundo é ignorada; `div_euclid` mantém a conta correta para anos < 400 a.C.
    let ano = if mes <= 2 { ano - 1 } else { ano };
    let era = ano.div_euclid(400);
    let yoe = ano - era * 400;
    let mp = if mes > 2 { mes as i64 - 3 } else { mes as i64 + 9 };
    let doy = (153 * mp + 2) / 5 + dia as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let dias = era * 146_097 + doe - 719_468;
    let no_dia = hora * 3600 + min * 60 + seg;
    let segundos = dias.checked_mul(86_400)?.checked_add(i64::try_from(no_dia).ok()?)?;
    u64::try_from(segundos).ok()
}

/// Lê um trecho só de dígitos ASCII. Qualquer outro byte = `None`.
fn digitos(b: &[u8]) -> Option<u64> {
    if b.is_empty() || !b.iter().all(u8::is_ascii_digit) {
        return None;
    }
    b.iter()
        .try_fold(0u64, |acc, &c| acc.checked_mul(10)?.checked_add(u64::from(c - b'0')))
}

/// Quanto esperar até renovar: `expira_em - agora - RENEW_BEFORE`, nunca negativo
/// (já dentro da janela = `Duration::ZERO`). `now_unix` em segundos.
pub(crate) fn renew_delay(expires_unix: u64, now_unix: u64) -> Duration {
    let restante = expires_unix.saturating_sub(now_unix);
    Duration::from_secs(restante.saturating_sub(RENEW_BEFORE.as_secs()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoca_zero() {
        assert_eq!(parse_iso_utc("1970-01-01T00:00:00Z"), Some(0));
    }

    #[test]
    fn data_conhecida() {
        assert_eq!(parse_iso_utc("2026-11-05T21:30:00Z"), Some(1_793_914_200));
    }

    #[test]
    fn com_fracao() {
        assert_eq!(parse_iso_utc("2026-11-05T21:30:00.123Z"), Some(1_793_914_200));
        assert_eq!(parse_iso_utc("1970-01-01T00:00:01.5Z"), Some(1));
    }

    #[test]
    fn bissexto() {
        // 2024-02-29 = 19782 dias desde a época.
        assert_eq!(parse_iso_utc("2024-02-29T00:00:00Z"), Some(19_782 * 86_400));
        assert_eq!(parse_iso_utc("2024-03-01T00:00:00Z"), Some(19_783 * 86_400));
    }

    #[test]
    fn invalidas() {
        assert_eq!(parse_iso_utc("2026-11-05T21:30:00"), None);
        assert_eq!(parse_iso_utc("2026-13-05T21:30:00Z"), None);
        assert_eq!(parse_iso_utc("2026-11-05T21:30:00.Z"), None);
        assert_eq!(parse_iso_utc("2026-11-05T24:00:00Z"), None);
        assert_eq!(parse_iso_utc("2026-11-05T21:60:00Z"), None);
        assert_eq!(parse_iso_utc("2026-11-05T21:30:00Zx"), None);
        assert_eq!(parse_iso_utc("2026-1a-05T21:30:00Z"), None);
        assert_eq!(parse_iso_utc(""), None);
    }

    #[test]
    fn renew_delay_normal() {
        let agora = 1_000_000;
        assert_eq!(renew_delay(agora + 86_400, agora), Duration::from_secs(23 * 3600));
    }

    #[test]
    fn renew_delay_dentro_da_janela() {
        let agora = 1_000_000;
        assert_eq!(renew_delay(agora + 1800, agora), Duration::ZERO);
    }

    #[test]
    fn renew_delay_ja_expirado() {
        let agora = 1_000_000;
        assert_eq!(renew_delay(agora - 60, agora), Duration::ZERO);
    }

    #[test]
    fn system_clock_now_unix_razoavel() {
        let agora = SystemClock.now_unix();
        assert!(agora > 1_700_000_000);
        let referencia = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        assert!(referencia.abs_diff(agora) <= 2);
    }

    #[tokio::test]
    async fn system_clock_sleep_espera() {
        let inicio = std::time::Instant::now();
        SystemClock.sleep(Duration::from_millis(30)).await;
        assert!(inicio.elapsed() >= Duration::from_millis(25));
    }
}
