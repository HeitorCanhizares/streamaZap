// Checagens do Diagnóstico a partir do DiagReport real (tarefa ui5-04).
import type { DiagReport, Snapshot } from '../../bridge/types'
import { montarChecks } from './diagModel'
import type { Check, LinhaCheck } from './diagModel'
import { checkServidor } from './diagServidor'

const PERFIS = ['Domínio', 'Privado', 'Público']

function checkRede(rel: DiagReport): Check {
  const { interfaces, radmin_ip } = rel.network
  const lines: LinhaCheck[] = interfaces.length > 0
    ? interfaces.map((i) => ({ tom: 'ok' as const, text: `${i.name} · ${i.ip}` + (i.radmin ? ' · Radmin' : '') }))
    : [{ tom: 'bad', text: 'Nenhuma interface de rede detectada' }]
  if (radmin_ip) {
    lines.push({ tom: 'ok', text: `Radmin VPN · ${radmin_ip}` })
  } else {
    lines.push({ tom: 'warn', text: 'Radmin não detectado', sub: 'O modo Online continua funcionando sem ele.' })
  }
  for (const v of rel.vpns) lines.push({ tom: 'warn', text: `VPN ativa: ${v}`, sub: 'Pode desviar o tráfego das salas.' })
  for (const a of rel.antivirus) lines.push({ tom: 'warn', text: `Antivírus com firewall próprio: ${a}` })
  const aviso = interfaces.length === 0 || !radmin_ip || rel.vpns.length > 0 || rel.antivirus.length > 0
  return { id: 'net', title: 'Rede', short: 'Rede', status: aviso ? 'warn' : 'ok', lines }
}

function checkFirewall(rel: DiagReport): Check {
  const base = { id: 'fw' as const, title: 'Firewall do Windows', short: 'Firewall' }
  const fw = rel.firewall
  if (!fw.suportado) {
    return { ...base, status: 'ok', lines: [{ tom: 'info', text: 'Firewall do Windows não se aplica neste sistema' }] }
  }
  if (fw.erro) {
    return { ...base, status: 'warn', lines: [{ tom: 'warn', text: 'Não foi possível ler o firewall', sub: fw.erro }] }
  }
  const lines: LinhaCheck[] = []
  if (fw.bloqueado) lines.push({ tom: 'bad', text: 'Há regra bloqueando o streamaZap' })
  if (fw.liberado) lines.push({ tom: 'ok', text: 'Regra de permissão do streamaZap encontrada' })
  else lines.push({ tom: 'warn', text: 'Sem regra de permissão para o streamaZap' })
  const bloqueios = fw.bloquear_tudo_por_perfil
  bloqueios.forEach((b, i) => {
    if (b) lines.push({ tom: 'warn', text: `Bloquear todas as conexões de entrada ligado no perfil ${PERFIS[i]}` })
  })
  if (fw.categoria_radmin) lines.push({ tom: 'info', text: `Rede do Radmin: ${fw.categoria_radmin}` })
  const aviso = fw.bloqueado || !fw.liberado || bloqueios.some(Boolean)
  return { ...base, status: aviso ? 'warn' : 'ok', lines }
}

function checkAmigos(rel: DiagReport): Check {
  const lines: LinhaCheck[] = rel.friends.length === 0
    ? [{ tom: 'info', text: 'Nenhum amigo com o app aberto agora.' }]
    : rel.friends.map((f) => {
        const text = `${f.name} · ${f.ip} · v${f.version}`
        if (!f.hears_you) {
          return { tom: 'bad' as const, text, badge: 'não recebe', sub: 'Não recebe nada do seu PC — o bloqueio está no PC dele.' }
        }
        const l: LinhaCheck = { tom: 'ok', text, badge: 'recebe você' }
        if (!f.same_version) l.sub = `Versão diferente (v${f.version})`
        return l
      })
  return {
    id: 'peers', title: 'Amigos com o app aberto', short: 'Amigos',
    status: rel.friends.some((f) => !f.hears_you) ? 'warn' : 'ok', lines,
  }
}

function checkSalas(rel: DiagReport): Check {
  const lines: LinhaCheck[] = rel.rooms.length === 0
    ? [{ tom: 'info', text: 'Nenhuma sala descoberta ainda.' }]
    : rel.rooms.map((r) => r.version_warn
        ? { tom: 'warn' as const, text: `${r.name} — ${r.host}`, sub: r.version_warn }
        : { tom: 'ok' as const, text: `${r.name} — ${r.host}`, sub: `${r.participants} pessoas · ${r.id}` })
  if (rel.newer_protocol != null) {
    lines.push({
      tom: 'warn',
      text: `Há uma versão mais nova do app na rede (protocolo ${rel.newer_protocol})`,
      sub: 'Atualize em Configurações.',
    })
  }
  return {
    id: 'rooms', title: 'Salas encontradas', short: 'Salas',
    status: rel.newer_protocol != null ? 'warn' : 'ok', lines,
  }
}

/** As 5 checagens (srv, net, fw, peers, rooms) com os dados do relatório; sem relatório, cai no snapshot. */
export function montarChecksReal(rel: DiagReport | null, snap: Snapshot): Check[] {
  if (rel === null) return montarChecks(snap)
  return [
    checkServidor(snap),
    checkRede(rel),
    checkFirewall(rel),
    checkAmigos(rel),
    checkSalas(rel),
  ]
}
