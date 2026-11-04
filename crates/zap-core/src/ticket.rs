//! Ingresso de mídia do modo Local (docs/api/mensagens.md, "Modo Local, mídia de outro
//! participante"): `ingresso = base64(HMAC-SHA256(chave = segredo_midia, msg = participante_id))`.
use base64::{engine::general_purpose::STANDARD, Engine as _};
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// 32 bytes aleatórios em base64.
pub fn new_media_secret() -> String {
    STANDARD.encode(rand::random::<[u8; 32]>())
}

/// Chave do HMAC: os bytes decodificados do segredo. Se o texto não for base64 válido,
/// usa os bytes UTF-8 do próprio texto (nunca falha).
fn chave(secret_b64: &str) -> Vec<u8> {
    STANDARD
        .decode(secret_b64)
        .unwrap_or_else(|_| secret_b64.as_bytes().to_vec())
}

/// `segredo_b64`: o segredo em base64 (a chave do HMAC são os bytes decodificados).
pub fn make_ticket(secret_b64: &str, participant_id: &str) -> String {
    // HMAC aceita chave de qualquer tamanho, então `new_from_slice` não falha na prática.
    // O `None` é só para evitar `expect`; nesse caso devolve texto vazio, que nunca verifica.
    let Ok(mut mac) = HmacSha256::new_from_slice(&chave(secret_b64)) else {
        return String::new();
    };
    mac.update(participant_id.as_bytes());
    STANDARD.encode(mac.finalize().into_bytes())
}

/// Verdadeiro se `ticket` é o ingresso (comparação em tempo constante) de algum dos `participant_ids`.
pub fn verify_ticket(secret_b64: &str, participant_ids: &[String], ticket: &str) -> bool {
    let Ok(recebido) = STANDARD.decode(ticket) else {
        return false;
    };
    let chave = chave(secret_b64);
    participant_ids.iter().any(|id| {
        let Ok(mut mac) = HmacSha256::new_from_slice(&chave) else {
            return false;
        };
        mac.update(id.as_bytes());
        // `verify_slice` compara em tempo constante e recusa tamanhos diferentes.
        mac.verify_slice(&recebido).is_ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn ticket_deterministico() {
        let s = new_media_secret();
        let a = make_ticket(&s, "alice");
        assert_eq!(a, make_ticket(&s, "alice"));
        assert_ne!(a, make_ticket(&s, "bob"));
        let outro = new_media_secret();
        assert_ne!(a, make_ticket(&outro, "alice"));
    }

    #[test]
    fn verify_aceita_membro_da_lista() {
        let s = new_media_secret();
        let t = make_ticket(&s, "bob");
        assert!(verify_ticket(&s, &ids(&["alice", "bob"]), &t));
        assert!(verify_ticket(&s, &ids(&["bob"]), &t));
    }

    #[test]
    fn verify_recusa_estranho_e_vazio_e_lixo() {
        let s = new_media_secret();
        let t = make_ticket(&s, "bob");
        // Participante fora da lista.
        assert!(!verify_ticket(&s, &ids(&["alice", "carol"]), &t));
        // Lista vazia.
        assert!(!verify_ticket(&s, &[], &t));
        // Ingresso de outro segredo.
        assert!(!verify_ticket(&new_media_secret(), &ids(&["bob"]), &t));
        // Lixo: não-base64, vazio e tamanho errado.
        assert!(!verify_ticket(&s, &ids(&["bob"]), "@@@"));
        assert!(!verify_ticket(&s, &ids(&["bob"]), ""));
        assert!(!verify_ticket(&s, &ids(&["bob"]), &STANDARD.encode([0u8; 5])));
    }

    #[test]
    fn segredo_novo_tem_32_bytes() {
        let a = new_media_secret();
        let b = new_media_secret();
        assert_eq!(STANDARD.decode(&a).map(|v| v.len()).ok(), Some(32));
        assert_ne!(a, b);
    }

    #[test]
    fn vetor_conhecido() {
        // Segredo = base64 de 32 bytes 0x0b; id "abc". Valor calculado aqui, independentemente.
        let segredo = STANDARD.encode([0x0bu8; 32]);
        let mut mac = Hmac::<Sha256>::new_from_slice(&[0x0bu8; 32]).ok().unwrap_or_else(|| {
            panic!("chave de teste inválida");
        });
        mac.update(b"abc");
        let esperado = STANDARD.encode(mac.finalize().into_bytes());
        assert_eq!(make_ticket(&segredo, "abc"), esperado);
        assert!(verify_ticket(&segredo, &ids(&["abc"]), &esperado));
    }
}
