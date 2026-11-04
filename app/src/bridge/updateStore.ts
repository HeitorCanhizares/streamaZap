// Estado da atualização: escuta `zap://update` (ou mock no navegador). Tarefa ui5-02.
// Mesmo padrão de store.ts: uma referência só, trocada por setUpdateState, e um ouvinte
// Tauri que existe só enquanto houver algum componente inscrito.
import { useSyncExternalStore } from 'react'
import { isTauri } from './api'
import type { UpdateState } from './types'

export function nadaState(): UpdateState {
  return { fase: 'nada', progresso: 0, versao: null, erro: null }
}

let current: UpdateState = nadaState()
const listeners = new Set<() => void>()

// Ouvinte do evento Tauri: existe so enquanto houver algum componente inscrito.
let stopListen: (() => void) | null = null
let pending: Promise<void> | null = null

/** Aplica um estado vindo do Rust (ou de um teste) e avisa os inscritos. */
export function setUpdateState(s: UpdateState): void {
  current = s
  for (const cb of listeners) cb()
}

export function getUpdateState(): UpdateState {
  return current
}

function startListening(): void {
  if (pending || !isTauri()) return
  // Import dinamico: o modulo de eventos so carrega dentro do app.
  pending = import('@tauri-apps/api/event')
    .then(({ listen }) => listen<UpdateState>('zap://update', (e) => setUpdateState(e.payload)))
    .then(
      (un) => {
        pending = null
        // Se todos saíram enquanto o listen subia, desfaz logo.
        if (listeners.size === 0) un()
        else stopListen = un
      },
      () => {
        pending = null
      },
    )
}

function stopListening(): void {
  if (stopListen) {
    stopListen()
    stopListen = null
  }
}

function subscribe(cb: () => void): () => void {
  listeners.add(cb)
  if (listeners.size === 1) startListening()
  return () => {
    if (!listeners.delete(cb)) return
    if (listeners.size === 0) stopListening()
  }
}

/** Hook: estado atual da atualização; re-renderiza a cada evento. */
export function useUpdateState(): UpdateState {
  return useSyncExternalStore(subscribe, getUpdateState)
}

/** Só para testes: volta ao estado inicial ("nada"). */
export function resetUpdateState(): void {
  current = nadaState()
}
