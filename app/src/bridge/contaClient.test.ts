import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { authStorage } from './authStorage'

const fake = { auth: { stopAutoRefresh: vi.fn(async () => {}) } }
const createClient = vi.fn((..._a: unknown[]) => fake)
vi.mock('@supabase/supabase-js', () => ({ createClient: (...a: unknown[]) => createClient(...a) }))

import { clienteAtual, descartarCliente, obterCliente } from './contaClient'

const conta = { provider: 'supabase', url: 'https://a.supabase.co', key: 'k1' }
const g = globalThis as Record<string, unknown>

beforeEach(() => {
  g.__TAURI_INTERNALS__ = {}
  createClient.mockClear()
  fake.auth.stopAutoRefresh.mockReset()
  fake.auth.stopAutoRefresh.mockResolvedValue(undefined)
})
afterEach(async () => {
  await descartarCliente()
  delete g.__TAURI_INTERNALS__
})

describe('contaClient', () => {
  it('cria com as opções do plano', async () => {
    const c = await obterCliente(conta)
    expect(c).toBe(fake)
    expect(createClient).toHaveBeenCalledWith(conta.url, conta.key, {
      auth: {
        flowType: 'pkce',
        detectSessionInUrl: false,
        persistSession: true,
        autoRefreshToken: true,
        storage: authStorage,
      },
    })
  })

  it('mesma conta, inclusive concorrente, cria um só cliente', async () => {
    const [a, b] = await Promise.all([obterCliente(conta), obterCliente({ ...conta })])
    const c = await obterCliente(conta)
    expect(a).toBe(b)
    expect(c).toBe(a)
    expect(createClient).toHaveBeenCalledTimes(1)
  })

  it('url diferente cria outro e para o antigo', async () => {
    await obterCliente(conta)
    await obterCliente({ ...conta, url: 'https://b.supabase.co' })
    expect(createClient).toHaveBeenCalledTimes(2)
    expect(fake.auth.stopAutoRefresh).toHaveBeenCalledTimes(1)
  })

  it('provider desconhecido rejeita', async () => {
    await expect(obterCliente({ ...conta, provider: 'outro' })).rejects.toThrow('Provedor de login não suportado.')
  })

  it('fora do Tauri usa o mock', async () => {
    delete g.__TAURI_INTERNALS__
    const c = await obterCliente(conta)
    expect(c).not.toBe(fake)
    expect(typeof c.auth.signOut).toBe('function')
    expect(createClient).not.toHaveBeenCalled()
  })

  it('clienteAtual e descartarCliente', async () => {
    expect(clienteAtual()).toBeNull()
    const c = await obterCliente(conta)
    expect(clienteAtual()).toBe(c)
    await descartarCliente()
    expect(clienteAtual()).toBeNull()
  })

  it('descartar sem cliente não lança e engole erro de stopAutoRefresh', async () => {
    await expect(descartarCliente()).resolves.toBeUndefined()
    await obterCliente(conta)
    fake.auth.stopAutoRefresh.mockRejectedValue(new Error('x'))
    await expect(descartarCliente()).resolves.toBeUndefined()
    expect(clienteAtual()).toBeNull()
  })
})
