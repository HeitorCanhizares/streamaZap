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

// Os testes de mock compartilham o estado do modulo: a leitura default vem primeiro.
describe('api fora do Tauri (mock)', () => {
  it('settingsGet devolve a porta default 47800', async () => {
    fora()
    const s = await api.settingsGet()
    expect(s.local.port).toBe(47800)
    expect(invokeMock).not.toHaveBeenCalled()
  })

  it('settingsSet faz merge e uma segunda settingsGet reflete a mudanca', async () => {
    fora()
    const devolvido = await api.settingsSet({ name: 'X' })
    expect(devolvido.name).toBe('X')
    const depois = await api.settingsGet()
    expect(depois.name).toBe('X')
    // o merge e por secao: as outras secoes continuam intactas
    expect(depois.local.port).toBe(47800)
  })
})

describe('api dentro do Tauri', () => {
  it('roomLeave chama invoke("room_leave")', async () => {
    dentro()
    await api.roomLeave()
    expect(invokeMock).toHaveBeenCalledWith('room_leave')
  })

  it('setVolume chama invoke("set_volume") com os argumentos em camelCase', async () => {
    dentro()
    await api.setVolume('a', 0.5, false)
    expect(invokeMock).toHaveBeenCalledWith('set_volume', {
      participantId: 'a',
      volume: 0.5,
      muted: false,
    })
  })

  it('erro string do invoke vira Error com a mesma mensagem', async () => {
    dentro()
    invokeMock.mockRejectedValueOnce('sala cheia')
    const erro = await api.roomLeave().catch((e: unknown) => e)
    expect(erro).toBeInstanceOf(Error)
    expect((erro as Error).message).toBe('sala cheia')
  })
})
