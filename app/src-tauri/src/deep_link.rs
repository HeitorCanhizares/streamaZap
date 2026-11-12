//! Deep links, só filtros puros (a ligação com o plugin fica em `deep_link_setup`):
//! - `streamazap://auth/callback` (volta do OAuth);
//! - `streamazap://sala/<alvo>` (convite de sala: código `ZAP-XXXX` ou `ip[:porta]` da sala Local).

use std::net::{Ipv4Addr, SocketAddrV4};

/// Só URLs que começam assim chegam à interface.
pub const CALLBACK_PREFIX: &str = "streamazap://auth/callback";

/// Evento emitido para a interface; o payload é a URL (texto), um evento por URL.
pub const EVENT: &str = "zap://deep-link";

/// Prefixo do convite de sala. O que vem depois é o alvo.
pub const INVITE_PREFIX: &str = "streamazap://sala/";

/// Evento do convite de sala; o payload é o alvo já validado e normalizado (`ZAP-7K2Q` ou
/// `26.14.7.88:47800`), nunca a URL crua.
pub const EVENT_INVITE: &str = "zap://convite";

/// Alvo de `streamazap://sala/<alvo>`, normalizado:
/// - código de sala (`zap-7k2q`, `7K2Q`...) vira `ZAP-7K2Q` ([`zap_proto::RoomCode::parse`]);
/// - IPv4 com ou sem porta vira `ip:porta` (sem porta: [`zap_proto::DEFAULT_MEDIA_PORT`]).
///
/// O alvo termina no primeiro `?`, `#` ou `/` (uma `/` sozinha no fim é aceita). Esquema e host
/// sem diferenciar maiúsculas. Qualquer outra coisa (vazio, nome de host, porta 0, IPv6) é `None`.
pub fn room_invite(url: &str) -> Option<String> {
    let n = INVITE_PREFIX.len();
    // `get` em vez de fatiar: não entra em pânico se `n` cair no meio de um char.
    let inicio = url.get(..n)?;
    if !inicio.eq_ignore_ascii_case(INVITE_PREFIX) {
        return None;
    }
    let resto = url.get(n..)?;
    let fim = resto.find(['?', '#', '/']).unwrap_or(resto.len());
    // Depois do alvo só pode vir nada, query, fragmento ou uma barra final.
    let depois = &resto[fim..];
    if depois.starts_with('/') && depois.len() > 1 {
        return None;
    }
    normalize_target(&resto[..fim])
}

/// Valida e normaliza um alvo de convite (ver [`room_invite`]).
fn normalize_target(alvo: &str) -> Option<String> {
    if alvo.is_empty() {
        return None;
    }
    if let Ok(code) = zap_proto::RoomCode::parse(alvo) {
        return Some(code.as_str().to_string());
    }
    if let Ok(sock) = alvo.parse::<SocketAddrV4>() {
        return (sock.port() != 0).then(|| sock.to_string());
    }
    let ip = alvo.parse::<Ipv4Addr>().ok()?;
    Some(SocketAddrV4::new(ip, zap_proto::DEFAULT_MEDIA_PORT).to_string())
}

/// Os alvos de convite de `urls` ([`room_invite`]), na ordem; o resto é descartado.
pub fn filter_invites<S: AsRef<str>>(urls: &[S]) -> Vec<String> {
    urls.iter().filter_map(|u| room_invite(u.as_ref())).collect()
}

/// `true` se `url` é exatamente o callback ou continua com `?`, `#` ou `/` depois dele
/// (`streamazap://auth/callbackx` é recusada). Esquema e host sem diferenciar maiúsculas.
pub fn is_auth_callback(url: &str) -> bool {
    let n = CALLBACK_PREFIX.len();
    // `get` em vez de fatiar: não entra em pânico se `n` cair no meio de um char.
    match url.get(..n) {
        Some(inicio) if inicio.eq_ignore_ascii_case(CALLBACK_PREFIX) => {
            // O resto precisa começar a fronteira de caminho/query/fragmento.
            match url.get(n..) {
                Some(resto) => matches!(resto.chars().next(), None | Some('?' | '#' | '/')),
                None => false,
            }
        }
        _ => false,
    }
}

/// Mantém só as URLs de [`is_auth_callback`], na ordem.
pub fn filter_callbacks<S: AsRef<str>>(urls: &[S]) -> Vec<String> {
    urls.iter()
        .map(|s| s.as_ref())
        .filter(|u| is_auth_callback(u))
        .map(String::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aceita_callbacks_validos() {
        let aceitas = [
            "streamazap://auth/callback",
            "streamazap://auth/callback?code=abc",
            "streamazap://auth/callback#access_token=x",
            "streamazap://auth/callback/",
            "STREAMAZAP://AUTH/CALLBACK?code=1",
        ];
        for url in aceitas {
            assert!(is_auth_callback(url), "deveria aceitar: {url}");
        }
    }

    #[test]
    fn recusa_urls_fora_do_callback() {
        let recusadas = [
            "streamazap://auth/callbackx",
            "streamazap://auth/callbacks?code=1",
            "streamazap://auth/other",
            "streamazap://auth",
            "streamazap://",
            "https://streamazap.com/auth/callback",
            "http://auth/callback",
            "",
            " streamazap://auth/callback",
            "streamazap://evil/auth/callback",
            "é",
        ];
        for url in recusadas {
            assert!(!is_auth_callback(url), "deveria recusar: {url}");
        }
    }

    #[test]
    fn filter_mantem_so_callbacks_na_ordem() {
        let entrada = [
            "https://x",
            "streamazap://auth/callback?code=1",
            "streamazap://join/abc",
            "streamazap://auth/callback?code=2",
        ];
        let esperado = vec![
            "streamazap://auth/callback?code=1".to_string(),
            "streamazap://auth/callback?code=2".to_string(),
        ];
        assert_eq!(filter_callbacks(&entrada), esperado);

        let owned: Vec<String> = entrada.iter().map(|s| s.to_string()).collect();
        assert_eq!(filter_callbacks(&owned), esperado);
    }

    #[test]
    fn filter_lista_vazia_devolve_vazia() {
        let vazia: [&str; 0] = [];
        assert!(filter_callbacks(&vazia).is_empty());
    }

    #[test]
    fn convite_com_codigo_vira_codigo_normalizado() {
        let casos = [
            ("streamazap://sala/ZAP-7K2Q", "ZAP-7K2Q"),
            ("streamazap://sala/zap-7k2q", "ZAP-7K2Q"),
            ("streamazap://sala/7K2Q", "ZAP-7K2Q"),
            ("streamazap://sala/ZAP-7K2Q/", "ZAP-7K2Q"),
            ("streamazap://sala/ZAP-7K2Q?de=whatsapp", "ZAP-7K2Q"),
            ("streamazap://sala/ZAP-7K2Q#x", "ZAP-7K2Q"),
            ("STREAMAZAP://SALA/ZAP-7K2Q", "ZAP-7K2Q"),
        ];
        for (url, alvo) in casos {
            assert_eq!(room_invite(url).as_deref(), Some(alvo), "{url}");
        }
    }

    #[test]
    fn convite_com_ip_vira_ip_e_porta() {
        let casos = [
            ("streamazap://sala/26.14.7.88:47800", "26.14.7.88:47800"),
            ("streamazap://sala/26.14.7.88:5000", "26.14.7.88:5000"),
            ("streamazap://sala/26.14.7.88", "26.14.7.88:47800"),
            ("streamazap://sala/192.168.0.10/", "192.168.0.10:47800"),
        ];
        for (url, alvo) in casos {
            assert_eq!(room_invite(url).as_deref(), Some(alvo), "{url}");
        }
    }

    #[test]
    fn convite_invalido_e_recusado() {
        let recusadas = [
            "streamazap://sala/",
            "streamazap://sala",
            "streamazap://sala/ZAP-7K2O",
            "streamazap://sala/ZAP-7K2QQ",
            "streamazap://sala/ZAP-7K2Q/extra",
            "streamazap://sala/exemplo.com",
            "streamazap://sala/26.14.7.88:0",
            "streamazap://sala/26.14.7.88:99999",
            "streamazap://sala/[::1]:47800",
            "streamazap://salax/ZAP-7K2Q",
            "streamazap://auth/callback?code=1",
            "https://heitorcanhizares.github.io/streamaZap/sala/#ZAP-7K2Q",
            " streamazap://sala/ZAP-7K2Q",
            "streamazap://sala/é",
            "",
        ];
        for url in recusadas {
            assert_eq!(room_invite(url), None, "deveria recusar: {url}");
        }
    }

    #[test]
    fn filter_invites_mantem_so_convites_na_ordem() {
        let entrada = [
            "streamazap://auth/callback?code=1",
            "streamazap://sala/zap-7k2q",
            "https://x",
            "streamazap://sala/26.14.7.88",
        ];
        assert_eq!(
            filter_invites(&entrada),
            vec!["ZAP-7K2Q".to_string(), "26.14.7.88:47800".to_string()]
        );
    }
}
