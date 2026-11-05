//! Texto do "Copiar relatório" (puro).

use std::fmt::Write as _;

use crate::diag::{DiagReport, Severity};
use crate::snapshot::ConverterView;

/// Relatório em texto simples, em português, pronto para colar no chat de suporte.
pub fn relatorio_texto(r: &DiagReport) -> String {
    let mut s = String::new();
    // Escrever numa String não falha; o `let _` só descarta o `Result`.
    let _ = writeln!(s, "StreamaZap {} (protocolo {})", r.app_version, r.protocol);
    s.push('\n');

    s.push_str("== Rede ==\n");
    if r.network.interfaces.is_empty() {
        s.push_str("- nenhuma interface IPv4\n");
    }
    for i in &r.network.interfaces {
        let _ = write!(s, "- {}: {}", i.name, i.ip);
        if i.radmin {
            s.push_str(" (Radmin)");
        }
        s.push('\n');
    }
    let _ = writeln!(s, "Outras VPNs: {}", lista(&r.vpns, "nenhuma"));
    let _ = writeln!(s, "Antivírus com firewall próprio: {}", lista(&r.antivirus, "nenhum"));

    s.push_str("\n== Firewall ==\n");
    if r.firewall.suportado {
        let fw = &r.firewall;
        let [dominio, privado, publico] = fw.bloquear_tudo_por_perfil;
        let _ = writeln!(s, "Bloqueado: {}", sim_nao(fw.bloqueado));
        let _ = writeln!(s, "Liberado: {}", sim_nao(fw.liberado));
        let _ = writeln!(
            s,
            "Bloquear tudo (domínio/privado/público): {}/{}/{}",
            sim_nao(dominio),
            sim_nao(privado),
            sim_nao(publico)
        );
        let _ = writeln!(
            s,
            "Categoria da rede do Radmin: {}",
            fw.categoria_radmin.as_deref().unwrap_or("desconhecida")
        );
        if let Some(erro) = &fw.erro {
            let _ = writeln!(s, "Erro: {erro}");
        }
    } else {
        s.push_str("não suportado neste sistema\n");
    }

    s.push_str("\n== Amigos ==\n");
    if r.friends.is_empty() {
        s.push_str("- nenhum\n");
    }
    for f in &r.friends {
        let ouve = if f.hears_you { "ouve você" } else { "NÃO ouve você" };
        let _ = writeln!(s, "- {} v{} ({}) — {}", f.name, f.version, f.ip, ouve);
    }

    s.push_str("\n== Salas ==\n");
    if r.rooms.is_empty() {
        s.push_str("- nenhuma\n");
    }
    for sala in &r.rooms {
        let _ = write!(
            s,
            "- {} (dono {}, {} pessoa(s))",
            sala.name, sala.host, sala.participants
        );
        if sala.has_password {
            s.push_str(" [senha]");
        }
        s.push('\n');
    }
    match &r.current_room {
        Some(atual) => {
            let _ = writeln!(s, "Sala atual: {} ({} pessoa(s))", atual.name, atual.participants);
        }
        None => s.push_str("Sala atual: nenhuma\n"),
    }

    s.push_str("\n== Vídeo ==\n");
    if r.video.adapters.is_empty() {
        s.push_str("- nenhuma placa de vídeo vista\n");
    }
    for placa in &r.video.adapters {
        let onde = if placa.video_processor {
            "conversão na GPU (Video Processor)"
        } else {
            "conversão pela CPU (sem Video Processor)"
        };
        let _ = writeln!(s, "- {}: {onde}", placa.name);
    }
    match &r.video.sharing {
        Some(t) => {
            let camadas: Vec<String> = t
                .conversion
                .iter()
                .map(|c| {
                    let onde = match c.converter {
                        ConverterView::Gpu => "GPU",
                        ConverterView::Cpu => "CPU",
                    };
                    format!("camada {} {onde}", c.layer)
                })
                .collect();
            let _ = writeln!(
                s,
                "Transmitindo: codificador {}; conversão: {}",
                t.encoder,
                lista(&camadas, "nenhuma camada codificando")
            );
        }
        None => s.push_str("Transmitindo: não\n"),
    }

    s.push_str("\n== Problemas ==\n");
    if r.problems.is_empty() {
        s.push_str("- nenhum\n");
    }
    for p in &r.problems {
        let _ = writeln!(s, "- [{}] {}", severidade(p.severity), p.message);
    }

    s
}

/// Itens separados por ", ", ou o texto de vazio quando não há nenhum.
fn lista(itens: &[String], vazio: &str) -> String {
    if itens.is_empty() {
        vazio.to_string()
    } else {
        itens.join(", ")
    }
}

fn sim_nao(valor: bool) -> &'static str {
    if valor {
        "sim"
    } else {
        "não"
    }
}

fn severidade(s: Severity) -> &'static str {
    match s {
        Severity::Info => "info",
        Severity::Warn => "aviso",
        Severity::Error => "erro",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::{DiagFriend, DiagInterface, DiagNetwork, DiagProblem, DiagRoom};
    use zap_sys::FirewallStatus;

    fn firewall_ativo() -> FirewallStatus {
        FirewallStatus {
            suportado: true,
            bloqueado: true,
            liberado: false,
            bloquear_tudo_por_perfil: [false, true, false],
            categoria_radmin: None,
            regras: Vec::new(),
            erro: Some("falhou".to_string()),
        }
    }

    fn relatorio(firewall: FirewallStatus) -> DiagReport {
        DiagReport {
            app_version: "3.0.0".to_string(),
            protocol: 7,
            network: DiagNetwork {
                interfaces: vec![DiagInterface {
                    name: "Radmin VPN".to_string(),
                    ip: "26.1.2.3".to_string(),
                    broadcast: None,
                    radmin: true,
                }],
                radmin_ip: Some("26.1.2.3".to_string()),
            },
            vpns: vec!["Hamachi".to_string()],
            antivirus: Vec::new(),
            firewall,
            friends: vec![DiagFriend {
                name: "Ana".to_string(),
                version: "3.0.0".to_string(),
                ip: "26.1.2.4".to_string(),
                hears_you: false,
                same_version: true,
            }],
            rooms: vec![DiagRoom {
                id: "r1".to_string(),
                name: "Sala".to_string(),
                host: "Bia".to_string(),
                participants: 2,
                has_password: true,
                addr: None,
                version_warn: None,
            }],
            current_room: None,
            other_versions: Vec::new(),
            newer_protocol: None,
            video: crate::diag::DiagVideo::default(),
            problems: vec![
                DiagProblem {
                    severity: Severity::Warn,
                    code: "a".to_string(),
                    message: "Cuidado".to_string(),
                },
                DiagProblem {
                    severity: Severity::Error,
                    code: "b".to_string(),
                    message: "Quebrou".to_string(),
                },
                DiagProblem {
                    severity: Severity::Info,
                    code: "c".to_string(),
                    message: "Só saber".to_string(),
                },
            ],
        }
    }

    #[test]
    fn texto_contem_secoes_na_ordem() {
        let t = relatorio_texto(&relatorio(firewall_ativo()));
        let pos = |marca: &str| t.find(marca).unwrap_or(usize::MAX);
        assert!(t.starts_with("StreamaZap 3.0.0 (protocolo 7)\n"));
        assert!(pos("== Rede ==") < pos("== Firewall =="));
        assert!(pos("== Firewall ==") < pos("== Amigos =="));
        assert!(pos("== Amigos ==") < pos("== Salas =="));
        assert!(pos("== Salas ==") < pos("== Vídeo =="));
        assert!(pos("== Vídeo ==") < pos("== Problemas =="));
    }

    #[test]
    fn texto_video_placas_e_transmissao() {
        use crate::diag::{DiagVideoAdapter, DiagVideoShare};
        use crate::snapshot::LayerConversion;
        let mut r = relatorio(firewall_ativo());
        let t = relatorio_texto(&r);
        assert!(t.contains("== Vídeo ==\n- nenhuma placa de vídeo vista\nTransmitindo: não\n"));

        r.video.adapters = vec![
            DiagVideoAdapter {
                name: "GPU A".into(),
                video_processor: true,
            },
            DiagVideoAdapter {
                name: "GPU B".into(),
                video_processor: false,
            },
        ];
        r.video.sharing = Some(DiagVideoShare {
            encoder: "NVENC".into(),
            conversion: vec![
                LayerConversion {
                    layer: 0,
                    converter: ConverterView::Gpu,
                },
                LayerConversion {
                    layer: 1,
                    converter: ConverterView::Cpu,
                },
            ],
        });
        let t = relatorio_texto(&r);
        assert!(t.contains("- GPU A: conversão na GPU (Video Processor)\n"));
        assert!(t.contains("- GPU B: conversão pela CPU (sem Video Processor)\n"));
        assert!(t.contains("Transmitindo: codificador NVENC; conversão: camada 0 GPU, camada 1 CPU\n"));

        r.video.sharing = Some(DiagVideoShare {
            encoder: "NVENC".into(),
            conversion: vec![],
        });
        assert!(relatorio_texto(&r).contains("conversão: nenhuma camada codificando\n"));
    }

    #[test]
    fn texto_marca_radmin_e_amigo_que_nao_ouve() {
        let t = relatorio_texto(&relatorio(firewall_ativo()));
        assert!(t.contains("- Radmin VPN: 26.1.2.3 (Radmin)\n"));
        assert!(t.contains("- Ana v3.0.0 (26.1.2.4) — NÃO ouve você\n"));
        assert!(t.contains("- Sala (dono Bia, 2 pessoa(s)) [senha]\n"));
        assert!(t.contains("Outras VPNs: Hamachi\n"));
        assert!(t.contains("Antivírus com firewall próprio: nenhum\n"));
        assert!(t.contains("Sala atual: nenhuma\n"));
    }

    #[test]
    fn texto_firewall_ativo_mostra_campos() {
        let t = relatorio_texto(&relatorio(firewall_ativo()));
        assert!(t.contains("Bloqueado: sim\n"));
        assert!(t.contains("Liberado: não\n"));
        assert!(t.contains("Bloquear tudo (domínio/privado/público): não/sim/não\n"));
        assert!(t.contains("Categoria da rede do Radmin: desconhecida\n"));
        assert!(t.contains("Erro: falhou\n"));
    }

    #[test]
    fn texto_listas_vazias() {
        let mut r = relatorio(firewall_ativo());
        r.network.interfaces.clear();
        r.vpns.clear();
        r.friends.clear();
        r.rooms.clear();
        r.problems.clear();
        let t = relatorio_texto(&r);
        assert!(t.contains("- nenhuma interface IPv4\n"));
        assert!(t.contains("Outras VPNs: nenhuma\n"));
        assert!(t.contains("== Amigos ==\n- nenhum\n"));
        assert!(t.contains("== Salas ==\n- nenhuma\n"));
        assert!(t.contains("== Problemas ==\n- nenhum\n"));
    }

    #[test]
    fn texto_firewall_nao_suportado() {
        let t = relatorio_texto(&relatorio(FirewallStatus::nao_suportado()));
        assert!(!t.contains("Bloqueado:"));
        assert!(t.contains("== Firewall ==\nnão suportado neste sistema\n"));
    }

    #[test]
    fn texto_problemas_com_severidade() {
        let t = relatorio_texto(&relatorio(firewall_ativo()));
        assert!(t.contains("- [aviso] Cuidado\n"));
        assert!(t.contains("- [erro] Quebrou\n"));
        assert!(t.contains("- [info] Só saber\n"));
    }

    #[test]
    fn termina_com_quebra_de_linha() {
        let t = relatorio_texto(&relatorio(firewall_ativo()));
        assert!(t.ends_with("- [info] Só saber\n"));
        assert!(!t.ends_with("\n\n"));
    }
}
