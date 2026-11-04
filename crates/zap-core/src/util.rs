//! Funções pequenas e puras usadas por vários módulos.

use std::time::{SystemTime, UNIX_EPOCH};

/// Milissegundos desde a época Unix.
pub fn now_ms() -> u64 {
    // Relógio antes de 1970 (ajuste de hora) vira 0 em vez de erro: quem chama só precisa de um número.
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Data/hora UTC no formato `2026-01-31T12:34:56Z` a partir de ms Unix (sem dependências de data).
pub fn iso8601_from_ms(ms: u64) -> String {
    let secs = ms / 1000;
    let dias = (secs / 86_400) as i64;
    let resto = secs % 86_400;
    let (ano, mes, dia) = civil_from_days(dias);
    format!(
        "{ano:04}-{mes:02}-{dia:02}T{:02}:{:02}:{:02}Z",
        resto / 3600,
        (resto % 3600) / 60,
        resto % 60
    )
}

pub fn iso8601_now() -> String {
    iso8601_from_ms(now_ms())
}

/// Converte dias desde 1970-01-01 em (ano, mês, dia) do calendário gregoriano proléptico.
/// Algoritmo `civil_from_days` de Howard Hinnant (domínio público).
fn civil_from_days(dias: i64) -> (i64, u32, u32) {
    let z = dias + 719_468;
    // Era de 400 anos; o ajuste para negativos mantém o `floor` correto.
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let ano = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let mes = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let ano = if mes <= 2 { ano + 1 } else { ano };
    (ano, mes, dia)
}

/// `prefixo_` + 12 hex aleatórios (ex.: `p_3fa9c01b22de`).
pub fn new_id(prefix: &str) -> String {
    let hex = uuid::Uuid::new_v4().simple().to_string();
    format!("{prefix}{}", &hex[..12])
}

/// Endereços IPv4 locais em texto para anunciar numa trilha/sala. `interface`: nome da interface
/// ou `None` (todas). `include_loopback` acrescenta `127.0.0.1` (testes). Usa
/// `zap_net::discovery::ipv4_interfaces`. Radmin primeiro, depois as demais, sem repetir.
pub fn local_addresses(interface: Option<&str>, include_loopback: bool) -> Vec<String> {
    let mut interfaces: Vec<_> = zap_net::discovery::ipv4_interfaces()
        .into_iter()
        .filter(|i| interface.is_none_or(|nome| i.name == nome))
        .collect();
    // sort_by_key é estável: a ordem relativa das demais interfaces é mantida.
    interfaces.sort_by_key(|i| !i.is_radmin);

    let mut enderecos: Vec<String> = Vec::new();
    for i in interfaces {
        let texto = i.ip.to_string();
        if !enderecos.contains(&texto) {
            enderecos.push(texto);
        }
    }
    if include_loopback && !enderecos.iter().any(|e| e == "127.0.0.1") {
        enderecos.push("127.0.0.1".to_string());
    }
    enderecos
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_epoca() {
        assert_eq!(iso8601_from_ms(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn iso_conhecido() {
        assert_eq!(iso8601_from_ms(1_700_000_000_000), "2023-11-14T22:13:20Z");
    }

    #[test]
    fn iso_bissexto() {
        assert_eq!(iso8601_from_ms(951_782_400_000), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn new_id_formato() {
        let a = new_id("p_");
        let b = new_id("p_");
        assert!(a.starts_with("p_"));
        assert_eq!(a.len(), 14);
        assert!(a[2..].chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn now_ms_razoavel() {
        assert!(now_ms() > 1_700_000_000_000);
    }

    #[test]
    fn local_addresses_loopback() {
        let todas = local_addresses(None, true);
        assert_eq!(todas.last().map(String::as_str), Some("127.0.0.1"));
        assert_eq!(todas.iter().filter(|e| *e == "127.0.0.1").count(), 1);

        let nenhuma = local_addresses(Some("nao-existe"), true);
        assert_eq!(nenhuma, vec!["127.0.0.1".to_string()]);
    }
}
