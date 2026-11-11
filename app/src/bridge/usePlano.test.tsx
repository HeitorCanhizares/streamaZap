import { act, cleanup, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { mockSnapshot } from './mock'
import { PRECOS_MOCK } from './mockConta'
import { resetOnlineEnabled, setOnlineEnabled } from './onlineStore'
import { resetPlano, setPlanoEstado } from './planoStore'
import { resetSnapshotForTests, setSnapshot, getSnapshot } from './store'
import type { QualityLocks, Snapshot } from './types'
import { travasDo, useCobrancaAtiva, useTravas } from './usePlano'

const HD: QualityLocks = {
  can_publish: true,
  screen: [{ preset: '1080p30', needs: 'fullhd' }, { preset: '1080p60', needs: 'fullhd' }],
  camera: [{ preset: '1080p30', needs: 'fullhd' }, { preset: '1080p60', needs: 'fullhd' }],
}
const snapCom = (quality_locks?: QualityLocks): Snapshot => ({ ...mockSnapshot(), quality_locks })

beforeEach(() => {
  resetSnapshotForTests()
  resetOnlineEnabled()
  resetPlano()
})
afterEach(() => {
  cleanup()
  resetSnapshotForTests()
  resetOnlineEnabled()
  resetPlano()
})

describe('travasDo', () => {
  it('modo local não trava', () => {
    expect(travasDo(snapCom(HD), 'local', true)).toEqual({ ativo: false, tela: [], camera: [], podeTransmitir: true, podeEntrar: true })
  })
  it('online ligado com travas do hd', () => {
    const t = travasDo(snapCom(HD), 'online', true)
    expect(t.ativo).toBe(true)
    expect(t.tela).toEqual(HD.screen)
    expect(t.camera).toEqual(HD.camera)
    expect(t.tela).toBe(HD.screen)
  })
  it('online desligado não trava', () => {
    expect(travasDo(snapCom(HD), 'online', false).ativo).toBe(false)
  })
  it('sem quality_locks não trava', () => {
    expect(travasDo(snapCom(undefined), 'online', true).ativo).toBe(false)
  })
  it('can_publish e plano refletem em podeTransmitir e podeEntrar', () => {
    expect(travasDo(snapCom({ ...HD, can_publish: false }), 'online', true).podeTransmitir).toBe(false)
    const base = snapCom(HD)
    const semPlano = { ...base, account: { ...base.account!, plan: undefined } } as Snapshot
    expect(travasDo(semPlano, 'online', true).podeEntrar).toBe(true)
    const nenhum = { ...base, account: { ...base.account!, plan: { id: 'nenhum', pode_entrar: false } } } as Snapshot
    expect(travasDo(nenhum, 'online', true).podeEntrar).toBe(false)
  })
  it('sem travas devolve listas de mesma referência', () => {
    const a = travasDo(snapCom(undefined), 'local', true)
    const b = travasDo(snapCom(undefined), 'online', false)
    expect(a.tela).toBe(b.tela)
    expect(a.camera).toBe(b.camera)
  })
})

describe('hooks', () => {
  it('useTravas reflete o snapshot', () => {
    setOnlineEnabled(true)
    const { result } = renderHook(() => useTravas('online'))
    expect(result.current.ativo).toBe(false)
    act(() => {
      setSnapshot({ ...getSnapshot(), version: getSnapshot().version + 1, quality_locks: HD })
    })
    expect(result.current.ativo).toBe(true)
    expect(result.current.tela).toBe(HD.screen)
  })
  it('useCobrancaAtiva muda com o estado de planos', () => {
    const { result } = renderHook(() => useCobrancaAtiva())
    expect(result.current).toBe(false)
    act(() => {
      setPlanoEstado({ config: { cobrancaAtiva: true, planoPadrao: null, precos: PRECOS_MOCK } })
    })
    expect(result.current).toBe(true)
    act(() => {
      resetPlano()
    })
    expect(result.current).toBe(false)
  })
})
