// Estado dos eventos `zap://update-aviso` (aviso de 30 s) e `zap://update-aplicada` ("Atualizado
// para a vX"). Mesmo padrão de updateStore.ts. Tarefa front-12.
import { useSyncExternalStore } from 'react'
import { api, isTauri } from './api'
import type { UpdateAplicada, UpdateAviso } from './types'

let aviso: UpdateAviso | null = null
let aplicada: UpdateAplicada | null = null
// O Rust entrega a atualização aplicada uma única vez: só pedimos na primeira inscrição.
let pedidoFeito = false

/** Canal de um evento Tauri: inscritos + ouvinte que existe só enquanto houver inscritos. */
function criarCanal<T>(evento: string, aplicar: (p: T) => void, aoPrimeiro?: () => void) {
  const listeners = new Set<() => void>()
  let stopListen: (() => void) | null = null
  let pending: Promise<void> | null = null

  function startListening(): void {
    if (pending || !isTauri()) return
    // Import dinamico: o modulo de eventos so carrega dentro do app.
    pending = import('@tauri-apps/api/event')
      .then(({ listen }) => listen<T>(evento, (e) => aplicar(e.payload)))
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

  function subscribe(cb: () => void): () => void {
    listeners.add(cb)
    if (listeners.size === 1) {
      startListening()
      if (isTauri()) aoPrimeiro?.()
    }
    return () => {
      if (!listeners.delete(cb)) return
      if (listeners.size === 0 && stopListen) {
        stopListen()
        stopListen = null
      }
    }
  }

  function notificar(): void {
    for (const cb of listeners) cb()
  }

  return { subscribe, notificar }
}

/** Consulta o Rust uma única vez (a lógica da "primeira inscrição"). */
async function pedirAplicada(): Promise<void> {
  if (pedidoFeito) return
  pedidoFeito = true
  try {
    const r = await api.updateAppliedTake()
    if (r) setUpdateAplicada(r)
  } catch {
    // Sem resposta do Rust: segue sem o cartão.
  }
}

const canalAviso = criarCanal<UpdateAviso>('zap://update-aviso', (a) => setUpdateAviso(a))
const canalAplicada = criarCanal<UpdateAplicada | null>(
  'zap://update-aplicada',
  (a) => setUpdateAplicada(a),
  () => {
    void pedirAplicada()
  },
)

/** Hook: aviso de instalação em curso (`segundos > 0`) ou `null` (nenhum / `segundos == 0`). */
export function useUpdateAviso(): UpdateAviso | null {
  return useSyncExternalStore(canalAviso.subscribe, () => aviso)
}

/** Aplica o payload do evento (ou de um teste). `segundos == 0` limpa o aviso. */
export function setUpdateAviso(a: UpdateAviso): void {
  aviso = a.segundos > 0 ? a : null
  canalAviso.notificar()
}

/** Hook: "Atualizado para a vX" ainda não dispensado, ou `null`. Na primeira inscrição pede
 *  `api.updateAppliedTake()` uma vez (no Tauri) e também ouve o evento. */
export function useUpdateAplicada(): UpdateAplicada | null {
  return useSyncExternalStore(canalAplicada.subscribe, () => aplicada)
}

/** Aplica o payload (ou um teste). */
export function setUpdateAplicada(a: UpdateAplicada | null): void {
  aplicada = a
  canalAplicada.notificar()
}

/** Esconde o cartão "Atualizado para a vX" (botão Fechar). */
export function dispensarAplicada(): void {
  setUpdateAplicada(null)
}

/** Só para testes: zera os dois estados e a flag do pedido. */
export function resetUpdateAviso(): void {
  aviso = null
  aplicada = null
  pedidoFeito = false
  canalAviso.notificar()
  canalAplicada.notificar()
}

/** Só para testes: roda a lógica de "primeira inscrição" sem checar `isTauri`. */
export async function __pedirAplicadaParaTeste(): Promise<void> {
  await pedirAplicada()
}
