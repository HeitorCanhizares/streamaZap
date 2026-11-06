import { describe, expect, it } from 'vitest'
import { criarMockConta } from './mockConta'
import type { ContaClient, ContaSessao } from './contaTipos'

const OPCOES = { redirectTo: 'x', skipBrowserRedirect: true }
const tick = () => new Promise<void>((r) => queueMicrotask(r))

function espiar(c: ContaClient) {
  const eventos: Array<[string, ContaSessao | null]> = []
  const { data } = c.auth.onAuthStateChange((e, s) => eventos.push([e, s]))
  return { eventos, sub: data.subscription }
}

describe('criarMockConta', () => {
  it('entrega INITIAL_SESSION com null no início', async () => {
    const c = criarMockConta()
    const { eventos } = espiar(c)
    expect(eventos).toEqual([])
    await tick()
    expect(eventos).toEqual([['INITIAL_SESSION', null]])
  })

  it('signInWithOAuth emite SIGNED_IN', async () => {
    const c = criarMockConta()
    const { eventos } = espiar(c)
    await tick()
    const r = await c.auth.signInWithOAuth({ provider: 'google', options: OPCOES })
    expect(r).toEqual({ data: { url: 'https://mock.supabase.co/auth/v1/authorize' }, error: null })
    const [evento, sessao] = eventos[1]
    expect(evento).toBe('SIGNED_IN')
    expect(sessao?.access_token).toBe('mock-token')
    expect(sessao?.user?.email).toBe('voce@exemplo.com')
  })

  it('verifyOtp aceita 123456 e rejeita outro', async () => {
    const c = criarMockConta()
    const { eventos } = espiar(c)
    await tick()
    const ruim = await c.auth.verifyOtp({ email: 'a@b.c', token: '000000', type: 'email' })
    expect(ruim.error?.code).toBe('otp_expired')
    expect(eventos).toHaveLength(1)
    const ok = await c.auth.verifyOtp({ email: 'a@b.c', token: '123456', type: 'email' })
    expect(ok.error).toBeNull()
    expect(eventos[1][0]).toBe('SIGNED_IN')
    expect(eventos[1][1]?.user?.email).toBe('a@b.c')
  })

  it('getUserIdentities, linkIdentity e unlinkIdentity', async () => {
    const c = criarMockConta()
    await c.auth.signInWithOAuth({ provider: 'google', options: OPCOES })
    let ids = (await c.auth.getUserIdentities()).data?.identities ?? []
    expect(ids.map((i) => i.provider)).toEqual(['google'])
    await c.auth.linkIdentity({ provider: 'azure', options: OPCOES })
    await c.auth.linkIdentity({ provider: 'azure', options: OPCOES })
    ids = (await c.auth.getUserIdentities()).data?.identities ?? []
    expect(ids.map((i) => i.provider)).toEqual(['google', 'azure'])
    await c.auth.unlinkIdentity(ids[0])
    ids = (await c.auth.getUserIdentities()).data?.identities ?? []
    expect(ids.map((i) => i.provider)).toEqual(['azure'])
  })

  it('signOut emite SIGNED_OUT e zera identidades', async () => {
    const c = criarMockConta()
    const { eventos } = espiar(c)
    await c.auth.signInWithOAuth({ provider: 'google', options: OPCOES })
    await c.auth.signOut()
    expect(eventos[eventos.length - 1]).toEqual(['SIGNED_OUT', null])
    expect((await c.auth.getUserIdentities()).data?.identities).toEqual([])
  })

  it('novo assinante após login recebe INITIAL_SESSION com a sessão', async () => {
    const c = criarMockConta()
    await c.auth.signInWithOAuth({ provider: 'google', options: OPCOES })
    const { eventos } = espiar(c)
    await tick()
    expect(eventos[0][0]).toBe('INITIAL_SESSION')
    expect(eventos[0][1]?.access_token).toBe('mock-token')
  })

  it('unsubscribe para as entregas', async () => {
    const c = criarMockConta()
    const a = espiar(c)
    a.sub.unsubscribe()
    await tick()
    await c.auth.signOut()
    expect(a.eventos).toEqual([])
  })

  it('clientes não compartilham estado', async () => {
    const a = criarMockConta()
    const b = criarMockConta()
    await a.auth.signInWithOAuth({ provider: 'google', options: OPCOES })
    expect((await b.auth.getUserIdentities()).data?.identities).toEqual([])
  })
})
