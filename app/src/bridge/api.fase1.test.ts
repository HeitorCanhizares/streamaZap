import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

// Dublê do plugin Tauri: invoke falso e Channel minimo com onmessage.
vi.mock('@tauri-apps/api/core', () => {
  class FakeChannel {
    onmessage: ((m: unknown) => void) | null = null
  }
  return { invoke: vi.fn(), Channel: FakeChannel }
})

import { invoke } from '@tauri-apps/api/core'
import { api } from './api'

const invokeMock = vi.mocked(invoke)

// Ambiente de navegador: sem internals do Tauri.
const fora = () => {
  delete (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
}
const dentro = () => {
  ;(globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {}
}

beforeEach(() => {
  invokeMock.mockReset()
  invokeMock.mockResolvedValue(undefined as never)
})

afterEach(() => {
  fora()
})

describe('Comandos da v3.3.0 fora do Tauri (mock)', () => {
  it('cameraQualitySet resolve sem erro e não chama invoke', async () => {
    fora()
    await expect(api.cameraQualitySet('1080p30')).resolves.toBeUndefined()
    expect(invokeMock).not.toHaveBeenCalled()
  })

  it('cameraLayoutSet resolve sem erro e não chama invoke', async () => {
    fora()
    await expect(api.cameraLayoutSet({ modo: 'solta' })).resolves.toBeUndefined()
    expect(invokeMock).not.toHaveBeenCalled()
  })

  it('updatePostpone resolve sem erro e não chama invoke', async () => {
    fora()
    await expect(api.updatePostpone()).resolves.toBeUndefined()
    expect(invokeMock).not.toHaveBeenCalled()
  })

  it('updateAppliedTake fora do Tauri resolve null sem chamar invoke', async () => {
    fora()
    await expect(api.updateAppliedTake()).resolves.toBeNull()
    expect(invokeMock).not.toHaveBeenCalled()
  })
})

describe('Comandos da v3.3.0 dentro do Tauri', () => {
  it('cameraQualitySet chama invoke("camera_quality_set", { preset: "1080p30" })', async () => {
    dentro()
    await api.cameraQualitySet('1080p30')
    expect(invokeMock).toHaveBeenCalledWith('camera_quality_set', { preset: '1080p30' })
  })

  it('cameraLayoutSet chama invoke("camera_layout_set", { layout: { modo: "canto", canto: "sup_esq" } })', async () => {
    dentro()
    await api.cameraLayoutSet({ modo: 'canto', canto: 'sup_esq' })
    expect(invokeMock).toHaveBeenCalledWith('camera_layout_set', { layout: { modo: 'canto', canto: 'sup_esq' } })
  })

  it('updatePostpone chama invoke("update_postpone")', async () => {
    dentro()
    await api.updatePostpone()
    expect(invokeMock).toHaveBeenCalledWith('update_postpone')
  })

  it('updateAppliedTake chama invoke("update_applied_take") e devolve o que o invoke devolver', async () => {
    dentro()
    const aplicada = { versao: '3.1.0', anterior: '3.0.0', notas: 'Câmera nova' }
    invokeMock.mockResolvedValueOnce(aplicada as never)
    const r = await api.updateAppliedTake()
    expect(invokeMock).toHaveBeenCalledWith('update_applied_take')
    expect(r).toEqual(aplicada)
  })
})
