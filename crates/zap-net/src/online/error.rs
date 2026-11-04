//! Erros da API (códigos de `docs/api/README.md#erros`).

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApiError {
    /// Resposta de erro do servidor: `{"erro": code, "mensagem": message}`.
    #[error("{code}: {message}")]
    Http { status: u16, code: String, message: String },
    /// Falha de rede (DNS, TLS, conexão, tempo esgotado, WebSocket fechado).
    #[error("rede: {0}")]
    Network(String),
    /// Resposta fora do contrato (JSON inválido, URL inválida).
    #[error("protocolo: {0}")]
    Protocol(String),
}

impl ApiError {
    /// Interpreta o corpo de uma resposta de erro HTTP. Corpo que não seja o JSON esperado vira
    /// `Http { code: "interno", message: <texto do corpo ou "erro HTTP <status>"> }`.
    pub(crate) fn from_body(status: u16, body: &[u8]) -> ApiError {
        // Campos extras (protocolo_min/max, campo, tentar_em_ms) são ignorados de propósito.
        // `erro` precisa ser texto; `mensagem` ausente ou não-texto vira vazio.
        let parsed: Option<(String, String)> = serde_json::from_slice::<serde_json::Value>(body).ok().and_then(|v| {
            let codigo = v.get("erro")?.as_str()?.to_string();
            let mensagem = v.get("mensagem").and_then(|m| m.as_str()).unwrap_or("").to_string();
            Some((codigo, mensagem))
        });
        if let Some((code, message)) = parsed {
            return ApiError::Http { status, code, message };
        }
        // Fora do contrato: guarda o texto do corpo para diagnóstico, ou um padrão se vazio.
        let texto = String::from_utf8_lossy(body).trim().to_string();
        let message = if texto.is_empty() {
            format!("erro HTTP {status}")
        } else {
            texto
        };
        ApiError::Http {
            status,
            code: "interno".to_string(),
            message,
        }
    }

    /// Código do servidor (`Http`), se houver.
    pub fn code(&self) -> Option<&str> {
        match self {
            ApiError::Http { code, .. } => Some(code),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ApiError;

    #[test]
    fn parse_senha_sala_403() {
        let e = ApiError::from_body(403, br#"{"erro":"senha_invalida","mensagem":"Senha incorreta."}"#);
        assert_eq!(
            e,
            ApiError::Http {
                status: 403,
                code: "senha_invalida".into(),
                message: "Senha incorreta.".into(),
            }
        );
        assert_eq!(e.code(), Some("senha_invalida"));
    }

    #[test]
    fn parse_protocolo_incompativel_com_extras_426() {
        let corpo = br#"{"erro":"protocolo_incompativel","mensagem":"Atualize o app.",
            "protocolo_min":3,"protocolo_max":4,"campo":"versao","tentar_em_ms":500}"#;
        let e = ApiError::from_body(426, corpo);
        assert_eq!(
            e,
            ApiError::Http {
                status: 426,
                code: "protocolo_incompativel".into(),
                message: "Atualize o app.".into(),
            }
        );
    }

    #[test]
    fn corpo_nao_json_vira_interno() {
        let e = ApiError::from_body(502, "  Bad Gateway  \n".as_bytes());
        assert_eq!(
            e,
            ApiError::Http {
                status: 502,
                code: "interno".into(),
                message: "Bad Gateway".into()
            }
        );
    }

    #[test]
    fn corpo_vazio_usa_mensagem_padrao() {
        let e = ApiError::from_body(500, b"");
        assert_eq!(
            e,
            ApiError::Http {
                status: 500,
                code: "interno".into(),
                message: "erro HTTP 500".into()
            }
        );
        let so_espacos = ApiError::from_body(503, b" \n\t ");
        assert_eq!(
            so_espacos,
            ApiError::Http {
                status: 503,
                code: "interno".into(),
                message: "erro HTTP 503".into()
            }
        );
    }

    #[test]
    fn code_helper_retorna_none_para_network() {
        assert_eq!(ApiError::Network("timeout".into()).code(), None);
        assert_eq!(ApiError::Protocol("x".into()).code(), None);
    }
}
