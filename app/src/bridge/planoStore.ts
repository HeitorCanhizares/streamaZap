// Estado de planos e cobrança para a interface (mesmo molde de contaStore.ts).
import { useSyncExternalStore } from 'react'
import type { PlanoEstado } from './planoTipos'
import { CONFIG_DESLIGADA } from './planoModel'

export function estadoInicialPlano(): PlanoEstado {
  return {
    config: CONFIG_DESLIGADA,
    assinatura: null,
    telaAberta: false,
    destaque: null,
    aguardandoPagamento: false,
    comprado: null,
    ocupado: false,
    aviso: null,
    info: null,
  }
}

let current: PlanoEstado = estadoInicialPlano()
const listeners = new Set<() => void>()

function subscribe(cb: () => void): () => void {
  listeners.add(cb)
  return () => {
    listeners.delete(cb)
  }
}

export function getPlanoEstado(): PlanoEstado {
  return current
}

/** Aplica um patch por `===`; sem mudança não avisa. */
export function setPlanoEstado(patch: Partial<PlanoEstado>): void {
  const mudou = (Object.keys(patch) as (keyof PlanoEstado)[]).some((k) => patch[k] !== current[k])
  if (!mudou) return
  current = { ...current, ...patch }
  for (const cb of listeners) cb()
}

export function usePlanoEstado(): PlanoEstado {
  return useSyncExternalStore(subscribe, getPlanoEstado)
}

/** Só para testes e para a limpeza do `contaSync`. */
export function resetPlano(): void {
  current = estadoInicialPlano()
  for (const cb of listeners) cb()
}
