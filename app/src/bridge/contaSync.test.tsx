import { act, cleanup, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('./contaClient', () => ({
  obterCliente: vi.fn(),
  descartarCliente: vi.fn(async () => undefined),
}))
vi.mock('./contaAcoes', () => ({ tratarDeepLink: vi.fn(async () => undefined) }))
vi.mock('./contaVinculo', () => ({ carregarIdentidades: vi.fn(async () => undefined) }))
vi.mock('./contaProvedores', () => ({ lerProvedores: vi.fn(async () => ['google']) }))

import { api } from './api'
import { obterCliente, descartarCliente } from './contaClient'
import { tratarDeepLink } from './contaAcoes'
import { carregarIdentidades } from './contaVinculo'
import { lerProvedores } from './contaProvedores'
import { getContaEstado, resetConta } from './contaStore'
import { iniciarConta, useContaSync } from './contaSync'
import { emptySnapshot, resetSnapshotForTests, setSnapshot } from './store'
import { resetOnlineEnabled, setOnlineEnabled } from './onlineStore'
import type { AccountProviderView, Snapshot } from './types'

const conta: AccountProviderView = { provider: 'supabase', url: 'https://x.supabase.co', key: 'k' }
type Cb = (e: string, s: { access_token: string; user?: { email?: string } } | null) => void

let cb: Cb | null
let unsubscribe: ReturnType<typeof vi.fn>
let unlisten: ReturnType<typeof vi.fn>
let linkCb: ((u: string) => void) | null
let setAccount: ReturnType<typeof vi.spyOn>

function fake() {
  return {
    auth: {
      onAuthStateChange: (c: Cb) => {
        cb = c
        return { data: { subscription: { unsubscribe } } }
      },
    },
  }
}

const flush = () =>
  act(async () => {
    await new Promise((r) => setTimeout(r, 5))
  })

beforeEach(() => {
  cb = null
  linkCb = null
  unsubscribe = vi.fn()
  unlisten = vi.fn()
  vi.mocked(obterCliente).mockReset()
  vi.mocked(obterCliente).mockResolvedValue(fake() as never)
  vi.mocked(descartarCliente).mockClear()
  vi.mocked(tratarDeepLink).mockClear()
  vi.mocked(carregarIdentidades).mockClear()
  setAccount = vi.spyOn(api, 'onlineSetAccount').mockResolvedValue(undefined)
  vi.spyOn(api, 'escutarDeepLink').mockImplementation(async (f) => {
    linkCb = f
    return unlisten as never
  })
  resetConta()
  resetSnapshotForTests()
  resetOnlineEnabled()
})

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
  resetConta()
})

describe('iniciarConta', () => {
  it('sem conta, outro provider ou desligado: não faz nada', async () => {
    iniciarConta(undefined, true)()
    iniciarConta({ ...conta, provider: 'outro' }, true)()
    iniciarConta(conta, false)()
    await flush()
    expect(obterCliente).not.toHaveBeenCalled()
    expect(getContaEstado().sessao).toBe('carregando')
  })

  it('INITIAL_SESSION com sessão', async () => {
    iniciarConta(conta, true)
    await flush()
    cb?.('INITIAL_SESSION', { access_token: 'tok', user: { email: 'a@b.c' } })
    expect(getContaEstado()).toMatchObject({ sessao: 'dentro', email: 'a@b.c' })
    expect(setAccount).toHaveBeenCalledWith('tok')
    await flush()
    expect(carregarIdentidades).toHaveBeenCalledTimes(1)
  })

  it('TOKEN_REFRESHED repassa o token novo sem recarregar identidades', async () => {
    iniciarConta(conta, true)
    await flush()
    cb?.('TOKEN_REFRESHED', { access_token: 'novo' })
    await flush()
    expect(setAccount).toHaveBeenCalledWith('novo')
    expect(carregarIdentidades).not.toHaveBeenCalled()
  })

  it('SIGNED_OUT e INITIAL_SESSION nula saem da conta', async () => {
    iniciarConta(conta, true)
    await flush()
    cb?.('SIGNED_IN', { access_token: 't', user: { email: 'a@b.c' } })
    cb?.('SIGNED_OUT', null)
    expect(getContaEstado()).toMatchObject({ sessao: 'fora', email: null, identidades: [] })
    expect(setAccount).toHaveBeenLastCalledWith(null)
  })

  it('INITIAL_SESSION nula parte de carregando e vai para fora', async () => {
    iniciarConta(conta, true)
    await flush()
    expect(getContaEstado().sessao).toBe('carregando')
    cb?.('INITIAL_SESSION', null)
    expect(getContaEstado()).toMatchObject({ sessao: 'fora', email: null })
    expect(setAccount).toHaveBeenCalledWith(null)
  })

  it('rejeição de onlineSetAccount é engolida', async () => {
    setAccount.mockRejectedValue(new Error('x'))
    iniciarConta(conta, true)
    await flush()
    cb?.('SIGNED_IN', { access_token: 't' })
    await flush()
    expect(getContaEstado().sessao).toBe('dentro')
  })

  describe('provedores ligados', () => {
    const g = globalThis as Record<string, unknown>
    beforeEach(() => {
      vi.mocked(lerProvedores).mockClear()
      vi.spyOn(api, 'deepLinkPendentes').mockResolvedValue([])
    })
    afterEach(() => {
      delete g.__TAURI_INTERNALS__
    })

    it('fora do Tauri não busca e ficam todos', async () => {
      iniciarConta(conta, true)
      await flush()
      expect(lerProvedores).not.toHaveBeenCalled()
      expect(getContaEstado().provedores).toEqual(['google', 'azure'])
    })

    it('no Tauri lê do Supabase do servidor', async () => {
      g.__TAURI_INTERNALS__ = {}
      iniciarConta(conta, true)
      await flush()
      expect(lerProvedores).toHaveBeenCalledWith('https://x.supabase.co', 'k')
      expect(getContaEstado().provedores).toEqual(['google'])
    })

    it('falha na leitura deixa todos', async () => {
      g.__TAURI_INTERNALS__ = {}
      vi.mocked(lerProvedores).mockResolvedValueOnce(null)
      iniciarConta(conta, true)
      await flush()
      expect(getContaEstado().provedores).toEqual(['google', 'azure'])
    })

    it('resposta depois do cleanup é ignorada', async () => {
      g.__TAURI_INTERNALS__ = {}
      iniciarConta(conta, true)()
      await flush()
      expect(getContaEstado().provedores).toEqual(['google', 'azure'])
    })
  })

  it('falha ao criar o cliente põe aviso', async () => {
    vi.mocked(obterCliente).mockRejectedValue(new Error('boom'))
    iniciarConta(conta, true)
    await flush()
    expect(getContaEstado()).toMatchObject({ sessao: 'fora', aviso: 'Não foi possível iniciar o login.' })
  })

  it('deep link chama tratarDeepLink', async () => {
    iniciarConta(conta, true)
    await flush()
    linkCb?.('streamazap://auth/callback?code=1')
    expect(tratarDeepLink).toHaveBeenCalledWith('streamazap://auth/callback?code=1')
  })

  it('callback que chegou com a interface fechada é tratado ao ligar o login', async () => {
    vi.spyOn(api, 'deepLinkPendentes').mockResolvedValue(['streamazap://auth/callback?code=frio'])
    iniciarConta(conta, true)
    await flush()
    expect(api.deepLinkPendentes).toHaveBeenCalledTimes(1)
    expect(tratarDeepLink).toHaveBeenCalledWith('streamazap://auth/callback?code=frio')
  })

  it('falha ao buscar os pendentes é engolida', async () => {
    vi.spyOn(api, 'deepLinkPendentes').mockRejectedValue(new Error('x'))
    iniciarConta(conta, true)
    await flush()
    expect(tratarDeepLink).not.toHaveBeenCalled()
    expect(getContaEstado().aviso).toBeNull()
  })

  it('cleanup desfaz tudo e volta a carregando', async () => {
    const stop = iniciarConta(conta, true)
    await flush()
    cb?.('SIGNED_IN', { access_token: 't' })
    stop()
    expect(unsubscribe).toHaveBeenCalled()
    expect(unlisten).toHaveBeenCalled()
    expect(descartarCliente).toHaveBeenCalled()
    expect(getContaEstado().sessao).toBe('carregando')
  })

  it('cleanup antes de obterCliente resolver não deixa assinatura', async () => {
    let resolve: (v: never) => void = () => undefined
    vi.mocked(obterCliente).mockReturnValue(
      new Promise((r) => {
        resolve = r as never
      }),
    )
    const stop = iniciarConta(conta, true)
    stop()
    resolve(fake() as never)
    await flush()
    expect(cb).toBeNull()
    expect(api.escutarDeepLink).not.toHaveBeenCalled()
  })
})

describe('useContaSync', () => {
  const comConta = (): Snapshot => ({
    ...emptySnapshot(),
    account: { account_provider: { ...conta }, requires_account: true, requires_password: false },
  })

  it('sem account não chama obterCliente', async () => {
    setOnlineEnabled(true)
    // O mock de reset já traz account; aqui o snapshot é explicitamente sem conta.
    act(() => setSnapshot({ ...emptySnapshot(), version: 1 }))
    renderHook(() => useContaSync())
    await flush()
    expect(obterCliente).not.toHaveBeenCalled()
  })

  it('com account e Online ligado chama uma vez, mesmo com snapshot novo', async () => {
    setOnlineEnabled(true)
    act(() => setSnapshot({ ...comConta(), version: 1 }))
    renderHook(() => useContaSync())
    await flush()
    expect(obterCliente).toHaveBeenCalledTimes(1)
    act(() => setSnapshot({ ...comConta(), version: 2 }))
    await flush()
    expect(obterCliente).toHaveBeenCalledTimes(1)
  })
})
