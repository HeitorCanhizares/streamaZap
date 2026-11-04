//! Backend de hardware: MFT assíncrono com `IMFDXGIDeviceManager`.
use crate::backend::{SendDevice, WorkerIo};
use crate::enumerate::Candidate;
use crate::{codec_api, logic, media_types, nal, sample};
use crate::{EncodeError, EncodedPicture, EncoderConfig, EncoderInfo};
use bytes::Bytes;
use std::collections::VecDeque;
use std::mem::ManuallyDrop;
use std::time::{Duration, Instant};
use windows::core::Interface;
use windows::Win32::Media::MediaFoundation::{
    ICodecAPI, IMFDXGIDeviceManager, IMFMediaEventGenerator, IMFSample, IMFTransform, MEError,
    METransformDrainComplete, METransformHaveOutput, METransformNeedInput, MFCreateDXGIDeviceManager,
    MFCreateMemoryBuffer, MFCreateSample, MFT_MESSAGE_COMMAND_FLUSH, MFT_MESSAGE_NOTIFY_BEGIN_STREAMING,
    MFT_MESSAGE_NOTIFY_END_OF_STREAM, MFT_MESSAGE_NOTIFY_START_OF_STREAM, MFT_MESSAGE_SET_D3D_MANAGER,
    MFT_OUTPUT_DATA_BUFFER, MFT_OUTPUT_STREAM_PROVIDES_SAMPLES, MF_EVENT_FLAG_NO_WAIT, MF_E_NO_EVENTS_AVAILABLE,
    MF_E_TRANSFORM_NEED_MORE_INPUT, MF_E_TRANSFORM_STREAM_CHANGE, MF_TRANSFORM_ASYNC_UNLOCK,
};

/// Teto da espera por um quadro quando o MFT já pediu entrada: não deixa a saída pronta
/// parada muito tempo atrás do `recv_timeout`.
const ESPERA_MAX_MS: u64 = 5;

pub(crate) struct HwEncoder {
    info: EncoderInfo,
    mft: IMFTransform,
    codec: ICodecAPI,
    event_gen: IMFMediaEventGenerator,
    /// Precisa viver tanto quanto o MFT.
    _mgr: IMFDXGIDeviceManager,
    seq_header: Vec<u8>,
}

impl HwEncoder {
    /// Ativa o MFT do candidato (`IMFActivate::ActivateObject`), destrava (`MF_TRANSFORM_ASYNC_UNLOCK`),
    /// cria `IMFDXGIDeviceManager` (`MFCreateDXGIDeviceManager` + `ResetDevice(dev.device)`),
    /// `MFT_MESSAGE_SET_D3D_MANAGER`, tipo de saída antes da entrada, `codec_api::apply_settings`,
    /// `NOTIFY_BEGIN_STREAMING` e `NOTIFY_START_OF_STREAM`. Roda na thread do codificador.
    pub(crate) fn open(dev: &SendDevice, cfg: &EncoderConfig, cand: Candidate) -> Result<Self, EncodeError> {
        // SAFETY: o ativador é um objeto COM válido; a thread tem COM/MF inicializados.
        let mft: IMFTransform =
            unsafe { cand.activate.ActivateObject() }.map_err(|e| EncodeError::os("ActivateObject", &e))?;

        // SAFETY: `mft` é válido; os atributos são do próprio MFT.
        let attrs = unsafe { mft.GetAttributes() }.map_err(|e| EncodeError::os("GetAttributes", &e))?;
        // SAFETY: `attrs` é válido.
        unsafe { attrs.SetUINT32(&MF_TRANSFORM_ASYNC_UNLOCK, 1) }
            .map_err(|e| EncodeError::os("MF_TRANSFORM_ASYNC_UNLOCK", &e))?;

        let mut token: u32 = 0;
        let mut mgr: Option<IMFDXGIDeviceManager> = None;
        // SAFETY: `token` e `mgr` são locais válidos para escrita.
        unsafe { MFCreateDXGIDeviceManager(&mut token, &mut mgr) }
            .map_err(|e| EncodeError::os("MFCreateDXGIDeviceManager", &e))?;
        let mgr = mgr.ok_or_else(|| EncodeError::Stopped("MFCreateDXGIDeviceManager sem gerenciador".into()))?;
        // SAFETY: o dispositivo D3D11 é válido e protegido para multithread.
        unsafe { mgr.ResetDevice(&dev.device, token) }.map_err(|e| EncodeError::os("ResetDevice", &e))?;
        // SAFETY: o ponteiro do gerenciador vale enquanto `mgr` viver (guardado na struct).
        unsafe { mft.ProcessMessage(MFT_MESSAGE_SET_D3D_MANAGER, mgr.as_raw() as usize) }
            .map_err(|e| EncodeError::os("SET_D3D_MANAGER", &e))?;

        // Saída antes da entrada: exigência dos MFTs de codificação.
        let saida = media_types::make_output_type(cfg)?;
        // SAFETY: tipo de mídia válido.
        unsafe { mft.SetOutputType(0, &saida, 0) }.map_err(|e| EncodeError::os("SetOutputType", &e))?;
        let entrada = media_types::make_input_type(cfg)?;
        // SAFETY: tipo de mídia válido.
        unsafe { mft.SetInputType(0, &entrada, 0) }.map_err(|e| EncodeError::os("SetInputType", &e))?;

        let codec: ICodecAPI = mft.cast().map_err(|e| EncodeError::os("ICodecAPI", &e))?;
        codec_api::apply_settings(&codec, cfg)?;

        // SAFETY: o MFT já tem os dois tipos definidos.
        let tipo_saida =
            unsafe { mft.GetOutputCurrentType(0) }.map_err(|e| EncodeError::os("GetOutputCurrentType", &e))?;
        let seq_header = media_types::read_sequence_header(&tipo_saida);

        // SAFETY: mensagens sem ponteiros.
        unsafe {
            mft.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)
                .map_err(|e| EncodeError::os("BEGIN_STREAMING", &e))?;
            mft.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)
                .map_err(|e| EncodeError::os("START_OF_STREAM", &e))?;
        }
        let event_gen: IMFMediaEventGenerator =
            mft.cast().map_err(|e| EncodeError::os("IMFMediaEventGenerator", &e))?;

        Ok(Self {
            info: cand.info,
            mft,
            codec,
            event_gen,
            _mgr: mgr,
            seq_header,
        })
    }
    pub(crate) fn info(&self) -> &EncoderInfo {
        &self.info
    }

    /// Refaz os tipos com o bitrate novo depois de um `FLUSH`. `Err` se o driver recusar.
    fn reabrir(&mut self, cfg: &EncoderConfig) -> Result<(), EncodeError> {
        // SAFETY: mensagem sem ponteiros.
        unsafe { self.mft.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0) }.map_err(|e| EncodeError::os("FLUSH", &e))?;
        let saida = media_types::make_output_type(cfg)?;
        // SAFETY: tipo de mídia válido.
        unsafe { self.mft.SetOutputType(0, &saida, 0) }.map_err(|e| EncodeError::os("SetOutputType", &e))?;
        let entrada = media_types::make_input_type(cfg)?;
        // SAFETY: tipo de mídia válido.
        unsafe { self.mft.SetInputType(0, &entrada, 0) }.map_err(|e| EncodeError::os("SetInputType", &e))?;
        if let Err(e) = codec_api::apply_settings(&self.codec, cfg) {
            tracing::warn!("reabertura: apply_settings falhou: {e}");
        }
        // SAFETY: tipo atual válido após SetOutputType.
        if let Ok(t) = unsafe { self.mft.GetOutputCurrentType(0) } {
            self.seq_header = media_types::read_sequence_header(&t);
        }
        // SAFETY: mensagem sem ponteiros.
        unsafe { self.mft.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0) }
            .map_err(|e| EncodeError::os("START_OF_STREAM", &e))?;
        Ok(())
    }

    /// Chama `ProcessOutput` uma vez. `Ok(None)` = sem saída agora.
    fn puxar_saida(&mut self) -> Result<Option<sample::OutputData>, EncodeError> {
        // SAFETY: MFT válido.
        let info =
            unsafe { self.mft.GetOutputStreamInfo(0) }.map_err(|e| EncodeError::os("GetOutputStreamInfo", &e))?;
        let provedor = info.dwFlags & MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 as u32 != 0;
        let amostra: Option<IMFSample> = if provedor {
            None
        } else {
            // O MFT não aloca: damos uma amostra com buffer do tamanho pedido.
            // SAFETY: chamadas de criação sem ponteiros de entrada.
            let s = unsafe { MFCreateSample() }.map_err(|e| EncodeError::os("MFCreateSample", &e))?;
            let b = unsafe { MFCreateMemoryBuffer(info.cbSize.max(1)) }
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
        // Recupera a posse dos ponteiros COM para soltá-los no Drop normal.
        let [mut item] = buf;
        // SAFETY: cada campo é tirado uma única vez e não é usado depois.
        let amostra = unsafe { ManuallyDrop::take(&mut item.pSample) };
        let _eventos = unsafe { ManuallyDrop::take(&mut item.pEvents) };
        match r {
            Ok(()) => match amostra {
                Some(s) => Ok(Some(sample::read_output(&s)?)),
                None => Ok(None),
            },
            Err(e) if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => Ok(None),
            Err(e) if e.code() == MF_E_TRANSFORM_STREAM_CHANGE => {
                // O tipo de saída mudou: renegocia com o primeiro tipo disponível.
                // SAFETY: MFT válido.
                if let Ok(t) = unsafe { self.mft.GetOutputAvailableType(0, 0) } {
                    // SAFETY: tipo válido.
                    if unsafe { self.mft.SetOutputType(0, &t, 0) }.is_ok() {
                        self.seq_header = media_types::read_sequence_header(&t);
                    }
                }
                Ok(None)
            }
            Err(e) => Err(EncodeError::os("ProcessOutput", &e)),
        }
    }

    /// Loop de eventos até `io.shared.stop` ou `io.rx` fechar. Veja o plano: NeedInput pega o
    /// próximo `Job` (espera até 1 intervalo de quadro), HaveOutput vira `EncodedPicture`
    /// (`encode_us` = agora - instante do submit do quadro; `nal::ensure_params` no keyframe),
    /// `pending_kbps` aplicado entre quadros (se recusar: reabre o MFT e o próximo sai keyframe).
    pub(crate) fn run(mut self, io: WorkerIo) {
        use std::sync::atomic::Ordering;
        let mut cfg = io.cfg.clone();
        let espera = Duration::from_micros((logic::frame_duration_hns(cfg.fps) / 10) as u64)
            .min(Duration::from_millis(ESPERA_MAX_MS));
        let mut need_input: u32 = 0;
        let mut forcar_proximo = false;
        // (capture_us, instante do submit), na ordem de envio.
        let mut enviados: VecDeque<(u64, Instant)> = VecDeque::new();

        'principal: while !io.shared.stop.load(Ordering::Acquire) {
            // Eventos primeiro, sem bloquear (assim `stop` é visto rápido).
            // SAFETY: gerador de eventos válido.
            match unsafe { self.event_gen.GetEvent(MF_EVENT_FLAG_NO_WAIT) } {
                Ok(ev) => {
                    // SAFETY: evento válido.
                    let tipo = match unsafe { ev.GetType() } {
                        Ok(t) => t,
                        Err(e) => {
                            tracing::warn!("GetType do evento falhou: {e}");
                            continue;
                        }
                    };
                    if tipo == METransformNeedInput.0 as u32 {
                        need_input += 1;
                    } else if tipo == METransformHaveOutput.0 as u32 {
                        match self.puxar_saida() {
                            Ok(Some(saida)) => {
                                let mut data = saida.data;
                                if saida.clean_point {
                                    data = nal::ensure_params(&data, &self.seq_header).0;
                                }
                                let has_params = nal::has_sps_pps(&data);
                                let capture_us = logic::hns_to_us(saida.time_hns);
                                let pos = enviados.iter().position(|(c, _)| *c == capture_us);
                                let (capture_us, inicio) = match pos {
                                    Some(i) => {
                                        let par = enviados[i];
                                        // Quadros anteriores sem saída (descartados pelo MFT) saem da fila.
                                        enviados.drain(..=i);
                                        par
                                    }
                                    None => (capture_us, Instant::now()),
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
                            Ok(None) => {}
                            Err(e) => {
                                tracing::error!("falha ao ler a saída do MFT: {e}");
                                break 'principal;
                            }
                        }
                    } else if tipo == METransformDrainComplete.0 as u32 {
                        tracing::info!("MFT terminou o drain");
                        break 'principal;
                    } else if tipo == MEError.0 as u32 {
                        // SAFETY: evento válido.
                        let st = unsafe { ev.GetStatus() };
                        tracing::error!(status = ?st, "MEError do MFT de hardware");
                        break 'principal;
                    }
                    continue;
                }
                Err(e) if e.code() == MF_E_NO_EVENTS_AVAILABLE => {}
                Err(e) => {
                    tracing::error!("GetEvent falhou: {e}");
                    break 'principal;
                }
            }

            if need_input == 0 {
                std::thread::sleep(Duration::from_millis(1));
                continue;
            }

            // Bitrate novo entre quadros.
            let kbps = io.shared.pending_kbps.swap(0, Ordering::AcqRel);
            if kbps != 0 {
                cfg.kbps = kbps;
                if let Err(e) = codec_api::set_bitrate(&self.codec, kbps) {
                    tracing::warn!("MFT recusou o bitrate em execução ({e}); reabrindo");
                    match self.reabrir(&cfg) {
                        Ok(()) => {
                            // Depois do FLUSH o MFT pede entrada de novo.
                            need_input = 0;
                            enviados.clear();
                            forcar_proximo = true;
                            continue;
                        }
                        Err(e) => {
                            tracing::error!("reabertura do MFT falhou: {e}");
                            break 'principal;
                        }
                    }
                }
            }

            match io.rx.recv_timeout(espera) {
                Ok(job) => {
                    if job.force_keyframe || forcar_proximo {
                        codec_api::force_keyframe(&self.codec);
                        forcar_proximo = false;
                    }
                    let amostra = match sample::input_from_texture(&job.nv12, job.capture_us, cfg.fps) {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::warn!("quadro descartado, amostra de entrada falhou: {e}");
                            continue;
                        }
                    };
                    // SAFETY: amostra válida; o MFT assíncrono pediu entrada.
                    match unsafe { self.mft.ProcessInput(0, &amostra, 0) } {
                        Ok(()) => {
                            need_input -= 1;
                            enviados.push_back((job.capture_us, Instant::now()));
                            while enviados.len() > 16 {
                                enviados.pop_front();
                            }
                        }
                        Err(e) => {
                            tracing::error!("ProcessInput falhou: {e}");
                            break 'principal;
                        }
                    }
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break 'principal,
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
            if let Err(e) = self.mft.ProcessMessage(MFT_MESSAGE_SET_D3D_MANAGER, 0) {
                tracing::debug!("SET_D3D_MANAGER(0): {e}");
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
    fn open_hardware_ou_pula() {
        let Ok(gpu) = zap_gpu::GpuDevice::create_default() else {
            eprintln!("sem MFT de hardware: pulando");
            return;
        };
        let Ok(_com) = zap_gpu::ComGuard::new() else {
            eprintln!("sem MFT de hardware: pulando");
            return;
        };
        let Ok(_mf) = MfGuard::acquire() else {
            eprintln!("sem MFT de hardware: pulando");
            return;
        };
        let mut lista = match enumerate(true) {
            Ok(l) if !l.is_empty() => l,
            _ => {
                eprintln!("sem MFT de hardware: pulando");
                return;
            }
        };
        let dev = SendDevice {
            device: gpu.d3d_device().clone(),
            context: gpu.context().clone(),
        };
        let cfg = EncoderConfig {
            width: 1280,
            height: 720,
            fps: 30,
            kbps: 4000,
            gop_frames: 300,
            choice: EncoderChoice::Hardware,
        };
        let cand = lista.remove(0);
        match HwEncoder::open(&dev, &cfg, cand) {
            Ok(enc) => assert!(enc.info().hardware),
            Err(e) => eprintln!("MFT de hardware não abriu neste driver ({e}): pulando"),
        }
    }
}
