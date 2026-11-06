//! Dados públicos de um vídeo do YouTube pelo oEmbed (sem chave de API), para a música da sala.

use std::time::Duration;

use serde::Deserialize;

const OEMBED: &str = "https://www.youtube.com/oembed";
const PRAZO: Duration = Duration::from_secs(5);

/// Título e canal do vídeo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoInfo {
    pub title: String,
    pub channel: Option<String>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum OembedError {
    /// O dono do vídeo não deixa tocar fora do YouTube, ou o vídeo é privado.
    #[error("esse vídeo não pode tocar fora do YouTube (ou é privado)")]
    NotEmbeddable,
    #[error("vídeo não encontrado")]
    NotFound,
    /// Rede fora ou resposta estranha: não dá para saber se o vídeo toca.
    #[error("não deu para consultar o YouTube: {0}")]
    Network(String),
}

/// Id de vídeo do YouTube: 11 caracteres de `[A-Za-z0-9_-]`.
pub fn is_video_id(id: &str) -> bool {
    id.len() == 11 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Consulta o oEmbed do vídeo.
pub async fn oembed(video_id: &str) -> Result<VideoInfo, OembedError> {
    if !is_video_id(video_id) {
        return Err(OembedError::NotFound);
    }
    let url = format!("{OEMBED}?format=json&url=https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3D{video_id}");
    let cliente = reqwest::Client::builder()
        .timeout(PRAZO)
        .build()
        .map_err(|e| OembedError::Network(e.to_string()))?;
    let resposta = cliente
        .get(url)
        .send()
        .await
        .map_err(|e| OembedError::Network(e.to_string()))?;
    let status = resposta.status().as_u16();
    let corpo = resposta.text().await.unwrap_or_default();
    interpret(status, &corpo)
}

#[derive(Deserialize)]
struct Resposta {
    title: String,
    #[serde(default)]
    author_name: Option<String>,
}

/// Lê a resposta do oEmbed: 200 = dados; 401/403 = não deixa embutir (ou privado); 400/404 = não
/// existe.
fn interpret(status: u16, corpo: &str) -> Result<VideoInfo, OembedError> {
    match status {
        200 => {
            let r: Resposta = serde_json::from_str(corpo).map_err(|e| OembedError::Network(e.to_string()))?;
            let title = r.title.trim().to_string();
            if title.is_empty() {
                return Err(OembedError::Network("resposta sem título".into()));
            }
            let channel = r.author_name.map(|c| c.trim().to_string()).filter(|c| !c.is_empty());
            Ok(VideoInfo { title, channel })
        }
        401 | 403 => Err(OembedError::NotEmbeddable),
        400 | 404 => Err(OembedError::NotFound),
        outro => Err(OembedError::Network(format!("o YouTube respondeu {outro}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_de_video() {
        assert!(is_video_id("dQw4w9WgXcQ"));
        assert!(is_video_id("a-b_c1234XY"));
        assert!(!is_video_id("dQw4w9WgXc"), "10 caracteres");
        assert!(!is_video_id("dQw4w9WgXcQQ"), "12 caracteres");
        assert!(!is_video_id("dQw4w9WgX&Q"), "caractere de URL");
        assert!(!is_video_id(""));
    }

    #[test]
    fn resposta_ok() {
        let corpo = r#"{"title":" Never Gonna Give You Up ","author_name":"Rick Astley","type":"video"}"#;
        assert_eq!(
            interpret(200, corpo),
            Ok(VideoInfo {
                title: "Never Gonna Give You Up".into(),
                channel: Some("Rick Astley".into()),
            })
        );
        let sem_canal = r#"{"title":"Live","author_name":"  "}"#;
        assert_eq!(interpret(200, sem_canal).map(|v| v.channel), Ok(None));
    }

    #[test]
    fn respostas_de_erro() {
        assert_eq!(interpret(401, "Unauthorized"), Err(OembedError::NotEmbeddable));
        assert_eq!(interpret(403, ""), Err(OembedError::NotEmbeddable));
        assert_eq!(interpret(404, "Not Found"), Err(OembedError::NotFound));
        assert_eq!(interpret(400, "Bad Request"), Err(OembedError::NotFound));
        assert!(matches!(interpret(500, ""), Err(OembedError::Network(_))));
        assert!(matches!(interpret(200, "não é json"), Err(OembedError::Network(_))));
        assert!(matches!(
            interpret(200, r#"{"title":""}"#),
            Err(OembedError::Network(_))
        ));
    }

    #[tokio::test]
    async fn id_invalido_nem_consulta() {
        assert_eq!(oembed("x").await, Err(OembedError::NotFound));
    }
}
