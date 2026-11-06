//! "Atualizado para a vX": guarda em disco o que vai ser instalado e, na abertura seguinte,
//! detecta que a atualização foi aplicada.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Nome do arquivo (dentro do diretório de dados do app).
pub const ARQUIVO_PENDENTE: &str = "update_pendente.json";

/// Gravado imediatamente antes de instalar.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Pendente {
    /// Versão que estava rodando.
    pub de: String,
    /// Versão que vai ser instalada.
    pub para: String,
    pub notas: Option<String>,
}

/// Payload do evento `zap://update-aplicada` e resposta de `update_applied_take`.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Aplicada {
    pub versao: String,
    pub anterior: String,
    pub notas: Option<String>,
}

/// Grava `<dir>/update_pendente.json`.
pub fn gravar_pendente(dir: &Path, p: &Pendente) -> io::Result<()> {
    // Cria o diretório de dados se ainda não existe (primeira instalação).
    std::fs::create_dir_all(dir)?;
    let bytes = serde_json::to_vec_pretty(p).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    std::fs::write(dir.join(ARQUIVO_PENDENTE), bytes)
}

/// Lê e APAGA `<dir>/update_pendente.json` (sempre, para não repetir). Devolve `Some` só se o
/// arquivo existe, é JSON válido e `para == versao_atual` (instalou mesmo). Arquivo ausente,
/// corrompido ou de outra versão (instalação que não aconteceu) -> `None`.
pub fn detectar(dir: &Path, versao_atual: &str) -> Option<Aplicada> {
    let caminho = dir.join(ARQUIVO_PENDENTE);
    if !caminho.exists() {
        return None;
    }
    // Apaga antes de validar: um arquivo ruim não pode ser relido na próxima abertura.
    let bytes = std::fs::read(&caminho).ok();
    let _ = std::fs::remove_file(&caminho);
    let pendente: Pendente = serde_json::from_slice(&bytes?).ok()?;
    if pendente.para.trim() != versao_atual.trim() {
        return None;
    }
    Some(Aplicada {
        versao: pendente.para,
        anterior: pendente.de,
        notas: pendente.notas,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Diretório temporário único por teste (sem `tempfile`, que o crate não tem).
    fn dir_unico(nome: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("zap-aplicada-{nome}-{}-{nanos}", std::process::id()))
    }

    fn pendente(de: &str, para: &str, notas: Option<&str>) -> Pendente {
        Pendente {
            de: de.to_string(),
            para: para.to_string(),
            notas: notas.map(str::to_string),
        }
    }

    #[test]
    fn grava_e_detecta_versao_nova() {
        let dir = dir_unico("nova");
        gravar_pendente(&dir, &pendente("3.2.1", "3.4.0", Some("Câmera nova"))).unwrap();
        assert_eq!(
            detectar(&dir, "3.4.0"),
            Some(Aplicada {
                versao: "3.4.0".into(),
                anterior: "3.2.1".into(),
                notas: Some("Câmera nova".into()),
            })
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn detectar_apaga_o_arquivo() {
        let dir = dir_unico("apaga");
        gravar_pendente(&dir, &pendente("3.2.1", "3.4.0", None)).unwrap();
        assert!(detectar(&dir, "3.4.0").is_some());
        assert!(!dir.join(ARQUIVO_PENDENTE).exists());
        assert_eq!(detectar(&dir, "3.4.0"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn versao_diferente_nao_anuncia_mas_apaga() {
        let dir = dir_unico("diferente");
        gravar_pendente(&dir, &pendente("3.2.1", "3.4.0", None)).unwrap();
        assert_eq!(detectar(&dir, "3.2.1"), None);
        assert!(!dir.join(ARQUIVO_PENDENTE).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sem_arquivo_devolve_none() {
        let dir = dir_unico("ausente");
        assert_eq!(detectar(&dir, "3.4.0"), None);
    }

    #[test]
    fn arquivo_corrompido_devolve_none_e_apaga() {
        let dir = dir_unico("corrompido");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(ARQUIVO_PENDENTE), "{ nao e json").unwrap();
        assert_eq!(detectar(&dir, "3.4.0"), None);
        assert!(!dir.join(ARQUIVO_PENDENTE).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn grava_cria_diretorio() {
        let dir = dir_unico("cria").join("aninhado");
        gravar_pendente(&dir, &pendente("3.2.1", "3.4.0", None)).unwrap();
        assert!(dir.join(ARQUIVO_PENDENTE).exists());
        let _ = std::fs::remove_dir_all(dir.parent().unwrap_or(&dir));
    }

    #[test]
    fn espacos_na_versao_sao_ignorados() {
        let dir = dir_unico("espacos");
        gravar_pendente(&dir, &pendente("3.2.1", "3.4.0\n", None)).unwrap();
        assert!(detectar(&dir, "3.4.0").is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
