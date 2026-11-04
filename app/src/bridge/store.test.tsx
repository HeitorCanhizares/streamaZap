import { act, renderHook } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { getSnapshot, resetSnapshotForTests, setSnapshot, subscribe, useSnapshot } from './store'
import { mockSnapshot } from './mock'
import type { Snapshot } from './types'

// Copia do snapshot do mock com outra versao (e so o campo version muda).
const comVersao = (version: number): Snapshot => ({ ...mockSnapshot(), version })

beforeEach(() => {
  resetSnapshotForTests()
})

describe('store de snapshot', () => {
  it('getSnapshot devolve o mock inicial', () => {
    expect(getSnapshot().rooms.map((r) => r.name)).toEqual(['Sexta do Rush', 'Filminho de domingo', 'Live de Python'])
  })

  it('getSnapshot devolve a mesma referência enquanto nada muda', () => {
    expect(getSnapshot()).toBe(getSnapshot())
  })

  it('setSnapshot com version maior notifica o listener e atualiza', () => {
    const cb = vi.fn()
    const off = subscribe(cb)
    const novo = comVersao(getSnapshot().version + 1)
    setSnapshot(novo)
    expect(cb).toHaveBeenCalledTimes(1)
    expect(getSnapshot()).toBe(novo)
    off()
  })

  it('setSnapshot com version menor é ignorado', () => {
    const cb = vi.fn()
    const off = subscribe(cb)
    const antes = getSnapshot()
    setSnapshot(comVersao(antes.version - 1))
    expect(cb).not.toHaveBeenCalled()
    expect(getSnapshot()).toBe(antes)
    off()
  })

  it('unsubscribe para de notificar', () => {
    const cb = vi.fn()
    const off = subscribe(cb)
    off()
    setSnapshot(comVersao(getSnapshot().version + 1))
    expect(cb).not.toHaveBeenCalled()
  })

  it('useSnapshot re-renderiza após setSnapshot', () => {
    const { result } = renderHook(() => useSnapshot())
    const novo = comVersao(result.current.version + 1)
    act(() => {
      setSnapshot(novo)
    })
    expect(result.current).toBe(novo)
  })
})
