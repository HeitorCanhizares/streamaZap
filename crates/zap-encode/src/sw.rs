//! Backend de software: MFT síncrono (`CLSID_MSH264EncoderMFT`), entrada em memória.
use crate::backend::{SendDevice, WorkerIo};
use crate::enumerate::Candidate;
use crate::{codec_api, logic, media_types, nal, sample};
use crate::{EncodeError, EncodedPicture, EncoderConfig, EncoderInfo};
use bytes::Bytes;
use std::collections::VecDeque;
use std::mem::ManuallyDrop;
use std::time::{Duration, Instant};
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Texture2D, D3D11_CPU_ACCESS_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_STAGING,
};
use windows::Win32::Media::MediaFoundation::{
    ICodecAPI, IMFSample, IMFTransform, MFCreateMemoryBuffer, MFCreateSample, MFT_MESSAGE_COMMAND_FLUSH,
    MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, MFT_MESSAGE_NOTIFY_END_OF_STREAM, MFT_MESSAGE_NOTIFY_START_OF_STREAM,
    MFT_OUTPUT_DATA_BUFFER, MFT_OUTPUT_STREAM_PROVIDES_SAMPLES, MF_E_TRANSFORM_NEED_MORE_INPUT,
    MF_E_TRANSFORM_STREAM_CHANGE,
};

pub(crate) struct SwEncoder {
    info: EncoderInfo,
    mft: IMFTransform,
    codec: ICodecAPI,
    seq_header: Vec<u8>,
    /// Tamanho de buffer de saída pedido pelo MFT.
    out_size: u32,
    /// O MFT aloca as próprias amostras de saída.
    provides_samples: bool,
}

impl SwEncoder {
    /// Ativa o MFT do candidato, tipos de saída e entrada (NV12 em memória), `apply_settings`
    /// (opções recusadas ignoradas), `NOTIFY_BEGIN_STREAMING`/`START_OF_STREAM`.
    pub(crate) fn open(cfg: &EncoderConfig, cand: Candidate) -> Result<Self, EncodeError> {
        // SAFETY: o ativador é um objeto COM válido; a thread tem COM/MF inicializados.
        let mft: IMFTransform =
            unsafe { cand.activate.ActivateObject() }.map_err(|e| EncodeError::os("ActivateObject", &e))?;

        // Saída antes da entrada: exigência dos MFTs de codificação.
        let saida = media_types::make_output_type(cfg)?;
        // SAFETY: tipo de mídia válido.
        unsafe { mft.SetOutputType(0, &saida, 0) }.map_err(|e| EncodeError::os("SetOutputType", &e))?;
        let entrada = media_types::make_input_type(cfg)?;
        // SAFETY: tipo de mídia válido.
        unsafe { mft.SetInputType(0, &entrada, 0) }.map_err(|e| EncodeError::os("SetInputType", &e))?;

        let codec: ICodecAPI = mft.cast().map_err(|e| EncodeError::os("ICodecAPI", &e))?;
        // O MFT de software aceita menos opções: qualquer erro é só logado.
        if let Err(e) = codec_api::apply_settings(&codec, cfg) {
            tracing::warn!("apply_settings no MFT de software: {e}");
        }

        // SAFETY: o MFT já tem os dois tipos definidos.
        let tipo_saida =
            unsafe { mft.GetOutputCurrentType(0) }.map_err(|e| EncodeError::os("GetOutputCurrentType", &e))?;
        let seq_header = media_types::read_sequence_header(&tipo_saida);

        // SAFETY: MFT válido.
        let si = unsafe { mft.GetOutputStreamInfo(0) }.map_err(|e| EncodeError::os("GetOutputStreamInfo", &e))?;
        let provides_samples = si.dwFlags & MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 as u32 != 0;

        // SAFETY: mensagens sem ponteiros.
        unsafe {
            mft.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)
                .map_err(|e| EncodeError::os("BEGIN_STREAMING", &e))?;
            mft.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)
                .map_err(|e| EncodeError::os("START_OF_STREAM", &e))?;
        }

        Ok(Self {
            info: cand.info,
            mft,
            codec,
            seq_header,
            out_size: si.cbSize,
            provides_samples,
        })
    }
    pub(crate) fn info(&self) -> &EncoderInfo {
        &self.info
    }

    /// Cria a textura staging NV12 de leitura com o tamanho pedido.
    fn criar_staging(dev: &SendDevice, w: u32, h: u32) -> Result<ID3D11Texture2D, EncodeError> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: w,
            Height: h,
            MipLevels: 1,
            ArraySize: 1,
            Format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_NV12,
            SampleDesc: windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };
        let mut tex: Option<ID3D11Texture2D> = None;
        // SAFETY: `desc` e `tex` são válidos durante a chamada; o dispositivo é válido.
        unsafe { dev.device.CreateTexture2D(&desc, None, Some(&mut tex)) }
            .map_err(|e| EncodeError::os("CreateTexture2D (staging)", &e))?;
        tex.ok_or_else(|| EncodeError::Stopped("staging nula após sucesso".into()))
    }

    /// Copia a textura NV12 para `buf` (w*h*3/2 bytes, sem padding de linha).
    fn ler_nv12(
        dev: &SendDevice,
        staging: &ID3D11Texture2D,
        origem: &ID3D11Texture2D,
        w: usize,
        h: usize,
        buf: &mut Vec<u8>,
    ) -> Result<(), EncodeError> {
        // SAFETY: ambos os recursos são do mesmo dispositivo, com formato e tamanho iguais.
        unsafe { dev.context.CopyResource(staging, origem) };
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: `mapped` é válido para escrita; a staging tem acesso de leitura.
        unsafe { dev.context.Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)) }
            .map_err(|e| EncodeError::os("Map (staging)", &e))?;
        let pitch = mapped.RowPitch as usize;
        let base = mapped.pData as *const u8;
        buf.clear();
        if base.is_null() || pitch < w {
            // SAFETY: o Map acima teve sucesso.
            unsafe { dev.context.Unmap(staging, 0) };
            return Err(EncodeError::Stopped("mapeamento NV12 inválido".into()));
        }
        // Y tem `h` linhas; UV tem `h/2` linhas logo depois, em `RowPitch * h`.
        for r in 0..(h + h / 2) {
            // SAFETY: o mapeamento cobre `pitch` bytes por linha (>= w) em h + h/2 linhas.
            let src = unsafe { std::slice::from_raw_parts(base.add(r * pitch), w) };
            buf.extend_from_slice(src);
        }
        // SAFETY: o subrecurso foi mapeado acima e os ponteiros não são mais usados.
        unsafe { dev.context.Unmap(staging, 0) };
        Ok(())
    }

    /// Um `ProcessOutput`. `Ok(None)` = precisa de mais entrada (ou mudança de tipo tratada).
    /// `Ok(Some(None))` = continue puxando sem saída útil.
    fn puxar_saida(&mut self, cfg: &EncoderConfig) -> Result<Option<Option<sample::OutputData>>, EncodeError> {
        let amostra: Option<IMFSample> = if self.provides_samples {
            None
        } else {
            // SAFETY: chamadas de criação sem ponteiros de entrada.
            let s = unsafe { MFCreateSample() }.map_err(|e| EncodeError::os("MFCreateSample", &e))?;
            let b = unsafe { MFCreateMemoryBuffer(self.out_size.max(1)) }
                .map_err(|e| EncodeError::os("MFCreateMemoryBuffer", &e))?;
            // SAFETY: `s` e `b` válidos.
            unsafe { s.AddBuffer(&b) }.map_err(|e| EncodeError::os("AddBuffer", &e))?;
            Some(s)
        };
        let mut buf = [MFT_OUTPUT_DATA_BUFFER {
            dwStreamID: 0,
            pSample: ManuallyDrop::new(amostra),
            dwStatus: 0,
            pEvents: ManuallyDrop::new(None),
        }];
        let mut status: u32 = 0;
        // SAFETY: `buf` e `status` são locais válidos durante a chamada.
        let r = unsafe { self.mft.ProcessOutput(0, &mut buf, &mut status) };
        let [mut item] = buf;
        // SAFETY: cada campo é tirado uma única vez e não é usado depois.
        let amostra = unsafe { ManuallyDrop::take(&mut item.pSample) };
        let _eventos = unsafe { ManuallyDrop::take(&mut item.pEvents) };
        match r {
            Ok(()) => match amostra {
                Some(s) => Ok(Some(Some(sample::read_output(&s)?))),
                None => Ok(Some(None)),
            },
            Err(e) if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => Ok(None),
            Err(e) if e.code() == MF_E_TRANSFORM_STREAM_CHANGE => {
                // SAFETY: MFT válido.
                match unsafe { self.mft.GetOutputAvailableType(0, 0) } {
                    Ok(t) => {
                        // SAFETY: tipo válido.
                        match unsafe { self.mft.SetOutputType(0, &t, 0) } {
                            Ok(()) => self.seq_header = media_types::read_sequence_header(&t),
                            Err(e) => tracing::warn!("SetOutputType após STREAM_CHANGE falhou: {e}"),
                        }
                    }
                    Err(e) => tracing::warn!("GetOutputAvailableType falhou: {e}"),
                }
                let _ = cfg;
                // SAFETY: MFT válido.
                if let Ok(si) = unsafe { self.mft.GetOutputStreamInfo(0) } {
                    self.out_size = si.cbSize;
                    self.provides_samples = si.dwFlags & MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 as u32 != 0;
                }
                Ok(Some(None))
            }
            Err(e) => Err(EncodeError::os("ProcessOutput", &e)),
        }
    }

    /// Para cada `Job`: copia a textura NV12 para staging (`CopyResource` + `Map`, sem padding de
    /// linha) usando `dev`, `ProcessInput` e `ProcessOutput` em laço até `MF_E_TRANSFORM_NEED_INPUT`.
    pub(crate) fn run(mut self, dev: SendDevice, io: WorkerIo) {
        use std::sync::atomic::Ordering;
        let cfg = io.cfg.clone();
        let (w, h) = (cfg.width as usize, cfg.height as usize);
        let mut staging: Option<ID3D11Texture2D> = None;
        let mut buf: Vec<u8> = Vec::with_capacity(w * h * 3 / 2);
        // (hns, capture_us, instante do recebimento do Job)
        let mut enviados: VecDeque<(i64, u64, Instant)> = VecDeque::new();

        'principal: while !io.shared.stop.load(Ordering::Acquire) {
            let job = match io.rx.recv_timeout(Duration::from_millis(100)) {
                Ok(j) => j,
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
            };
            let recebido = Instant::now();

            let kbps = io.shared.pending_kbps.swap(0, Ordering::AcqRel);
            if kbps != 0 {
                if let Err(e) = codec_api::set_bitrate(&self.codec, kbps) {
                    tracing::warn!("MFT de software recusou o bitrate em execução: {e}");
                }
            }
            if job.force_keyframe {
                codec_api::force_keyframe(&self.codec);
            }

            if staging.is_none() {
                match Self::criar_staging(&dev, cfg.width, cfg.height) {
                    Ok(s) => staging = Some(s),
                    Err(e) => {
                        tracing::warn!("quadro descartado, staging falhou: {e}");
                        continue;
                    }
                }
            }
            let Some(st) = staging.as_ref() else { continue };
            if let Err(e) = Self::ler_nv12(&dev, st, job.nv12.raw(), w, h, &mut buf) {
                tracing::warn!("quadro descartado, leitura NV12 falhou: {e}");
                continue;
            }
            let amostra = match sample::input_from_memory(&buf, job.capture_us, cfg.fps) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("quadro descartado, amostra de entrada falhou: {e}");
                    continue;
                }
            };
            // SAFETY: amostra válida; o MFT síncrono aceita entrada.
            if let Err(e) = unsafe { self.mft.ProcessInput(0, &amostra, 0) } {
                tracing::warn!("ProcessInput falhou, quadro descartado: {e}");
                continue;
            }
            enviados.push_back((logic::us_to_hns(job.capture_us), job.capture_us, recebido));
            while enviados.len() > 16 {
                enviados.pop_front();
            }

            loop {
                match self.puxar_saida(&cfg) {
                    Ok(None) => break,
                    Ok(Some(None)) => continue,
                    Ok(Some(Some(saida))) => {
                        let mut data = saida.data;
                        if saida.clean_point {
                            data = nal::ensure_params(&data, &self.seq_header).0;
                        }
                        let has_params = nal::has_sps_pps(&data);
                        let (capture_us, inicio) = match enviados.iter().position(|(t, _, _)| *t == saida.time_hns) {
                            Some(i) => {
                                let (_, c, ini) = enviados[i];
                                enviados.drain(..=i);
                                (c, ini)
                            }
                            None => (logic::hns_to_us(saida.time_hns), recebido),
                        };
                        let encode_us = u32::try_from(inicio.elapsed().as_micros()).unwrap_or(u32::MAX);
                        let pic = EncodedPicture {
                            data: Bytes::from(data),
                            keyframe: saida.clean_point,
                            has_params,
                            capture_us,
                            encode_us,
                        };
                        if io.out.send(pic).is_err() {
                            break 'principal;
                        }
                    }
                    Err(e) => {
                        tracing::warn!("falha ao ler a saída do MFT de software: {e}");
                        break;
                    }
                }
            }
        }

        // SAFETY: mensagens sem ponteiros; erros no encerramento só são logados.
        unsafe {
            if let Err(e) = self.mft.ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0) {
                tracing::debug!("END_OF_STREAM: {e}");
            }
            if let Err(e) = self.mft.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0) {
                tracing::debug!("FLUSH: {e}");
            }
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::enumerate::enumerate;
    use crate::mf::MfGuard;
    use crate::EncoderChoice;

    #[test]
    fn open_software_ou_pula() {
        let Ok(_com) = zap_gpu::ComGuard::new() else {
            eprintln!("sem COM: pulando");
            return;
        };
        let Ok(_mf) = MfGuard::acquire() else {
            eprintln!("sem Media Foundation: pulando");
            return;
        };
        let mut lista = match enumerate(false) {
            Ok(l) if !l.is_empty() => l,
            _ => {
                eprintln!("sem MFT de software: pulando");
                return;
            }
        };
        let cfg = EncoderConfig {
            width: 640,
            height: 360,
            fps: 30,
            kbps: 1000,
            gop_frames: 300,
            choice: EncoderChoice::Software,
        };
        let cand = lista.remove(0);
        match SwEncoder::open(&cfg, cand) {
            Ok(enc) => assert!(!enc.info().hardware),
            Err(e) => eprintln!("MFT de software não abriu ({e}): pulando"),
        }
    }
}
