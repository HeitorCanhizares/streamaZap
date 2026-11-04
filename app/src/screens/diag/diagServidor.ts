// Card "Servidor online" do Diagnóstico a partir do snapshot. Tarefa ui-online-08.
import type { Snapshot } from '../../bridge/types'
import type { Check, LinhaCheck } from './diagModel'

export function checkServidor(snap: Snapshot): Check {
  const server = snap.server
  const base = { id: 'srv' as const, title: 'Servidor online', short: 'Servidor' }

  // Sem endereço configurado: ainda não é erro, só informação (status 'soon').
  if (!server || server.state === 'off') {
    return {
      ...base,
      status: 'soon',
      lines: [{
        tom: 'info',
        text: 'Servidor não configurado',
        sub: 'Informe o endereço em Configurações › Conexão para usar salas Online.',
      }],
    }
  }

  if (server.state === 'connecting') {
    return {
      ...base,
      status: 'warn',
      lines: [{ tom: 'warn', text: 'Conectando ao servidor…' }],
    }
  }

  if (server.state === 'online') {
    // Ping só aparece quando o número veio do servidor; sem ele a propriedade não existe.
    const linha: LinhaCheck = {
      tom: 'ok',
      text: `Conectado${server.name ? ' a ' + server.name : ''}`,
    }
    if (typeof server.ping_ms === 'number' && Number.isFinite(server.ping_ms)) {
      linha.badge = `${Math.round(server.ping_ms)} ms`
    }
    return { ...base, status: 'ok', lines: [linha] }
  }

  // 'error': a mensagem do servidor, quando existir, vira o detalhe.
  const linha: LinhaCheck = { tom: 'bad', text: 'Servidor desconectado' }
  if (server.message) linha.sub = server.message
  return { ...base, status: 'warn', lines: [linha] }
}
