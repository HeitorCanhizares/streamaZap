//! Moldura do stream de controle: `u32` big-endian com o tamanho + JSON.
use crate::control::ControlMsg;
use crate::error::ProtoError;
use crate::version::MAX_CONTROL_MESSAGE;

/// Serializa a mensagem como `[len u32 BE][json]`.
pub fn encode_control(msg: &ControlMsg) -> Result<Vec<u8>, ProtoError> {
    let json = serde_json::to_vec(msg)?;
    if json.len() > MAX_CONTROL_MESSAGE {
        return Err(ProtoError::TooLarge(json.len()));
    }
    let mut out = Vec::with_capacity(4 + json.len());
    out.extend_from_slice(&(json.len() as u32).to_be_bytes());
    out.extend_from_slice(&json);
    Ok(out)
}

#[derive(Default)]
pub struct ControlDecoder {
    pub(crate) buf: Vec<u8>,
}

impl ControlDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// `Ok(None)` = falta byte.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<Option<ControlMsg>, ProtoError> {
        if self.buf.len() < 4 {
            return Ok(None);
        }
        let len = u32::from_be_bytes([self.buf[0], self.buf[1], self.buf[2], self.buf[3]]) as usize;
        if len > MAX_CONTROL_MESSAGE {
            // Stream corrompido: não dá para ressincronizar, descarta tudo.
            self.buf.clear();
            return Err(ProtoError::TooLarge(len));
        }
        if self.buf.len() < 4 + len {
            return Ok(None);
        }
        // Remove o frame antes de interpretar, para o decoder sempre avançar.
        let frame: Vec<u8> = self.buf.drain(..4 + len).skip(4).collect();
        match serde_json::from_slice::<ControlMsg>(&frame) {
            Ok(msg) => Ok(Some(msg)),
            Err(e) => {
                // Distingue tipo desconhecido de campos errados pela mensagem do
                // serde ("unknown variant"): só o primeiro vira UnknownMessage.
                if e.to_string().contains("unknown variant") {
                    if let Ok(serde_json::Value::Object(map)) = serde_json::from_slice::<serde_json::Value>(&frame) {
                        if let Some(serde_json::Value::String(t)) = map.get("t") {
                            return Err(ProtoError::UnknownMessage(t.clone()));
                        }
                    }
                }
                Err(ProtoError::Json(e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ping() -> ControlMsg {
        ControlMsg::Ping { ts_ms: 42 }
    }

    fn frame(body: &[u8]) -> Vec<u8> {
        let mut v = (body.len() as u32).to_be_bytes().to_vec();
        v.extend_from_slice(body);
        v
    }

    #[test]
    fn roundtrip_ping() {
        let mut d = ControlDecoder::new();
        d.push(&encode_control(&ping()).unwrap());
        assert_eq!(d.next().unwrap(), Some(ping()));
        assert_eq!(d.next().unwrap(), None);
    }

    #[test]
    fn prefixo_big_endian() {
        let b = encode_control(&ping()).unwrap();
        let json = serde_json::to_vec(&ping()).unwrap();
        assert_eq!(b[..4], (json.len() as u32).to_be_bytes());
        assert_eq!(&b[4..], &json[..]);
    }

    #[test]
    fn fragmentado() {
        let b = encode_control(&ping()).unwrap();
        let mut d = ControlDecoder::new();
        for (i, byte) in b.iter().enumerate() {
            d.push(&[*byte]);
            let r = d.next().unwrap();
            if i + 1 < b.len() {
                assert!(r.is_none());
            } else {
                assert_eq!(r, Some(ping()));
            }
        }
    }

    #[test]
    fn duas_mensagens_num_push() {
        let mut b = encode_control(&ping()).unwrap();
        b.extend(encode_control(&ControlMsg::Pong { ts_ms: 7 }).unwrap());
        let mut d = ControlDecoder::new();
        d.push(&b);
        assert_eq!(d.next().unwrap(), Some(ping()));
        assert_eq!(d.next().unwrap(), Some(ControlMsg::Pong { ts_ms: 7 }));
        assert_eq!(d.next().unwrap(), None);
    }

    #[test]
    fn grande_demais_envio() {
        let m = ControlMsg::error("x", "a".repeat(70_000), None);
        assert!(matches!(encode_control(&m), Err(ProtoError::TooLarge(_))));
    }

    #[test]
    fn grande_demais_recepcao() {
        let mut d = ControlDecoder::new();
        d.push(&[0, 1, 0, 1]);
        assert!(matches!(d.next(), Err(ProtoError::TooLarge(65537))));
        assert!(d.buf.is_empty());
        d.push(&encode_control(&ping()).unwrap());
        assert_eq!(d.next().unwrap(), Some(ping()));
    }

    #[test]
    fn desconhecida_e_segue() {
        let mut d = ControlDecoder::new();
        d.push(&frame(br#"{"t":"zzz","a":1}"#));
        d.push(&encode_control(&ping()).unwrap());
        match d.next() {
            Err(ProtoError::UnknownMessage(t)) => assert_eq!(t, "zzz"),
            other => panic!("inesperado: {other:?}"),
        }
        assert_eq!(d.next().unwrap(), Some(ping()));
    }

    #[test]
    fn json_quebrado() {
        let mut d = ControlDecoder::new();
        d.push(&frame(b"abc"));
        d.push(&encode_control(&ping()).unwrap());
        assert!(matches!(d.next(), Err(ProtoError::Json(_))));
        assert_eq!(d.next().unwrap(), Some(ping()));
    }

    #[test]
    fn campos_errados_nao_e_desconhecida() {
        let mut d = ControlDecoder::new();
        d.push(&frame(br#"{"t":"ping"}"#));
        assert!(matches!(d.next(), Err(ProtoError::Json(_))));
    }

    #[test]
    fn vazio() {
        assert_eq!(ControlDecoder::new().next().unwrap(), None);
    }
}
