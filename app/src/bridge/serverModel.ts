// Funções puras do modo Online na interface (frente ui-online). Tarefa ui-online-01.
import { linkConvite } from './site'
import type { ServerStatus, ServerTestResult } from './types'

export type TomServidor = 'ok' | 'warn' | 'bad' | 'off'
export interface BarraServidor { texto: string; tom: TomServidor; ms: number | null }
export interface TextoTeste { texto: string; cor: string }

const CINZA = '#7E978B'
const CLARO = '#A3BDB1'
const VERDE = '#3BF08A'
const VERMELHO = '#FF8592'

// Sem 0, O, 1, I, L: evita confusão ao ler o código em voz alta ou em tela pequena.
const ALFABETO_CODIGO = /^[23456789ABCDEFGHJKMNPQRSTUVWXYZ]{4}$/

/** Texto e tom do status do servidor na barra de título. */
export function barraServidor(server: ServerStatus | undefined): BarraServidor {
  if (!server || server.state === 'off') {
    return { texto: 'Servidor · não configurado', tom: 'off', ms: null }
  }
  switch (server.state) {
    case 'connecting':
      return { texto: 'Servidor · conectando…', tom: 'warn', ms: null }
    case 'online': {
      const ping = server.ping_ms
      if (typeof ping === 'number' && Number.isFinite(ping)) {
        const ms = Math.round(ping)
        return { texto: `Servidor online · ${ms} ms`, tom: 'ok', ms }
      }
      return { texto: 'Servidor online', tom: 'ok', ms: null }
    }
    case 'error':
      return { texto: 'Servidor · desconectado', tom: 'bad', ms: null }
    default:
      return { texto: 'Servidor · não configurado', tom: 'off', ms: null }
  }
}

/** Texto e cor do resultado de "Testar conexão" (null = ainda não testado). */
export function textoTeste(res: ServerTestResult | null, testando: boolean): TextoTeste {
  if (testando) return { texto: 'Conectando…', cor: CLARO }
  if (!res) return { texto: 'Ainda não testado', cor: CINZA }
  if (res.ok) {
    const ping = res.ping_ms
    const texto = typeof ping === 'number' && Number.isFinite(ping)
      ? `Conectado · senha aceita · ${Math.round(ping)} ms`
      : 'Conectado · senha aceita'
    return { texto, cor: VERDE }
  }
  return { texto: res.message || 'Não foi possível conectar', cor: VERMELHO }
}

/** 'zap-7k2q', ' 7K2Q ' -> 'ZAP-7K2Q'; formato inválido -> null. */
export function normalizarCodigo(texto: string): string | null {
  const s = texto.trim().toUpperCase()
  // Prefixo "ZAP" e hífen/espaços são opcionais; o código tem sempre 4 caracteres.
  const m = s.match(/^(?:ZAP[\s-]*)?([^\s-]{4})$/)
  if (!m) return null
  const codigo = m[1]
  return ALFABETO_CODIGO.test(codigo) ? `ZAP-${codigo}` : null
}

/**
 * Texto do convite copiado para a área de transferência: o link (abre o app e entra na sala) e o
 * código, para quem preferir digitar.
 */
export function conviteTexto(codigo: string, nomeSala: string): string {
  const nome = nomeSala.trim()
  const fim = `${linkConvite(codigo)} (código ${codigo})`
  if (!nome) return `Entra na minha sala no streamaZap: ${fim}`
  return `Entra na minha sala "${nome}" no streamaZap: ${fim}`
}
