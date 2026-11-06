//! Deep link `streamazap://auth/callback` (volta do OAuth): filtro puro + ligação com o plugin.

/// Só URLs que começam assim chegam à interface.
pub const CALLBACK_PREFIX: &str = "streamazap://auth/callback";

/// Evento emitido para a interface; o payload é a URL (texto), um evento por URL.
pub const EVENT: &str = "zap://deep-link";

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
}
