//! Montagem pura do relatório de diagnóstico (sem I/O). Tipos em `diag`.

use std::net::IpAddr;

use zap_sys::Interface;

use crate::diag::{
    DiagCurrentRoom, DiagFriend, DiagInput, DiagInterface, DiagNetwork, DiagOtherVersion, DiagProblem, DiagReport,
    DiagRoom, DiagVideo, DiagVideoAdapter, DiagVideoShare, Severity,
};
use crate::snapshot::{ConverterView, PeerListing, RoomListing, RoomView, ShareView};

pub fn rede_do(interfaces: &[Interface]) -> DiagNetwork {
    DiagNetwork {
        interfaces: interfaces
            .iter()
            .map(|i| DiagInterface {
                name: i.nome.clone(),
                ip: i.ip.to_string(),
                broadcast: i.broadcast.map(|b| b.to_string()),
                radmin: i.radmin,
            })
            .collect(),
        // Primeira interface Radmin, na ordem recebida.
        radmin_ip: interfaces.iter().find(|i| i.radmin).map(|i| i.ip.to_string()),
    }
}

pub fn amigos_do(peers: &[PeerListing], app_version: &str) -> Vec<DiagFriend> {
    peers
        .iter()
        .map(|p| DiagFriend {
            name: p.name.clone(),
            version: p.version.clone(),
            ip: p.ip.clone(),
            hears_you: p.receives_you,
            same_version: p.version == app_version,
        })
        .collect()
}

pub fn salas_do(rooms: &[RoomListing]) -> Vec<DiagRoom> {
    rooms
        .iter()
        .map(|r| DiagRoom {
            id: r.room_id.clone(),
            name: r.name.clone(),
            host: r.host.clone(),
            participants: r.participants,
            has_password: r.has_password,
            addr: r.addr.clone(),
            version_warn: r.version_warn.clone(),
        })
        .collect()
}

pub fn sala_atual_do(room: Option<&RoomView>) -> Option<DiagCurrentRoom> {
    room.map(|r| DiagCurrentRoom {
        name: r.name.clone(),
        mode: r.mode,
        participants: r.participants.len() as u32,
        code: r.code.clone(),
    })
}

/// Uma linha por par (ip, protocolo) distinto, do protocolo mais novo para o mais antigo.
pub fn outras_versoes_do(vistas: &[(IpAddr, u32)], meu_protocolo: u32) -> Vec<DiagOtherVersion> {
    let mut pares: Vec<(String, u32)> = vistas.iter().map(|(ip, v)| (ip.to_string(), *v)).collect();
    // Ordena antes do dedup: pares iguais ficam vizinhos.
    pares.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    pares.dedup();
    pares
        .into_iter()
        .map(|(ip, v)| DiagOtherVersion {
            ip,
            protocol: v,
            newer: v > meu_protocolo,
        })
        .collect()
}

/// Placas de vídeo da sonda e, se houver transmissão, o codificador e o conversor por camada.
pub fn video_do(adapters: &[zap_media::VideoAdapter], share: Option<&ShareView>) -> DiagVideo {
    DiagVideo {
        adapters: adapters
            .iter()
            .map(|a| DiagVideoAdapter {
                name: a.name.clone(),
                video_processor: a.video_processor,
            })
            .collect(),
        sharing: share.map(|s| DiagVideoShare {
            encoder: s.encoder.clone(),
            conversion: s.conversion.clone(),
        }),
    }
}

/// Maior protocolo visto que seja maior que `meu_protocolo`.
pub fn protocolo_maior(vistas: &[(IpAddr, u32)], meu_protocolo: u32) -> Option<u32> {
    vistas.iter().map(|(_, v)| *v).filter(|v| *v > meu_protocolo).max()
}

/// Achados do relatório, na ordem fixa dos códigos (a interface escolhe o texto/ação).
pub fn encontrar_problemas(r: &DiagReport) -> Vec<DiagProblem> {
    let mut out = Vec::new();
    let fw = &r.firewall;

    if r.network.interfaces.is_empty() {
        out.push(problema(
            Severity::Error,
            "network_none",
            "Nenhuma rede IPv4 ativa encontrada.".into(),
        ));
    } else if r.network.radmin_ip.is_none() {
        out.push(problema(
            Severity::Warn,
            "no_radmin",
            "Radmin VPN não encontrado: só amigos na mesma rede local aparecem.".into(),
        ));
    }

    if fw.suportado {
        if let Some(erro) = &fw.erro {
            out.push(problema(
                Severity::Warn,
                "firewall_error",
                format!("Não consegui ler o firewall: {erro}"),
            ));
        }
        if fw.bloqueado {
            out.push(problema(
                Severity::Error,
                "firewall_blocked",
                "O firewall do Windows tem uma regra bloqueando o StreamaZap.".into(),
            ));
        }
        if !fw.bloqueado && !fw.liberado && fw.erro.is_none() {
            out.push(problema(
                Severity::Warn,
                "firewall_not_allowed",
                "O firewall não tem regra liberando o StreamaZap.".into(),
            ));
        }
        // Só o perfil do Radmin importa: "bloquear tudo" em outro perfil não afeta a VPN.
        if let Some(idx) = fw
            .categoria_radmin
            .as_deref()
            .and_then(zap_sys::firewall::perfil_da_categoria)
        {
            if fw.bloquear_tudo_por_perfil.get(idx) == Some(&true) {
                out.push(problema(
                    Severity::Error,
                    "firewall_block_all",
                    "O perfil de rede do Radmin bloqueia todas as conexões de entrada.".into(),
                ));
            }
        }
    }

    if !r.antivirus.is_empty() {
        out.push(problema(
            Severity::Warn,
            "antivirus",
            format!("Antivírus com firewall próprio: {}.", r.antivirus.join(", ")),
        ));
    }

    if !r.vpns.is_empty() {
        out.push(problema(
            Severity::Info,
            "other_vpn",
            format!("Outras VPNs ativas: {}.", r.vpns.join(", ")),
        ));
    }

    // Placa sem Video Processor: transmite, mas converte a cor pela CPU.
    let mut sem_vp = false;
    for placa in r.video.adapters.iter().filter(|a| !a.video_processor) {
        sem_vp = true;
        let mut msg = format!(
            "{}: sem Video Processor, a conversão de cor vai pela CPU (funciona, mas usa mais processador).",
            placa.name
        );
        if placa.name.contains("Basic Render") || placa.name.contains("Basic Display") {
            msg.push_str(" Instale o driver da placa de vídeo.");
        }
        out.push(problema(Severity::Warn, "video_cpu_conversion", msg));
    }
    // Placa com Video Processor, mas a transmissão caiu para a CPU (o VP falhou no meio).
    if let Some(s) = &r.video.sharing {
        let camadas: Vec<String> = s
            .conversion
            .iter()
            .filter(|c| c.converter == ConverterView::Cpu)
            .map(|c| c.layer.to_string())
            .collect();
        if !sem_vp && !camadas.is_empty() {
            out.push(problema(
                Severity::Warn,
                "share_cpu_conversion",
                format!(
                    "A transmissão atual converte a cor pela CPU (camada {}): usa mais processador.",
                    camadas.join(", ")
                ),
            ));
        }
    }

    if r.friends.is_empty() {
        out.push(problema(
            Severity::Info,
            "no_friends",
            "Nenhum amigo visto na rede ainda.".into(),
        ));
    }

    for amigo in r.friends.iter().filter(|a| !a.hears_you) {
        out.push(problema(
            Severity::Warn,
            "friend_not_hearing",
            format!("{} não está recebendo você (firewall ou antivírus dele?).", amigo.name),
        ));
    }

    for amigo in r.friends.iter().filter(|a| !a.same_version) {
        out.push(problema(
            Severity::Info,
            "version_mismatch",
            format!("{} usa a versão {}.", amigo.name, amigo.version),
        ));
    }

    if let Some(p) = r.newer_protocol {
        out.push(problema(
            Severity::Warn,
            "newer_protocol",
            format!(
                "Alguém na rede usa o protocolo {p}, mais novo que o seu ({}). Atualize o StreamaZap.",
                r.protocol
            ),
        ));
    }

    out
}

fn problema(severity: Severity, code: &str, message: String) -> DiagProblem {
    DiagProblem {
        severity,
        code: code.into(),
        message,
    }
}

/// Junta os dados brutos num relatório. Sem I/O: `problems` sai de `encontrar_problemas`
/// depois que o resto do relatório está montado, porque os achados leem os campos prontos.
pub fn montar_relatorio(input: &DiagInput) -> DiagReport {
    // Ordena e tira repetidas: a ordem da coleta não pode mudar o relatório.
    let mut vpns = input.vpns.clone();
    vpns.sort();
    vpns.dedup();
    let mut antivirus = input.antivirus.clone();
    antivirus.sort();
    antivirus.dedup();

    let mut r = DiagReport {
        app_version: input.app_version.clone(),
        protocol: input.protocol,
        network: rede_do(&input.interfaces),
        vpns,
        antivirus,
        firewall: input.firewall.clone(),
        friends: amigos_do(&input.peers, &input.app_version),
        rooms: salas_do(&input.rooms),
        current_room: sala_atual_do(input.room.as_ref()),
        other_versions: outras_versoes_do(&input.other_versions, input.protocol),
        newer_protocol: protocolo_maior(&input.other_versions, input.protocol),
        video: video_do(&input.video_adapters, input.share.as_ref()),
        problems: Vec::new(),
    };
    r.problems = encontrar_problemas(&r);
    r
}

#[cfg(test)]
mod tests_basicos {
    use std::net::Ipv4Addr;

    use super::*;
    use crate::snapshot::{Participant, RoomView};
    use crate::types::RoomMode;

    fn iface(nome: &str, ip: Ipv4Addr, broadcast: Option<Ipv4Addr>, radmin: bool) -> Interface {
        Interface {
            nome: nome.into(),
            ip,
            broadcast,
            radmin,
        }
    }

    #[test]
    fn rede_do_ordem_e_radmin_ip() {
        let interfaces = [
            iface("Ethernet", Ipv4Addr::new(192, 168, 0, 10), None, false),
            iface(
                "Radmin VPN",
                Ipv4Addr::new(26, 1, 2, 3),
                Some(Ipv4Addr::new(26, 255, 255, 255)),
                true,
            ),
        ];
        let rede = rede_do(&interfaces);
        assert_eq!(rede.interfaces.len(), 2);
        assert_eq!(rede.interfaces[0].name, "Ethernet");
        assert_eq!(rede.interfaces[0].ip, "192.168.0.10");
        assert_eq!(rede.interfaces[0].broadcast, None);
        assert_eq!(rede.interfaces[1].name, "Radmin VPN");
        assert_eq!(rede.interfaces[1].broadcast.as_deref(), Some("26.255.255.255"));
        assert!(rede.interfaces[1].radmin);
        assert_eq!(rede.radmin_ip.as_deref(), Some("26.1.2.3"));
    }

    #[test]
    fn rede_do_vazio() {
        let rede = rede_do(&[]);
        assert!(rede.interfaces.is_empty());
        assert_eq!(rede.radmin_ip, None);
    }

    #[test]
    fn amigos_do_marca_versao_e_ouve() {
        let peers = [
            PeerListing {
                name: "Ana".into(),
                version: "3.0.0".into(),
                ip: "10.0.0.2".into(),
                receives_you: true,
            },
            PeerListing {
                name: "Beto".into(),
                version: "2.9.1".into(),
                ip: "10.0.0.3".into(),
                receives_you: false,
            },
        ];
        let amigos = amigos_do(&peers, "3.0.0");
        assert_eq!(amigos.len(), 2);
        assert!(amigos[0].same_version);
        assert!(amigos[0].hears_you);
        assert_eq!(amigos[1].name, "Beto");
        assert!(!amigos[1].same_version);
        assert!(!amigos[1].hears_you);
    }

    #[test]
    fn salas_do_copia_campos() {
        let mut sala = RoomListing::basic("zap-x".into(), "Sala".into(), "Host".into(), 4, true, RoomMode::Online);
        sala.addr = Some("10.0.0.9:7000".into());
        sala.version_warn = Some("versão antiga".into());
        let salas = salas_do(&[sala]);
        assert_eq!(salas.len(), 1);
        assert_eq!(salas[0].id, "zap-x");
        assert_eq!(salas[0].name, "Sala");
        assert_eq!(salas[0].host, "Host");
        assert_eq!(salas[0].participants, 4);
        assert!(salas[0].has_password);
        assert_eq!(salas[0].addr.as_deref(), Some("10.0.0.9:7000"));
        assert_eq!(salas[0].version_warn.as_deref(), Some("versão antiga"));
    }

    #[test]
    fn sala_atual_do_none_e_some() {
        assert_eq!(sala_atual_do(None), None);
        let pessoa = |id: &str| Participant {
            id: id.into(),
            name: id.into(),
            color: "#fff".into(),
            ping_ms: 10,
            sharing: false,
            camera: false,
            volume: None,
            muted: None,
        };
        let view = RoomView {
            id: "zap-x".into(),
            name: "Sala".into(),
            mode: RoomMode::Online,
            code: Some("ZAP-7K2Q".into()),
            participants: vec![pessoa("a"), pessoa("b")],
            tracks: vec![],
            chat: vec![],
            unread: 0,
            me_id: String::new(),
        };
        let atual = sala_atual_do(Some(&view)).expect("sala presente");
        assert_eq!(atual.name, "Sala");
        assert_eq!(atual.mode, RoomMode::Online);
        assert_eq!(atual.participants, 2);
        assert_eq!(atual.code.as_deref(), Some("ZAP-7K2Q"));
    }
}

#[cfg(test)]
mod tests_problemas {
    use zap_sys::FirewallStatus;

    use super::*;

    fn relatorio_ok() -> DiagReport {
        DiagReport {
            app_version: "3.0.0".into(),
            protocol: 4,
            network: DiagNetwork {
                interfaces: vec![DiagInterface {
                    name: "Radmin VPN".into(),
                    ip: "26.1.2.3".into(),
                    broadcast: None,
                    radmin: true,
                }],
                radmin_ip: Some("26.1.2.3".into()),
            },
            vpns: vec![],
            antivirus: vec![],
            firewall: FirewallStatus {
                suportado: true,
                bloqueado: false,
                liberado: true,
                bloquear_tudo_por_perfil: [false; 3],
                categoria_radmin: Some("Private".into()),
                regras: vec![],
                erro: None,
            },
            friends: vec![DiagFriend {
                name: "Ana".into(),
                version: "3.0.0".into(),
                ip: "26.1.2.4".into(),
                hears_you: true,
                same_version: true,
            }],
            rooms: vec![],
            current_room: None,
            other_versions: vec![],
            newer_protocol: None,
            video: DiagVideo::default(),
            problems: vec![],
        }
    }

    fn codigos(problemas: &[DiagProblem]) -> Vec<&str> {
        problemas.iter().map(|p| p.code.as_str()).collect()
    }

    #[test]
    fn relatorio_saudavel_sem_problemas_graves() {
        assert!(encontrar_problemas(&relatorio_ok()).is_empty());
    }

    fn placa(nome: &str, vp: bool) -> DiagVideoAdapter {
        DiagVideoAdapter {
            name: nome.into(),
            video_processor: vp,
        }
    }

    fn transmitindo(conv: &[(u8, ConverterView)]) -> Option<DiagVideoShare> {
        Some(DiagVideoShare {
            encoder: "NVIDIA H.264 Encoder MFT".into(),
            conversion: conv
                .iter()
                .map(|&(layer, converter)| crate::snapshot::LayerConversion { layer, converter })
                .collect(),
        })
    }

    #[test]
    fn placa_com_video_processor_nao_gera_aviso() {
        let mut r = relatorio_ok();
        r.video.adapters = vec![placa("NVIDIA GeForce GTX 1060 6GB", true)];
        r.video.sharing = transmitindo(&[(0, ConverterView::Gpu), (1, ConverterView::Gpu)]);
        assert!(encontrar_problemas(&r).is_empty());
    }

    #[test]
    fn placa_sem_video_processor_avisa_conversao_pela_cpu() {
        let mut r = relatorio_ok();
        r.video.adapters = vec![placa("Placa X", false)];
        // A transmissão na CPU é consequência da placa: um aviso só.
        r.video.sharing = transmitindo(&[(0, ConverterView::Cpu)]);
        let problemas = encontrar_problemas(&r);
        assert_eq!(codigos(&problemas), vec!["video_cpu_conversion"]);
        assert_eq!(problemas[0].severity, Severity::Warn);
        assert!(problemas[0].message.starts_with("Placa X: sem Video Processor"));
        assert!(!problemas[0].message.contains("driver"));
    }

    #[test]
    fn driver_basico_sugere_instalar_o_driver() {
        let mut r = relatorio_ok();
        r.video.adapters = vec![placa("Microsoft Basic Render Driver", false)];
        let problemas = encontrar_problemas(&r);
        assert_eq!(codigos(&problemas), vec!["video_cpu_conversion"]);
        assert!(problemas[0].message.ends_with("Instale o driver da placa de vídeo."));
    }

    #[test]
    fn transmissao_na_cpu_com_video_processor_avisa_as_camadas() {
        let mut r = relatorio_ok();
        r.video.adapters = vec![placa("NVIDIA GeForce GTX 1060 6GB", true)];
        r.video.sharing = transmitindo(&[
            (0, ConverterView::Gpu),
            (1, ConverterView::Cpu),
            (2, ConverterView::Cpu),
        ]);
        let problemas = encontrar_problemas(&r);
        assert_eq!(codigos(&problemas), vec!["share_cpu_conversion"]);
        assert!(
            problemas[0].message.contains("(camada 1, 2)"),
            "{}",
            problemas[0].message
        );
    }

    #[test]
    fn sem_interfaces_gera_network_none() {
        let mut r = relatorio_ok();
        r.network = DiagNetwork {
            interfaces: vec![],
            radmin_ip: None,
        };
        let problemas = encontrar_problemas(&r);
        assert_eq!(codigos(&problemas), vec!["network_none"]);
        assert_eq!(problemas[0].severity, Severity::Error);
    }

    #[test]
    fn sem_radmin() {
        let mut r = relatorio_ok();
        r.network.radmin_ip = None;
        let problemas = encontrar_problemas(&r);
        assert_eq!(codigos(&problemas), vec!["no_radmin"]);
        assert_eq!(problemas[0].severity, Severity::Warn);
    }

    #[test]
    fn firewall_bloqueado_e_nao_liberado() {
        let mut r = relatorio_ok();
        r.firewall.bloqueado = true;
        r.firewall.liberado = false;
        let problemas = encontrar_problemas(&r);
        assert_eq!(codigos(&problemas), vec!["firewall_blocked"]);
        assert_eq!(problemas[0].severity, Severity::Error);
    }

    #[test]
    fn firewall_nao_suportado_nao_gera_codigos_de_firewall() {
        let mut r = relatorio_ok();
        r.firewall.suportado = false;
        r.firewall.bloqueado = true;
        r.firewall.liberado = false;
        r.firewall.erro = Some("x".into());
        r.firewall.bloquear_tudo_por_perfil = [false, true, false];
        assert!(encontrar_problemas(&r).is_empty());
    }

    #[test]
    fn bloquear_tudo_no_perfil_do_radmin() {
        let mut r = relatorio_ok();
        r.firewall.bloquear_tudo_por_perfil = [false, true, false];
        assert_eq!(codigos(&encontrar_problemas(&r)), vec!["firewall_block_all"]);

        // Mesmo bloqueio, mas no perfil de domínio: não é o perfil do Radmin.
        r.firewall.categoria_radmin = Some("Dominio".into());
        assert!(encontrar_problemas(&r).is_empty());
    }

    #[test]
    fn antivirus_e_vpn() {
        let mut r = relatorio_ok();
        r.antivirus = vec!["Defender".into(), "Avast".into()];
        r.vpns = vec!["Hamachi".into()];
        let problemas = encontrar_problemas(&r);
        assert_eq!(codigos(&problemas), vec!["antivirus", "other_vpn"]);
        assert_eq!(problemas[0].message, "Antivírus com firewall próprio: Defender, Avast.");
        assert_eq!(problemas[1].message, "Outras VPNs ativas: Hamachi.");
        assert_eq!(problemas[1].severity, Severity::Info);
    }

    #[test]
    fn amigo_nao_ouve_e_versao_diferente() {
        let mut r = relatorio_ok();
        r.friends.push(DiagFriend {
            name: "Beto".into(),
            version: "2.9.1".into(),
            ip: "26.1.2.5".into(),
            hears_you: false,
            same_version: false,
        });
        let problemas = encontrar_problemas(&r);
        assert_eq!(codigos(&problemas), vec!["friend_not_hearing", "version_mismatch"]);
        assert_eq!(
            problemas[0].message,
            "Beto não está recebendo você (firewall ou antivírus dele?)."
        );
        assert_eq!(problemas[1].message, "Beto usa a versão 2.9.1.");
    }

    #[test]
    fn protocolo_novo() {
        let mut r = relatorio_ok();
        r.newer_protocol = Some(5);
        let problemas = encontrar_problemas(&r);
        assert_eq!(codigos(&problemas), vec!["newer_protocol"]);
        assert_eq!(problemas[0].severity, Severity::Warn);
        assert_eq!(
            problemas[0].message,
            "Alguém na rede usa o protocolo 5, mais novo que o seu (4). Atualize o StreamaZap."
        );
    }

    #[test]
    fn ordem_dos_codigos() {
        let mut r = relatorio_ok();
        r.network.radmin_ip = None;
        r.firewall.erro = Some("acesso negado".into());
        r.firewall.bloqueado = true;
        r.firewall.liberado = false;
        r.firewall.bloquear_tudo_por_perfil = [false, true, false];
        r.antivirus = vec!["Defender".into()];
        r.vpns = vec!["Hamachi".into()];
        r.friends.clear();
        r.newer_protocol = Some(9);
        assert_eq!(
            codigos(&encontrar_problemas(&r)),
            vec![
                "no_radmin",
                "firewall_error",
                "firewall_blocked",
                "firewall_block_all",
                "antivirus",
                "other_vpn",
                "no_friends",
                "newer_protocol",
            ]
        );
    }
}

#[cfg(test)]
mod tests_versoes {
    use std::net::{IpAddr, Ipv4Addr};

    use super::*;

    fn ip(d: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, d))
    }

    #[test]
    fn outras_versoes_ordena_e_marca_newer() {
        let vistas = [(ip(2), 5), (ip(1), 3), (ip(2), 5)];
        let out = outras_versoes_do(&vistas, 4);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].ip, "10.0.0.2");
        assert_eq!(out[0].protocol, 5);
        assert!(out[0].newer);
        assert_eq!(out[1].ip, "10.0.0.1");
        assert_eq!(out[1].protocol, 3);
        assert!(!out[1].newer);
    }

    #[test]
    fn outras_versoes_vazio() {
        assert!(outras_versoes_do(&[], 4).is_empty());
    }

    #[test]
    fn protocolo_maior_pega_o_maior() {
        let vistas = [(ip(1), 3), (ip(2), 5), (ip(3), 6)];
        assert_eq!(protocolo_maior(&vistas, 4), Some(6));
    }

    #[test]
    fn protocolo_maior_none_quando_nenhum() {
        let vistas = [(ip(1), 3), (ip(2), 4)];
        assert_eq!(protocolo_maior(&vistas, 4), None);
    }

    #[test]
    fn protocolo_maior_lista_vazia() {
        assert_eq!(protocolo_maior(&[], 4), None);
    }
}

#[cfg(test)]
mod tests_montar {
    use std::net::{IpAddr, Ipv4Addr};

    use zap_sys::FirewallStatus;

    use super::*;
    use crate::snapshot::{Participant, RoomView};
    use crate::types::RoomMode;

    fn firewall_liberado() -> FirewallStatus {
        FirewallStatus {
            suportado: true,
            bloqueado: false,
            liberado: true,
            bloquear_tudo_por_perfil: [false; 3],
            categoria_radmin: Some("Private".into()),
            regras: vec![],
            erro: None,
        }
    }

    fn entrada() -> DiagInput {
        let pessoa = |id: &str| Participant {
            id: id.into(),
            name: id.into(),
            color: "#fff".into(),
            ping_ms: 10,
            sharing: false,
            camera: false,
            volume: None,
            muted: None,
        };
        DiagInput {
            app_version: "3.0.0".into(),
            protocol: 4,
            interfaces: vec![
                Interface {
                    nome: "Ethernet".into(),
                    ip: Ipv4Addr::new(192, 168, 0, 10),
                    broadcast: None,
                    radmin: false,
                },
                Interface {
                    nome: "Radmin VPN".into(),
                    ip: Ipv4Addr::new(26, 1, 2, 3),
                    broadcast: Some(Ipv4Addr::new(26, 255, 255, 255)),
                    radmin: true,
                },
            ],
            vpns: vec![],
            antivirus: vec![],
            firewall: firewall_liberado(),
            peers: vec![PeerListing {
                name: "Ana".into(),
                version: "3.0.0".into(),
                ip: "26.1.2.4".into(),
                receives_you: true,
            }],
            rooms: vec![RoomListing::basic(
                "zap-x".into(),
                "Sala".into(),
                "Ana".into(),
                2,
                false,
                RoomMode::Online,
            )],
            room: Some(RoomView {
                id: "zap-x".into(),
                name: "Sala".into(),
                mode: RoomMode::Online,
                code: None,
                participants: vec![pessoa("a")],
                tracks: vec![],
                chat: vec![],
                unread: 0,
                me_id: String::new(),
            }),
            other_versions: vec![(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)), 5)],
            video_adapters: vec![zap_media::VideoAdapter {
                name: "NVIDIA GeForce GTX 1060 6GB".into(),
                video_processor: true,
            }],
            share: None,
        }
    }

    #[test]
    fn montar_leva_placas_e_transmissao() {
        use crate::snapshot::{ConverterView, LayerConversion, ShareView};
        let mut e = entrada();
        let r = montar_relatorio(&e);
        assert_eq!(r.video.adapters.len(), 1);
        assert!(r.video.adapters[0].video_processor);
        assert!(r.video.sharing.is_none());

        e.share = Some(ShareView {
            source: "screen:1".into(),
            audio: "Sem som".into(),
            preset: crate::types::Preset::P1080p60,
            encoder: "NVIDIA H.264 Encoder MFT".into(),
            layers: vec![0],
            viewers_per_layer: vec![1, 0, 0],
            encode_ms: 2.0,
            conversion: vec![LayerConversion {
                layer: 0,
                converter: ConverterView::Gpu,
            }],
            request: None,
        });
        let r = montar_relatorio(&e);
        let t = r.video.sharing.clone().expect("transmissão");
        assert_eq!(t.encoder, "NVIDIA H.264 Encoder MFT");
        assert_eq!(t.conversion.len(), 1);
        let json = serde_json::to_value(&r.video).expect("json");
        assert_eq!(json["adapters"][0]["video_processor"], true);
        assert_eq!(json["sharing"]["conversion"][0]["converter"], "gpu");
    }

    #[test]
    fn monta_relatorio_completo() {
        let r = montar_relatorio(&entrada());
        assert_eq!(r.app_version, "3.0.0");
        assert_eq!(r.protocol, 4);
        assert_eq!(r.network.interfaces.len(), 2);
        assert_eq!(r.network.radmin_ip.as_deref(), Some("26.1.2.3"));
        assert_eq!(r.friends.len(), 1);
        assert!(r.friends[0].same_version);
        assert_eq!(r.rooms.len(), 1);
        assert_eq!(r.current_room.as_ref().map(|s| s.participants), Some(1));
        assert_eq!(r.other_versions.len(), 1);
        assert!(r.other_versions[0].newer);
        assert_eq!(r.newer_protocol, Some(5));
        assert!(r.problems.iter().any(|p| p.code == "newer_protocol"));
    }

    #[test]
    fn monta_relatorio_serializa_json_roundtrip() {
        let original = montar_relatorio(&entrada());
        let json = serde_json::to_string(&original).expect("serializa");
        let volta: DiagReport = serde_json::from_str(&json).expect("desserializa");
        assert_eq!(volta, original);
    }

    #[test]
    fn vpns_ordenadas_sem_repeticao() {
        let mut input = entrada();
        input.vpns = vec!["WireGuard".into(), "NordVPN".into(), "WireGuard".into()];
        let r = montar_relatorio(&input);
        assert_eq!(r.vpns, vec!["NordVPN".to_string(), "WireGuard".to_string()]);
    }
}
