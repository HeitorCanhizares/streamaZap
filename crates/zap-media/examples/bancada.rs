//! Bancada de medição (Windows): `cargo run -p zap-media --example bancada -- --monitor 1
//! --preset 1080p60 --segundos 10`. Ver tarefa media-08.
//!
//! Liga todas as camadas da escada de um monitor, conta quadros e bytes por camada e imprime os
//! tempos de cada etapa do pipeline. Fora do Windows só imprime um aviso.
//! Os argumentos são lidos à mão (sem crate nova), em `parse_args`, que é função pura e testável.
#![cfg_attr(not(windows), allow(dead_code))]

use zap_rate::ladder::Preset;

const USO: &str = "uso: bancada [--monitor N] [--preset 720p30|720p60|1080p30|1080p60] [--segundos S] [--software] [--animar|--sem-animar] [--aquecimento S]";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Args {
    /// Índice do monitor, a partir de 1 (principal = 1).
    monitor: u32,
    preset: Preset,
    segundos: u64,
    /// Força o codificador por software (sem tentar hardware).
    software: bool,
    /// Abre a janela animada no monitor medido (padrão: sim).
    animar: bool,
    /// Segundos de aquecimento antes de contar quadros (padrão: 2).
    aquecimento: u64,
}

/// Lê os argumentos (sem o nome do programa). Erro = mensagem a mostrar junto do uso.
fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut out = Args {
        monitor: 1,
        preset: Preset::P1080p60,
        segundos: 10,
        software: false,
        animar: true,
        aquecimento: 2,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--monitor" => {
                let s = proximo(&mut it, "--monitor")?;
                let n: u32 = s
                    .parse()
                    .map_err(|_| format!("--monitor precisa de inteiro, veio {s:?}"))?;
                // Índice de monitor começa em 1: zero nunca existe.
                if n == 0 {
                    return Err("--monitor começa em 1".into());
                }
                out.monitor = n;
            }
            "--preset" => {
                out.preset = parse_preset(proximo(&mut it, "--preset")?)?;
            }
            "--segundos" => {
                let s = proximo(&mut it, "--segundos")?;
                let n: u64 = s
                    .parse()
                    .map_err(|_| format!("--segundos precisa de inteiro, veio {s:?}"))?;
                if n == 0 {
                    return Err("--segundos precisa ser maior que 0".into());
                }
                out.segundos = n;
            }
            "--software" => out.software = true,
            "--animar" => out.animar = true,
            "--sem-animar" => out.animar = false,
            "--aquecimento" => {
                let s = proximo(&mut it, "--aquecimento")?;
                // Zero é válido: mede desde o primeiro quadro.
                out.aquecimento = s
                    .parse()
                    .map_err(|_| format!("--aquecimento precisa de inteiro, veio {s:?}"))?;
            }
            outro => return Err(format!("argumento desconhecido: {outro:?}")),
        }
    }
    Ok(out)
}

/// Pega o valor que segue uma flag; sem valor, erro.
fn proximo<'a>(it: &mut std::slice::Iter<'a, String>, flag: &str) -> Result<&'a str, String> {
    it.next()
        .map(|s| s.as_str())
        .ok_or_else(|| format!("{flag} precisa de um valor"))
}

/// Aviso de tela parada: sem quadros novos da captura os tempos medem espera, não trabalho.
fn aviso_tela_parada(novos_por_s: f32, animando: bool) -> Option<String> {
    let limite = if animando { 30.0 } else { 5.0 };
    if novos_por_s < limite {
        Some(format!(
            "AVISO: tela quase parada ({novos_por_s:.1} quadros novos/s): os tempos medem espera, não trabalho"
        ))
    } else {
        None
    }
}

fn parse_preset(s: &str) -> Result<Preset, String> {
    match s {
        "720p30" => Ok(Preset::P720p30),
        "720p60" => Ok(Preset::P720p60),
        "1080p30" => Ok(Preset::P1080p30),
        "1080p60" => Ok(Preset::P1080p60),
        _ => Err(format!("--preset inválido: {s:?}")),
    }
}

#[cfg(not(windows))]
fn main() {
    println!("bancada só roda no Windows");
}

#[cfg(windows)]
fn main() {
    let argumentos: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse_args(&argumentos) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}\n{USO}");
            std::process::exit(2);
        }
    };
    if let Err(e) = medir(args) {
        eprintln!("erro: {e}");
        std::process::exit(1);
    }
}

#[cfg(windows)]
fn medir(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    use std::collections::BTreeMap;
    use std::time::{Duration, Instant};
    use zap_media::{EncodedFrame, EncoderChoiceSpec, MonitorRef, VideoPipeline, VideoPipelineConfig, VideoSourceSpec};
    use zap_rate::allocator::LayerRequest;
    use zap_rate::ladder::screen_ladder;

    // Resolução de 1 ms enquanto a bancada roda; sem ela as esperas ficam em ~15,6 ms e distorcem a medição.
    let timer = zap_sys::platform::TimerResolution::high();
    let timer_ativo = timer.is_active();

    let monitores = zap_capture::list_monitors()?;
    let m = monitores
        .iter()
        .find(|m| m.index == args.monitor)
        .ok_or_else(|| format!("monitor {} não existe (encontrados: {})", args.monitor, monitores.len()))?;
    let (w, h) = (m.width, m.height);
    let cabecalho = format!("monitor {} ({}), {}x{}", args.monitor, m.name, w, h);

    // Declarada antes do pipeline: é descartada depois dele, inclusive em erro por `?`.
    let _janela = if args.animar {
        match zap_capture::movimento::JanelaAnimada::abrir(m) {
            Ok(j) => {
                std::thread::sleep(Duration::from_millis(500));
                Some(j)
            }
            Err(e) => {
                eprintln!("aviso: não consegui abrir a janela animada ({e}); seguindo sem animação");
                None
            }
        }
    } else {
        None
    };
    let animando = _janela.is_some();

    let escada = screen_ladder(args.preset, w, h);
    let encoder = if args.software {
        EncoderChoiceSpec::Software
    } else {
        EncoderChoiceSpec::Auto
    };
    let timer_txt = if timer_ativo { "sim" } else { "não" };
    println!(
        "{cabecalho}, preset {:?}, {} s (aquecimento {} s), codificador pedido {:?}, timer 1 ms: {timer_txt}, animação: {}",
        args.preset,
        args.segundos,
        args.aquecimento,
        encoder,
        if animando { "sim" } else { "não" }
    );

    let (tx, rx) = crossbeam_channel::unbounded::<EncodedFrame>();
    // `pipeline` é declarado depois de `rx`: é descartado primeiro, então a thread encerra antes.
    let pipeline = VideoPipeline::start(
        VideoPipelineConfig {
            track: 1,
            source: VideoSourceSpec::Monitor(MonitorRef(args.monitor)),
            ladder: escada.clone(),
            encoder,
        },
        tx,
    )?;
    let pedidos: Vec<LayerRequest> = escada
        .iter()
        .map(|l| LayerRequest {
            layer: l.layer,
            kbps: l.kbps,
        })
        .collect();
    pipeline.set_requests(&pedidos);

    // Por camada: (quadros, bytes) recebidos durante a janela de medição.
    let mut contagem: BTreeMap<u8, (u64, u64)> = BTreeMap::new();
    // Aquecimento: descarta os quadros para o codificador e o conversor estabilizarem.
    let fim_aquecimento = Instant::now() + Duration::from_secs(args.aquecimento);
    loop {
        let restante = fim_aquecimento.saturating_duration_since(Instant::now());
        if restante.is_zero() || rx.recv_timeout(restante).is_err() {
            break;
        }
    }
    contagem.clear();
    let inicio = Instant::now();
    let fim = inicio + Duration::from_secs(args.segundos);
    loop {
        let restante = fim.saturating_duration_since(Instant::now());
        if restante.is_zero() {
            break;
        }
        match rx.recv_timeout(restante) {
            Ok(quadro) => {
                let c = contagem.entry(quadro.layer).or_insert((0, 0));
                c.0 += 1;
                c.1 += quadro.data.len() as u64;
            }
            // Tempo esgotado ou canal fechado: fim da medição.
            Err(_) => break,
        }
    }
    let decorrido = inicio.elapsed().as_secs_f64().max(f64::EPSILON);

    let st = pipeline.stats();
    let da_camada = |camada: u8| {
        st.layers
            .iter()
            .find(|s| s.layer == camada)
            .cloned()
            .unwrap_or_default()
    };
    // Tempos em ms: as estatísticas vêm em microssegundos.
    let ms = |us: f64| us / 1000.0;

    println!();
    println!(
        "{:>6} {:>11} {:>8} {:>10} {:>22} {:>22} {:>22} {:>16}",
        "camada",
        "WxH",
        "fps",
        "kbps",
        "conversão média/p95",
        "conv. GPU média/p95",
        "codificação média/p95",
        "espera lock p95"
    );
    for l in &escada {
        let (quadros, bytes) = contagem.get(&l.layer).copied().unwrap_or((0, 0));
        let fps = quadros as f64 / decorrido;
        let kbps = bytes as f64 * 8.0 / 1000.0 / decorrido;
        let s = da_camada(l.layer);
        println!(
            "{:>6} {:>11} {:>8.1} {:>10.0} {:>22} {:>22} {:>22} {:>16}",
            l.layer,
            format!("{}x{}", l.width, l.height),
            fps,
            kbps,
            format!("{:.2} / {:.2}", ms(s.convert.avg_us), ms(s.convert.p95_us)),
            format!("{:.2} / {:.2}", ms(s.convert_gpu.avg_us), ms(s.convert_gpu.p95_us)),
            format!("{:.2} / {:.2}", ms(s.encode.avg_us), ms(s.encode.p95_us)),
            format!("{:.3}", ms(s.lock_wait.p95_us)),
        );
    }

    let cap = st.capture;
    println!();
    println!("quadros novos da captura: {:.1}/s", st.capture_new_fps);
    if let Some(aviso) = aviso_tela_parada(st.capture_new_fps, animando) {
        println!("{aviso}");
    }
    println!(
        "captura média/p95: {:.2} / {:.2} ms; descartados: captura {}, codificação {}; codificador: {}",
        ms(cap.avg_us),
        ms(cap.p95_us),
        st.dropped_capture,
        st.dropped_encode,
        st.encoder_name
    );
    let camada0 = da_camada(0);
    let soma = ms(cap.p95_us + camada0.convert.p95_us + camada0.encode.p95_us);
    println!("p95 captura+conversão+codificação camada 0: {soma:.1} ms (meta <= 8)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn defaults_sem_argumentos() {
        let a = parse_args(&v(&[])).expect("defaults");
        assert_eq!(
            a,
            Args {
                monitor: 1,
                preset: Preset::P1080p60,
                segundos: 10,
                software: false,
                animar: true,
                aquecimento: 2,
            }
        );
    }

    #[test]
    fn animar_flags() {
        assert!(!parse_args(&v(&["--sem-animar"])).expect("flag").animar);
        assert!(parse_args(&v(&["--sem-animar", "--animar"])).expect("flag").animar);
    }

    #[test]
    fn aquecimento_lido() {
        assert_eq!(parse_args(&v(&["--aquecimento", "5"])).map(|a| a.aquecimento), Ok(5));
        assert_eq!(parse_args(&v(&["--aquecimento", "0"])).map(|a| a.aquecimento), Ok(0));
        assert!(parse_args(&v(&["--aquecimento", "x"])).is_err());
        assert!(parse_args(&v(&["--aquecimento"])).is_err());
    }

    #[test]
    fn aviso_tela_parada_limites() {
        assert!(aviso_tela_parada(29.9, true).is_some());
        assert!(aviso_tela_parada(30.0, true).is_none());
        assert!(aviso_tela_parada(4.9, false).is_some());
        assert!(aviso_tela_parada(5.0, false).is_none());
        assert_eq!(
            aviso_tela_parada(2.0, false).as_deref(),
            Some("AVISO: tela quase parada (2.0 quadros novos/s): os tempos medem espera, não trabalho")
        );
    }

    #[test]
    fn flags_lidas() {
        let a = parse_args(&v(&[
            "--monitor",
            "2",
            "--preset",
            "720p30",
            "--segundos",
            "5",
            "--software",
        ]))
        .expect("flags");
        assert_eq!(a.monitor, 2);
        assert_eq!(a.preset, Preset::P720p30);
        assert_eq!(a.segundos, 5);
        assert!(a.software);
    }

    #[test]
    fn todos_os_presets_validos() {
        assert_eq!(
            parse_args(&v(&["--preset", "720p60"])).map(|a| a.preset),
            Ok(Preset::P720p60)
        );
        assert_eq!(
            parse_args(&v(&["--preset", "1080p30"])).map(|a| a.preset),
            Ok(Preset::P1080p30)
        );
    }

    #[test]
    fn preset_invalido_e_erro() {
        assert!(parse_args(&v(&["--preset", "4k60"])).is_err());
    }

    #[test]
    fn segundos_nao_numerico_e_erro() {
        assert!(parse_args(&v(&["--segundos", "dez"])).is_err());
        assert!(parse_args(&v(&["--segundos", "0"])).is_err());
    }

    #[test]
    fn monitor_zero_ou_sem_valor_e_erro() {
        assert!(parse_args(&v(&["--monitor", "0"])).is_err());
        assert!(parse_args(&v(&["--monitor"])).is_err());
    }

    #[test]
    fn argumento_desconhecido_e_erro() {
        assert!(parse_args(&v(&["--xpto"])).is_err());
    }
}
