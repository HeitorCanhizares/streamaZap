import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

// Dublês do plugin Tauri: invoke falso, Channel minimo, opener e listen falsos.
vi.mock('@tauri-apps/api/core', () => {
  class FakeChannel {
    onmessage: ((m: unknown) => void) | null = null
  }
  return { invoke: vi.fn(), Channel: FakeChannel }
})

vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }))

vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }))

import { invoke } from '@tauri-apps/api/core'
import { openUrl } from '@tauri-apps/plugin-opener'
import { listen } from '@tauri-apps/api/event'
import { api } from './api'

const invokeMock = vi.mocked(invoke)
const openUrlMock = vi.mocked(openUrl)
const listenMock = vi.mocked(listen)

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
  openUrlMock.mockReset()
  openUrlMock.mockResolvedValue(undefined as never)
  listenMock.mockReset()
})

afterEach(() => {
  fora()
})

describe('Conta fora do Tauri (mock)', () => {
  it('authStoreSet e authStoreGet guardam e devolvem o valor em memória', async () => {
    fora()
    await api.authStoreSet('k', 'v')
    await expect(api.authStoreGet('k')).resolves.toBe('v')
    expect(invokeMock).not.toHaveBeenCalled()
  })

  it('authStoreGet de chave ausente devolve null', async () => {
    fora()
    await expect(api.authStoreGet('naoexiste')).resolves.toBeNull()
  })

  it('authStoreRemove apaga a chave: o get seguinte devolve null', async () => {
    fora()
    await api.authStoreSet('k', 'v')
    await api.authStoreRemove('k')
    await expect(api.authStoreGet('k')).resolves.toBeNull()
    expect(invokeMock).not.toHaveBeenCalled()
  })

  it('onlineSetAccount resolve sem erro e não chama invoke', async () => {
    fora()
    await expect(api.onlineSetAccount('t')).resolves.toBeUndefined()
    expect(invokeMock).not.toHaveBeenCalled()
  })

  it('abrirUrl resolve sem erro e não chama invoke nem o opener', async () => {
    fora()
    await expect(api.abrirUrl('https://x')).resolves.toBeUndefined()
    expect(invokeMock).not.toHaveBeenCalled()
    expect(openUrlMock).not.toHaveBeenCalled()
  })

  it('escutarDeepLink devolve uma função que pode ser chamada', async () => {
    fora()
    const desfazer = await api.escutarDeepLink(() => undefined)
    expect(typeof desfazer).toBe('function')
    expect(() => desfazer()).not.toThrow()
    expect(listenMock).not.toHaveBeenCalled()
  })
})

describe('Conta dentro do Tauri', () => {
  beforeEach(() => {
    dentro()
  })

  it('authStoreGet chama invoke("auth_store_get", { key })', async () => {
    invokeMock.mockResolvedValueOnce('valor' as never)
    const r = await api.authStoreGet('k')
    expect(invokeMock).toHaveBeenCalledWith('auth_store_get', { key: 'k' })
    expect(r).toBe('valor')
  })

  it('authStoreSet chama invoke("auth_store_set", { key, value })', async () => {
    await api.authStoreSet('k', 'v')
    expect(invokeMock).toHaveBeenCalledWith('auth_store_set', { key: 'k', value: 'v' })
  })

  it('authStoreRemove chama invoke("auth_store_remove", { key })', async () => {
    await api.authStoreRemove('k')
    expect(invokeMock).toHaveBeenCalledWith('auth_store_remove', { key: 'k' })
  })

  it('onlineSetAccount("tok") chama invoke("online_set_account", { token: "tok" })', async () => {
    await api.onlineSetAccount('tok')
    expect(invokeMock).toHaveBeenCalledWith('online_set_account', { token: 'tok' })
  })

  it('onlineSetAccount(null) chama invoke("online_set_account", { token: null })', async () => {
    await api.onlineSetAccount(null)
    expect(invokeMock).toHaveBeenCalledWith('online_set_account', { token: null })
  })

  it('abrirUrl chama o opener com a URL', async () => {
    await api.abrirUrl('https://x')
    expect(openUrlMock).toHaveBeenCalledWith('https://x')
  })

  it('escutarDeepLink usa o evento zap://deep-link, repassa o payload e devolve o unlisten', async () => {
    const unlisten = vi.fn()
    listenMock.mockResolvedValueOnce(unlisten as never)
    const cb = vi.fn()

    const desfazer = await api.escutarDeepLink(cb)

    expect(listenMock).toHaveBeenCalledWith('zap://deep-link', expect.any(Function))
    const handler = listenMock.mock.calls[0][1] as (e: { payload: string }) => void
    handler({ payload: 'zap://deep-link/entrar?codigo=1' })
    expect(cb).toHaveBeenCalledWith('zap://deep-link/entrar?codigo=1')

    desfazer()
    expect(unlisten).toHaveBeenCalledTimes(1)
  })
})
