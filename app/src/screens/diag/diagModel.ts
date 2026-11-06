// Modelo puro da tela Diagnóstico: transforma o Snapshot na lista de checagens exibida.
import type { Snapshot } from '../../bridge/types'

/** Tom de cada linha/checagem (TONE do design: ok verde, warn amarelo, bad vermelho, info cinza). */
export type Tom = 'ok' | 'warn' | 'bad' | 'info'
/** Estado de uma checagem inteira. 'soon' = ainda não existe (Servidor e Firewall). */
export type StatusCheck = 'ok' | 'warn' | 'soon'

export interface LinhaCheck { tom: Tom; text: string; sub?: string; badge?: string }
export interface Check {
  id: 'srv' | 'net' | 'fw' | 'peers' | 'rooms' | 'video'
  title: string
  short: string
  status: StatusCheck
  lines: LinhaCheck[]
}
export interface Veredito { titulo: string; sub: string; cor: string; avisos: number }

/** As 5 checagens, nesta ordem: srv, net, fw, peers, rooms. Com `online` falso o card 'srv' não
 * entra (4 itens) e nenhum texto cita o modo Online. */
export function montarChecks(snap: Snapshot, online = true): Check[] {
  const interfaces = snap.network.interfaces
  const radmin = snap.network.radmin_ip

  // Rede: uma linha por interface; sem Radmin é aviso (o modo Online segue funcionando).
  const linhasRede: LinhaCheck[] = interfaces.length > 0
    ? interfaces.map((nome) => ({ tom: 'ok' as const, text: nome }))
    : [{ tom: 'bad', text: 'Nenhuma interface de rede detectada' }]
  if (radmin) {
    linhasRede.push({ tom: 'ok', text: 'Radmin VPN · ' + radmin })
  } else {
    linhasRede.push({
      tom: 'warn',
      text: 'Radmin não detectado',
      ...(online ? { sub: 'O modo Online continua funcionando sem ele.' } : {}),
    })
  }

  // Amigos: quem não recebe nada do seu PC indica bloqueio no PC dele, então é aviso.
  const linhasPeers: LinhaCheck[] = snap.peers.length === 0
    ? [{ tom: 'info', text: 'Nenhum amigo com o app aberto agora.' }]
    : snap.peers.map((p) => {
        const text = `${p.name} · ${p.ip} · v${p.version}`
        return p.receives_you
          ? { tom: 'ok' as const, text, badge: 'recebe você' }
          : { tom: 'bad' as const, text, badge: 'não recebe', sub: 'Não recebe nada do seu PC — o bloqueio está no PC dele.' }
      })
  const algumNaoRecebe = snap.peers.some((p) => !p.receives_you)

  // Salas: só informativo, status sempre 'ok'.
  const linhasSalas: LinhaCheck[] = snap.rooms.length === 0
    ? [{ tom: 'info', text: 'Nenhuma sala descoberta ainda.' }]
    : snap.rooms.map((r) => ({
        tom: 'ok' as const,
        text: `${r.name} — ${r.host}`,
        sub: `${r.mode === 'online' ? 'online' : 'local'} · ${r.room_id}`,
      }))

  const srv: Check = {
    id: 'srv',
    title: 'Servidor online',
    short: 'Servidor',
    status: 'soon',
    lines: [{ tom: 'info', text: 'Disponível em breve' }],
  }
  const lista: Check[] = [
    {
      id: 'net',
      title: 'Rede',
      short: 'Rede',
      status: interfaces.length === 0 || !radmin ? 'warn' : 'ok',
      lines: linhasRede,
    },
    {
      id: 'fw',
      title: 'Firewall do Windows',
      short: 'Firewall',
      status: 'soon',
      lines: [{ tom: 'info', text: 'Disponível em breve' }],
    },
    {
      id: 'peers',
      title: 'Amigos com o app aberto',
      short: 'Amigos',
      status: algumNaoRecebe ? 'warn' : 'ok',
      lines: linhasPeers,
    },
    {
      id: 'rooms',
      title: 'Salas encontradas',
      short: 'Salas',
      status: 'ok',
      lines: linhasSalas,
    },
  ]
  return online ? [srv, ...lista] : lista
}

/** Veredito do resumo. `verificando` = true durante o "Verificar de novo". */
export function calcularVeredito(checks: Check[], verificando: boolean, semRede: boolean, online = true): Veredito {
  const avisos = checks.filter((c) => c.status === 'warn').length
  if (verificando) {
    return {
      titulo: 'Verificando sua conexão…',
      sub: online ? 'Olhando servidor, Radmin, firewall e quem está online.' : 'Olhando Radmin, firewall e quem está por aqui.',
      cor: '#A3BDB1',
      avisos,
    }
  }
  if (semRede) {
    return {
      titulo: 'Sem rede',
      sub: 'Nenhuma interface de rede foi detectada. Confira a conexão do PC.',
      cor: '#FF8592',
      avisos,
    }
  }
  if (avisos === 0) {
    return {
      titulo: 'Tudo certo pra transmitir',
      sub: 'Seu PC recebe e envia sem bloqueio.',
      cor: '#3BF08A',
      avisos,
    }
  }
  // Só o vídeo em amarelo: a conexão está boa, o aviso é de uso de processador.
  const soVideo = checks.every((c) => c.status !== 'warn' || c.id === 'video')
  return {
    titulo: 'Quase lá · ' + avisos + (avisos === 1 ? ' aviso' : ' avisos'),
    sub: soVideo
      ? 'Dá pra transmitir, mas a conversão de cor vai pela CPU e usa mais processador. Veja o item em amarelo.'
      : 'Dá pra entrar nas salas, mas alguns amigos podem não te ver. Veja os itens em amarelo.',
    cor: '#FFB547',
    avisos,
  }
}

const ROTULO_STATUS: Record<StatusCheck, string> = {
  ok: 'Tudo certo',
  warn: 'Atenção',
  soon: 'em breve',
}

/** Texto do relatório copiado (uma linha por checagem e por linha de detalhe). */
export function montarRelatorio(checks: Check[], veredito: Veredito): string {
  const out: string[] = ['streamaZap · diagnóstico', veredito.titulo, '']
  checks.forEach((c, i) => {
    out.push(`## ${c.title} — ${ROTULO_STATUS[c.status]}`)
    for (const l of c.lines) {
      let linha = `- ${l.text}`
      if (l.sub) linha += ` (${l.sub})`
      if (l.badge) linha += ` [${l.badge}]`
      out.push(linha)
    }
    if (i < checks.length - 1) out.push('')
  })
  return out.join('\n')
}
