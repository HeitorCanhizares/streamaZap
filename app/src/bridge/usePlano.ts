// Hooks de planos para as telas (docs/planos/fase3-app-planos.md §4.4).
import { useMemo } from 'react'
import { useOnlineEnabled } from './onlineStore'
import { usePlanoEstado } from './planoStore'
import { useSnapshot } from './store'
import type { PresetLock, RoomMode, Snapshot } from './types'

export interface Travas { ativo: boolean; tela: PresetLock[]; camera: PresetLock[]; podeTransmitir: boolean; podeEntrar: boolean }

// Listas vazias compartilhadas: mesma referência entre chamadas evita re-render/efeito à toa.
const SEM_TRAVAS_LISTA: PresetLock[] = Object.freeze([]) as unknown as PresetLock[]
const NADA_TRAVADO: Travas = Object.freeze({
  ativo: false,
  tela: SEM_TRAVAS_LISTA,
  camera: SEM_TRAVAS_LISTA,
  podeTransmitir: true,
  podeEntrar: true,
})

/** Puro. Local, Online desligado ou snapshot sem `quality_locks` → nada travado. */
export function travasDo(snap: Snapshot, modo: RoomMode, onlineLigado: boolean): Travas {
  const locks = snap.quality_locks
  if (modo !== 'online' || !onlineLigado || !locks) return NADA_TRAVADO
  return {
    ativo: true,
    tela: locks.screen,
    camera: locks.camera,
    podeTransmitir: locks.can_publish,
    podeEntrar: snap.account?.plan?.pode_entrar ?? true,
  }
}

export function useTravas(modo: RoomMode): Travas {
  const snap = useSnapshot()
  const ligado = useOnlineEnabled()
  const locks = snap.quality_locks
  const podeEntrar = snap.account?.plan?.pode_entrar
  // Dependências finas: o objeto snapshot muda a cada atualização, as travas não.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  return useMemo(() => travasDo(snap, modo, ligado), [locks, podeEntrar, modo, ligado])
}

export function useCobrancaAtiva(): boolean {
  return usePlanoEstado().config.cobrancaAtiva
}
