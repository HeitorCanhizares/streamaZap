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

beforeEach(() => {
  invokeMock.mockReset()
  invokeMock.mockResolvedValue(undefined as never)
})

afterEach(() => {
  fora()
})

describe('M4 Online fora do Tauri (mock)', () => {
  it('serverTest com endereco devolve ok true e ping de exemplo', async () => {
    fora()
    const r = await api.serverTest('x', '')
    expect(r.ok).toBe(true)
    expect(r.ping_ms).toBe(42)
    expect(invokeMock).not.toHaveBeenCalled()
  })

  it('serverTest sem endereco devolve ok false com codigo invalido', async () => {
    fora()
    const r = await api.serverTest('', '')
    expect(r.ok).toBe(false)
    expect(r.code).toBe('invalido')
    expect(invokeMock).not.toHaveBeenCalled()
  })

  it('roomPreview e roomPreviewStop resolvem sem erro e sem invoke', async () => {
    fora()
    await expect(api.roomPreview('ZAP-ABCD', 1, () => {})).resolves.toBeUndefined()
    await expect(api.roomPreviewStop(1)).resolves.toBeUndefined()
    expect(invokeMock).not.toHaveBeenCalled()
  })
})
