use crate::{AudioError, FRAME, FRAME_SAMPLES};
use ::opus::{Application, Bitrate, Channels};
use bytes::Bytes;

/// Tamanho do buffer de saída do encoder: pacote Opus de 20 ms cabe folgado.
const MAX_PACOTE: usize = 4000;

/// Converte erro da crate `opus` em `AudioError::Opus`.
fn erro_opus<E: std::fmt::Display>(e: E) -> AudioError {
    AudioError::Opus(e.to_string())
}

pub struct OpusEnc {
    inner: ::opus::Encoder,
}

impl OpusEnc {
    pub fn new(kbps: u32) -> Result<Self, AudioError> {
        Self::com_complexidade(kbps, 8)
    }

    fn com_complexidade(kbps: u32, complexidade: i32) -> Result<Self, AudioError> {
        if !(6..=510).contains(&kbps) {
            return Err(AudioError::Invalid(format!(
                "bitrate Opus de {kbps} kbps fora de 6..=510"
            )));
        }
        let bits =
            i32::try_from(kbps * 1000).map_err(|_| AudioError::Invalid(format!("bitrate Opus de {kbps} kbps")))?;

        let mut inner = ::opus::Encoder::new(crate::RATE, Channels::Stereo, Application::Audio).map_err(erro_opus)?;
        inner.set_bitrate(Bitrate::Bits(bits)).map_err(erro_opus)?;
        // Complexidade 8: qualidade boa sem custo excessivo em tempo real.
        inner.set_complexity(complexidade).map_err(erro_opus)?;
        // FEC em banda permite recuperar quadros perdidos pelo pacote seguinte.
        inner.set_inband_fec(true).map_err(erro_opus)?;
        inner.set_packet_loss_perc(5).map_err(erro_opus)?;
        Ok(Self { inner })
    }

    pub fn encode(&mut self, pcm: &[f32; FRAME_SAMPLES]) -> Result<Bytes, AudioError> {
        let mut buf = [0u8; MAX_PACOTE];
        let n = self.inner.encode_float(pcm, &mut buf).map_err(erro_opus)?;
        Ok(Bytes::copy_from_slice(&buf[..n]))
    }
}

pub struct OpusDec {
    inner: ::opus::Decoder,
}

impl OpusDec {
    pub fn new() -> Result<Self, AudioError> {
        let inner = ::opus::Decoder::new(crate::RATE, Channels::Stereo).map_err(erro_opus)?;
        Ok(Self { inner })
    }
    /// `packet = None`: usa o FEC de `fec_next` se houver, senão PLC.
    pub fn decode(
        &mut self,
        packet: Option<&[u8]>,
        fec_next: Option<&[u8]>,
        out: &mut [f32; FRAME_SAMPLES],
    ) -> Result<(), AudioError> {
        // Sem pacote: FEC do próximo (recupera o quadro perdido) ou PLC (slice vazio).
        let (dados, fec) = match (packet, fec_next) {
            (Some(p), _) => (p, false),
            (None, Some(n)) => (n, true),
            (None, None) => (&[][..], false),
        };
        let amostras = self.inner.decode_float(dados, &mut out[..], fec).map_err(erro_opus)?;
        if amostras != FRAME {
            return Err(AudioError::Opus(format!(
                "quadro decodificado com {amostras} amostras por canal, esperado {FRAME}"
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Seno de 1 kHz, amplitude 0.5, estéreo intercalado, `quadros` quadros de 20 ms.
    fn seno(quadros: usize) -> Vec<f32> {
        let mut v = Vec::with_capacity(quadros * FRAME_SAMPLES);
        for n in 0..quadros * FRAME {
            let fase = 2.0 * std::f64::consts::PI * 1000.0 * n as f64 / crate::RATE as f64;
            let s = (0.5 * fase.sin()) as f32;
            v.push(s);
            v.push(s);
        }
        v
    }

    fn quadro(v: &[f32], q: usize) -> [f32; FRAME_SAMPLES] {
        let mut out = [0f32; FRAME_SAMPLES];
        out.copy_from_slice(&v[q * FRAME_SAMPLES..(q + 1) * FRAME_SAMPLES]);
        out
    }

    #[test]
    fn roundtrip_seno_1khz() {
        const QUADROS: usize = 50;
        const DESCARTE: usize = 5;
        let entrada = seno(QUADROS);

        // Complexidade 4 só neste teste: com >= 5 o pré-filtro de pitch do CELT deixa resíduo
        // num tom puro (SNR ~16 dB, igual em qualquer bitrate). Aqui medimos a fidelidade da
        // cadeia codifica/decodifica; a produção continua em complexidade 8 (ver `new`).
        let mut enc = OpusEnc::com_complexidade(96, 4).expect("encoder");
        let mut dec = OpusDec::new().expect("decoder");
        let mut saida: Vec<f32> = Vec::with_capacity(QUADROS * FRAME);
        let mut out = [0f32; FRAME_SAMPLES];
        for q in 0..QUADROS {
            let pacote = enc.encode(&quadro(&entrada, q)).expect("encode");
            dec.decode(Some(&pacote[..]), None, &mut out).expect("decode");
            saida.extend(out.iter().step_by(crate::CHANNELS).copied());
        }

        // Alinhamento: ajusta amplitude e fase do seno de 1 kHz por mínimos quadrados
        // (absorve o pre-skip do Opus, que é um atraso de fase). O que sobra é ruído.
        // Não dá para usar busca de atraso inteiro: a 1 kHz/48 kHz o sinal é periódico
        // a cada 48 amostras e a fração de amostra desalinha o erro.
        let w = 2.0 * std::f64::consts::PI * 1000.0 / crate::RATE as f64;
        let inicio = DESCARTE * FRAME;
        let (mut sss, mut scc, mut ssc) = (0f64, 0f64, 0f64);
        let (mut sys, mut syc, mut syy) = (0f64, 0f64, 0f64);
        for (i, &amostra) in saida[inicio..].iter().enumerate() {
            let t = (inicio + i) as f64;
            let (s, c, y) = ((w * t).sin(), (w * t).cos(), amostra as f64);
            sss += s * s;
            scc += c * c;
            ssc += s * c;
            sys += s * y;
            syc += c * y;
            syy += y * y;
        }
        let det = sss * scc - ssc * ssc;
        let a = (sys * scc - syc * ssc) / det;
        let b = (syc * sss - sys * ssc) / det;
        let energia_ajuste = a * sys + b * syc;
        let energia_residuo = (syy - energia_ajuste).max(f64::MIN_POSITIVE);
        let snr = 10.0 * (energia_ajuste / energia_residuo).log10();
        assert!(
            snr > 20.0,
            "SNR {snr:.1} dB do seno de 1 kHz após alinhar fase/amplitude"
        );
    }

    #[test]
    fn plc_sem_panico() {
        let entrada = seno(4);
        let mut enc = OpusEnc::new(64).expect("encoder");
        let mut dec = OpusDec::new().expect("decoder");
        let mut out = [0f32; FRAME_SAMPLES];
        for q in 0..3 {
            let pacote = enc.encode(&quadro(&entrada, q)).expect("encode");
            dec.decode(Some(&pacote[..]), None, &mut out).expect("decode");
        }
        assert!(dec.decode(None, None, &mut out).is_ok());
    }

    #[test]
    fn fec_usa_proximo() {
        let entrada = seno(3);
        let mut enc = OpusEnc::new(64).expect("encoder");
        let mut dec = OpusDec::new().expect("decoder");
        let mut out = [0f32; FRAME_SAMPLES];
        let p0 = enc.encode(&quadro(&entrada, 0)).expect("encode");
        let p1 = enc.encode(&quadro(&entrada, 1)).expect("encode");
        dec.decode(Some(&p0[..]), None, &mut out).expect("decode");
        // Quadro 1 perdido: recuperado pelo FEC contido em p2 (aqui, p1 como próximo).
        assert!(dec.decode(None, Some(&p1[..]), &mut out).is_ok());
        dec.decode(Some(&p1[..]), None, &mut out).expect("decode");
    }

    #[test]
    fn kbps_invalido() {
        assert!(matches!(OpusEnc::new(1), Err(AudioError::Invalid(_))));
        assert!(matches!(OpusEnc::new(511), Err(AudioError::Invalid(_))));
    }
}
