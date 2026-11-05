// Dados de exemplo do diagnóstico e do update para o navegador (dev/testes). Tarefa ui5-01.
import type { DiagReport } from './types'
import { mockSnapshot } from './mock'

/** Relatório de exemplo coerente com o mockSnapshot (Radmin, 4 amigos, 1 não recebe). */
export function mockDiagReport(): DiagReport {
  const snap = mockSnapshot()
  return {
    app_version: '3.0.0',
    protocol: 3,
    network: {
      interfaces: [
        { name: 'Radmin VPN', ip: '26.100.5.20', broadcast: '26.255.255.255', radmin: true },
        { name: 'Ethernet', ip: '192.168.0.10', radmin: false },
      ],
      radmin_ip: '26.100.5.20',
    },
    vpns: [],
    antivirus: [],
    firewall: {
      suportado: true,
      bloqueado: false,
      liberado: true,
      bloquear_tudo_por_perfil: [false, false, false],
      categoria_radmin: 'Privada',
      regras: [],
      erro: null,
    },
    // Mesmos 4 peers do mockSnapshot: quem não recebe o nosso áudio/vídeo é o Rafa.
    friends: snap.peers.map((p) => ({
      name: p.name,
      version: p.version,
      ip: p.ip,
      hears_you: p.receives_you,
      same_version: p.version === '3.0.0',
    })),
    // Salas do mockSnapshot (id, nome, anfitrião, participantes); has_password fixo em false, como na spec.
    rooms: snap.rooms.map((r) => ({
      id: r.room_id,
      name: r.name,
      host: r.host,
      participants: r.participants,
      has_password: false,
    })),
    other_versions: [],
    video: {
      adapters: [{ name: 'NVIDIA GeForce GTX 1060 6GB', video_processor: true }],
    },
    problems: [
      {
        severity: 'warn',
        code: 'friend_no_hear',
        message: 'Rafa não recebe nada do seu PC — o bloqueio está no PC dele.',
      },
    ],
  }
}

/** Texto do relatório para copiar, no formato do Rust (zap_core::diag_text). */
export function mockDiagText(): string {
  const r = mockDiagReport()
  const linhas: string[] = [
    'streamaZap · relatório de diagnóstico',
    `Versão: ${r.app_version} (protocolo ${r.protocol})`,
    `Radmin: ${r.network.radmin_ip ?? 'não encontrado'}`,
    `Firewall: ${r.firewall.liberado ? 'liberado' : 'não liberado'} (rede ${r.firewall.categoria_radmin ?? '?'})`,
    'Amigos:',
    ...r.friends.map(
      (f) => `  ${f.name} · ${f.ip} · recebe você: ${f.hears_you ? 'sim' : 'não'}`,
    ),
    'Vídeo:',
    ...r.video.adapters.map(
      (a) => `  ${a.name}: conversão ${a.video_processor ? 'na GPU (Video Processor)' : 'pela CPU (sem Video Processor)'}`,
    ),
    'Problemas:',
    ...r.problems.map((p) => `  [${p.severity}] ${p.message}`),
  ]
  return linhas.join('\n') + '\n'
}
