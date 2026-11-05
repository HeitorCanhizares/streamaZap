//! Coleta do diagnóstico no sistema e na descoberta. Bloqueante (COM/WMI): chamar de
//! `spawn_blocking` na camada do app.

use std::path::Path;

use crate::diag::{DiagInput, DiagReport};
use crate::engine::Core;

/// Lê rede, VPNs, antivírus e firewall (zap-sys), as placas de vídeo (zap-media) e a
/// descoberta/estado do núcleo.
///
/// **Bloqueante**: as consultas COM/WMI do zap-sys são lentas; chame de `spawn_blocking`.
/// Nenhum lock é segurado durante essas chamadas (usa o snapshot atual e um clone do `Arc`
/// da descoberta), para não travar o resto do núcleo.
pub fn coletar(core: &Core, exe: &Path) -> DiagInput {
    let snap = core.snapshot();
    // Clona o Arc e solta o lock na mesma instrução.
    let discovery = core.shared.discovery.lock().clone();
    let other_versions = match discovery {
        Some(d) => d.other_versions(),
        None => Vec::new(),
    };
    DiagInput {
        app_version: env!("CARGO_PKG_VERSION").into(),
        protocol: zap_proto::PROTOCOL_VERSION,
        interfaces: zap_sys::net::interfaces_ipv4(),
        vpns: zap_sys::vpn::outras_vpns(),
        antivirus: zap_sys::antivirus::firewalls_de_terceiros(),
        firewall: zap_sys::status(exe),
        peers: snap.peers.clone(),
        rooms: snap.rooms.clone(),
        room: snap.room.clone(),
        other_versions,
        video_adapters: zap_media::probe_video_adapters(),
        share: snap.share.clone(),
    }
}

impl Core {
    /// Relatório completo: `montar_relatorio(&coletar(self, exe))`. `exe` é o executável do app.
    pub fn diagnostics(&self, exe: &Path) -> DiagReport {
        crate::diag_build::montar_relatorio(&coletar(self, exe))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::CoreOptions;
    use crate::testutil::TestSink;

    fn core_teste() -> (Core, std::path::PathBuf) {
        let d = std::env::temp_dir().join(format!("zapcore-diag-{}", uuid::Uuid::new_v4()));
        let opts = CoreOptions {
            discovery: false,
            broadcast: false,
            include_loopback: true,
            audio_out: false,
            ..Default::default()
        };
        let core = Core::start_with(d.clone(), TestSink::new(), opts).unwrap();
        (core, d)
    }

    #[test]
    fn coletar_sem_descoberta() {
        let (core, d) = core_teste();
        let input = coletar(&core, Path::new("streamazap.exe"));
        assert!(input.other_versions.is_empty());
        assert_eq!(input.protocol, zap_proto::PROTOCOL_VERSION);
        assert_eq!(input.app_version, env!("CARGO_PKG_VERSION"));
        drop(core);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn diagnostics_devolve_relatorio_serializavel() {
        let (core, d) = core_teste();
        let report = core.diagnostics(Path::new("streamazap.exe"));
        assert!(serde_json::to_string(&report).is_ok());
        #[cfg(not(windows))]
        assert!(!report.firewall.suportado);
        // A sonda de vídeo roda de verdade; sem transmissão, não há bloco `sharing`.
        eprintln!("placas: {:?}", report.video.adapters);
        assert!(report.video.sharing.is_none());
        #[cfg(not(windows))]
        assert!(report.video.adapters.is_empty());
        drop(core);
        let _ = std::fs::remove_dir_all(d);
    }
}
