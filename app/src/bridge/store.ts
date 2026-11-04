// useSnapshot() sobre listen("zap://state") ou o mock (tarefa ui-09).
import { useSyncExternalStore } from 'react'
import { isTauri } from './api'
import { mockSnapshot } from './mock'
import type { Snapshot } from './types'

// Estado unico do front. Trocado so por setSnapshot, para getSnapshot devolver
// sempre a mesma referencia enquanto nada mudar (exigido pelo useSyncExternalStore).
let current: Snapshot = mockSnapshot()
const listeners = new Set<() => void>()

// Ouvinte do evento Tauri: existe so enquanto houver algum componente inscrito.
let stopListen: (() => void) | null = null
let pending: Promise<void> | null = null

// Aplica um snapshot vindo do Rust. Versao menor que a atual e atrasada: ignora.
export function setSnapshot(s: Snapshot): void {
  if (s.version < current.version) return
  current = s
  for (const cb of listeners) cb()
}

export function getSnapshot(): Snapshot {
  return current
}

function startListening(): void {
  if (pending || !isTauri()) return
  // Import dinamico: o modulo de eventos so carrega dentro do app.
  pending = import('@tauri-apps/api/event')
    .then(({ listen }) => listen<Snapshot>('zap://state', (e) => setSnapshot(e.payload)))
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

export function subscribe(cb: () => void): () => void {
  listeners.add(cb)
  if (listeners.size === 1) startListening()
  return () => {
    if (!listeners.delete(cb)) return
    if (listeners.size === 0) stopListening()
  }
}

export function useSnapshot(): Snapshot {
  return useSyncExternalStore(subscribe, getSnapshot)
}

// So para testes: volta ao snapshot do mock.
export function resetSnapshotForTests(): void {
  current = mockSnapshot()
}
