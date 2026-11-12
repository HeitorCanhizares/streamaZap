// Convite de sala por link (plano contas-planos-camera §I2). O link compartilhado é https, porque
// WhatsApp e Discord não deixam clicar em `streamazap://`: a página `sala/` do site abre
// `streamazap://sala/<alvo>`, e o app (que valida o alvo no Rust) entra na sala.
import { normalizarCodigo } from './serverModel'
import type { JoinTarget } from './types'

export { linkConvite } from './site'

// Porta padrão da sala Local (`zap_proto::DEFAULT_MEDIA_PORT`).
const PORTA_PADRAO = 47800

const IPV4 = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/

/**
 * Alvo do convite (`ZAP-7K2Q`, `7k2q`, `26.14.7.88` ou `26.14.7.88:47800`) para o `room_join`.
 * Código vira `{ code }` normalizado; IPv4 vira `{ ip, port }` (sem porta: 47800). Inválido: null.
 */
export function alvoParaEntrar(alvo: string): JoinTarget | null {
  const limpo = alvo.trim()
  const codigo = normalizarCodigo(limpo)
  if (codigo !== null) return { code: codigo }
  const i = limpo.lastIndexOf(':')
  const ip = i === -1 ? limpo : limpo.slice(0, i)
  const portaTxt = i === -1 ? '' : limpo.slice(i + 1)
  const m = IPV4.exec(ip)
  if (m === null || m.slice(1).some((p) => Number(p) > 255)) return null
  if (portaTxt !== '' && !/^\d{1,5}$/.test(portaTxt)) return null
  const port = portaTxt === '' ? PORTA_PADRAO : Number(portaTxt)
  if (port < 1 || port > 65535) return null
  return { ip, port }
}

/** Como o alvo aparece para a pessoa: o código, ou `ip:porta`. */
export function rotuloAlvo(alvo: JoinTarget): string {
  if ('code' in alvo) return alvo.code
  if ('ip' in alvo) return `${alvo.ip}:${alvo.port}`
  return alvo.room_id
}
