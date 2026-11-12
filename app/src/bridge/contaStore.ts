// Estado da conta para a interface (mesmo padrão de onlineStore.ts: useSyncExternalStore). Tarefa front-07.
import { useSyncExternalStore } from 'react'
import type { ContaEstado } from './contaTipos'
import { PROVEDORES_OAUTH } from './contaModel'

/**
 * Estado inicial: { sessao: 'carregando', email: null, identidades: [], aviso: null, otpEmail: null,
 * ocupado: false, provedores: PROVEDORES_OAUTH }.
 */
export function estadoInicialConta(): ContaEstado {
  return {
    sessao: 'carregando',
    email: null,
    identidades: [],
    aviso: null,
    otpEmail: null,
    ocupado: false,
    provedores: PROVEDORES_OAUTH,
  }
}

let current: ContaEstado = estadoInicialConta()
const listeners = new Set<() => void>()

function subscribe(cb: () => void): () => void {
  listeners.add(cb)
  return () => {
    listeners.delete(cb)
  }
}

function notificar(): void {
  for (const cb of listeners) cb()
}

/** Estado atual (mesma referência enquanto nada mudar). */
export function getContaEstado(): ContaEstado {
  return current
}

/**
 * Aplica um patch (cria objeto novo e avisa os inscritos; patch sem mudança não avisa).
 * Comparação por `===`: arrays são comparados por referência.
 */
export function setContaEstado(patch: Partial<ContaEstado>): void {
  const mudou = (Object.keys(patch) as (keyof ContaEstado)[]).some((k) => patch[k] !== current[k])
  // Sem mudança não avisa: evita re-render à toa dos componentes inscritos.
  if (!mudou) return
  current = { ...current, ...patch }
  notificar()
}

/** Hook: re-renderiza quando o estado muda. */
export function useContaEstado(): ContaEstado {
  return useSyncExternalStore(subscribe, getContaEstado)
}

/** Só para testes: volta ao estado inicial e avisa. */
export function resetConta(): void {
  current = estadoInicialConta()
  notificar()
}
