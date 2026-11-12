import { describe, expect, it, vi } from 'vitest'
import { lerProvedores } from './contaProvedores'

function resposta(ok: boolean, json: unknown): Response {
  return { ok, json: async () => json } as Response
}

describe('lerProvedores', () => {
  it('busca /auth/v1/settings com a chave e devolve os ligados', async () => {
    const buscar = vi.fn(async () => resposta(true, { external: { google: true, azure: false, email: true } }))
    expect(await lerProvedores('https://x.supabase.co/', 'chave-publica', buscar as typeof fetch)).toEqual(['google'])
    expect(buscar).toHaveBeenCalledWith('https://x.supabase.co/auth/v1/settings', { headers: { apikey: 'chave-publica' } })
  })

  it('HTTP fora de 2xx dá null', async () => {
    const buscar = vi.fn(async () => resposta(false, { external: { google: true } }))
    expect(await lerProvedores('https://x.supabase.co', 'k', buscar as typeof fetch)).toBeNull()
  })

  it('erro de rede ou JSON ruim dá null', async () => {
    const semRede = vi.fn(async () => {
      throw new TypeError('Failed to fetch')
    })
    expect(await lerProvedores('https://x.supabase.co', 'k', semRede as typeof fetch)).toBeNull()

    const jsonRuim = vi.fn(async () => ({ ok: true, json: async () => { throw new SyntaxError('x') } }) as unknown as Response)
    expect(await lerProvedores('https://x.supabase.co', 'k', jsonRuim as typeof fetch)).toBeNull()

    const semExternal = vi.fn(async () => resposta(true, { msg: 'Invalid API key' }))
    expect(await lerProvedores('https://x.supabase.co', 'k', semExternal as typeof fetch)).toBeNull()
  })
})
