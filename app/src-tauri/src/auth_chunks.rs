//! Funções puras para guardar valores grandes em credenciais pequenas.
//!
//! O Gerenciador de Credenciais do Windows aceita no máximo 2560 bytes por credencial e o keyring
//! grava o texto em UTF-16 (2 bytes por unidade), então cada pedaço leva no máximo
//! [`MAX_UNITS`] unidades UTF-16. Sem I/O.

/// Máximo de unidades UTF-16 por pedaço (1000 unidades = 2000 bytes, abaixo do limite de 2560).
pub const MAX_UNITS: usize = 1000;

/// Nome da credencial do pedaço `i` da chave `key`: `"<key>#<i>"`.
pub fn chunk_key(key: &str, i: usize) -> String {
    format!("{key}#{i}")
}

/// Divide `value` em pedaços de no máximo `max_units` unidades UTF-16, sem partir caractere.
/// Valor vazio devolve lista vazia. `max_units` menor que 2 é tratado como 2.
pub fn split_value(value: &str, max_units: usize) -> Vec<String> {
    // Mínimo 2 para sempre caber um par substituto inteiro.
    let max = max_units.max(2);
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut units = 0usize;
    for c in value.chars() {
        let len = c.len_utf16();
        if units + len > max {
            chunks.push(std::mem::take(&mut current));
            units = 0;
        }
        current.push(c);
        units += len;
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

/// Junta os pedaços na ordem recebida (inverso de [`split_value`]).
pub fn join_chunks(chunks: &[String]) -> String {
    chunks.concat()
}

/// Lê a credencial de contagem (`"3"`). Texto que não é número → `None`.
pub fn parse_count(text: &str) -> Option<usize> {
    text.trim().parse::<usize>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn units(s: &str) -> usize {
        s.encode_utf16().count()
    }

    #[test]
    fn chunk_key_formato() {
        assert_eq!(chunk_key("sb-x-auth-token", 3), "sb-x-auth-token#3");
    }

    #[test]
    fn split_vazio_da_lista_vazia() {
        assert!(split_value("", MAX_UNITS).is_empty());
    }

    #[test]
    fn split_cabe_em_um_pedaco() {
        assert_eq!(split_value("abc", 1000), vec!["abc".to_string()]);
    }

    #[test]
    fn split_6kb_ida_e_volta() {
        let valor = "a".repeat(6000);
        let pedacos = split_value(&valor, MAX_UNITS);
        assert_eq!(pedacos.len(), 6);
        assert!(pedacos.iter().all(|p| units(p) <= 1000));
        assert_eq!(join_chunks(&pedacos), valor);
    }

    #[test]
    fn split_nao_parte_caractere() {
        let valor = "😀".repeat(600);
        let pedacos = split_value(&valor, 1000);
        assert!(pedacos.iter().all(|p| units(p) <= 1000));
        assert!(pedacos.iter().all(|p| !p.is_empty() && p.chars().all(|c| c == '😀')));
        assert_eq!(join_chunks(&pedacos), valor);
    }

    #[test]
    fn split_max_pequeno_vira_2() {
        assert_eq!(split_value("abcd", 0), vec!["ab".to_string(), "cd".to_string()]);
    }

    #[test]
    fn join_vazio_e_string_vazia() {
        assert_eq!(join_chunks(&[]), "");
    }

    #[test]
    fn parse_count_casos() {
        assert_eq!(parse_count("3"), Some(3));
        assert_eq!(parse_count(" 12 "), Some(12));
        assert_eq!(parse_count(""), None);
        assert_eq!(parse_count("x"), None);
        assert_eq!(parse_count("-1"), None);
    }
}
