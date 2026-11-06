// "Modo Online ligado?" (`settings.server.enabled`) para a interface inteira (plano §4, frente A).
// Mesmo padrão de updateStore.ts: uma referência só, trocada por setOnlineEnabled, e useSyncExternalStore.
// Valor inicial: fora do Tauri (mock/testes) `true` (o design mostra o Online); dentro do app `false`
// até `carregarOnlineEnabled()` ler `settings_get`. Tarefa front-01.
import { useSyncExternalStore } from 'react'
import { api, isTauri } from './api'

// Fora do Tauri o Online vem ligado (mock); dentro do app, desligado até a leitura.
const valorInicial = (): boolean => !isTauri()

let current: boolean = valorInicial()
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

/** Hook: o modo Online está ligado? Re-renderiza quando muda. */
export function useOnlineEnabled(): boolean {
  return useSyncExternalStore(subscribe, getOnlineEnabled)
}

/** Valor atual (sem hook). */
export function getOnlineEnabled(): boolean {
  return current
}

/** Troca o valor e avisa os inscritos (chamado por ConfigScreen depois de `settingsSet`). */
export function setOnlineEnabled(v: boolean): void {
  // Sem mudança não avisa: evita re-render à toa dos componentes inscritos.
  if (current === v) return
  current = v
  notificar()
}

/** Lê `api.settingsGet()` e aplica `server?.enabled ?? false`. Erro de leitura: mantém o valor. */
export async function carregarOnlineEnabled(): Promise<void> {
  try {
    const s = await api.settingsGet()
    setOnlineEnabled(s.server?.enabled ?? false)
  } catch {
    // Leitura falhou (ex.: backend ainda não subiu): mantém o valor atual.
  }
}

/** Só para testes: volta ao valor inicial. */
export function resetOnlineEnabled(): void {
  current = valorInicial()
  notificar()
}
