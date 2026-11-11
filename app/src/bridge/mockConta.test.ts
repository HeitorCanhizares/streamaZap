import { afterEach, describe, expect, it, vi } from 'vitest'
import { criarMockConta, PRECOS_MOCK } from './mockConta'
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

const ASSIN = { plano: 'fullhd', status: 'ativa', ciclo: 'mensal', valido_ate: '2030-01-01T00:00:00.000Z' }
async function logar(c: ContaClient) {
  await c.auth.verifyOtp({ email: 'a@b.c', token: '123456', type: 'email' })
}
async function erroDe(r: { error: { context?: unknown } | null }) {
  return (r.error?.context as { json(): Promise<unknown> }).json()
}
const assinar = (c: ContaClient, body?: unknown) => c.functions.invoke('assinar', { body })
const lerAssinatura = async (c: ContaClient) => (await c.from('assinaturas').select('*').maybeSingle()).data

describe('criarMockConta: config, assinaturas e assinar', () => {
  afterEach(() => vi.useRealTimers())

  it('config_publica', async () => {
    const r = await criarMockConta({ cobrancaAtiva: true }).from('config_publica').select('x').maybeSingle()
    expect(r.data).toEqual({ cobranca_ativa: true, plano_padrao: 'fullhd', precos: PRECOS_MOCK })
    const d = await criarMockConta().from('config_publica').select('x').maybeSingle()
    expect((d.data as { cobranca_ativa: boolean }).cobranca_ativa).toBe(false)
    const f = await criarMockConta({ falharConfig: true }).from('config_publica').select('x').maybeSingle()
    expect(f.error).not.toBeNull()
    expect(f.data).toBeNull()
  })

  it('assinaturas só com sessão', async () => {
    const c = criarMockConta({ assinatura: ASSIN })
    expect(await lerAssinatura(c)).toBeNull()
    await logar(c)
    expect(await lerAssinatura(c)).toEqual(ASSIN)
  })

  it('assinar: erros na ordem', async () => {
    const c = criarMockConta({ cobrancaAtiva: true })
    expect(await erroDe(await assinar(c, { plano: 'hd', ciclo: 'mensal' }))).toMatchObject({
      erro: 'nao_autenticado',
      mensagem: 'Entre na sua conta.',
    })
    await logar(c)
    const off = criarMockConta({ cobrancaAtiva: false })
    await logar(off)
    expect(await erroDe(await assinar(off, { plano: 'hd', ciclo: 'mensal' }))).toMatchObject({
      erro: 'cobranca_desligada',
      mensagem: 'As assinaturas ainda não estão abertas.',
    })
    expect(await erroDe(await assinar(c, { plano: 'ouro', ciclo: 'mensal' }))).toMatchObject({
      erro: 'invalido',
      mensagem: 'Pedido inválido.',
    })
    expect(await erroDe(await assinar(c, { plano: 'hd', ciclo: 'mensal', cpf_cnpj: '123' }))).toMatchObject({
      erro: 'invalido',
      mensagem: 'Informe um CPF ou CNPJ válido.',
    })
  })

  it('assinar: sucesso anual, mensal, repetição e CPF', async () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-01-01T00:00:00.000Z'))
    const c = criarMockConta({ cobrancaAtiva: true })
    await logar(c)
    const r = await assinar(c, { plano: 'hd', ciclo: 'anual' })
    expect((r.data as { link: string }).link).toBe('https://sandbox.asaas.com/i/mock-hd-anual')
    const a = (await lerAssinatura(c)) as typeof ASSIN
    expect([a.plano, a.status, a.ciclo]).toEqual(['hd', 'ativa', 'anual'])
    expect(new Date(a.valido_ate).getTime() - Date.now()).toBe(365 * 86_400_000)
    expect(await erroDe(await assinar(c, { plano: 'hd', ciclo: 'mensal' }))).toMatchObject({
      erro: 'ja_assinante',
      mensagem: 'Você já tem esse plano.',
    })
    const ok = await assinar(c, { plano: 'fullhd', ciclo: 'mensal', cpf_cnpj: '12345678901' })
    expect(ok.error).toBeNull()
    const m = (await lerAssinatura(c)) as typeof ASSIN
    expect(new Date(m.valido_ate).getTime() - Date.now()).toBe(30 * 86_400_000)
  })

  it('invoke de outro nome devolve ok', async () => {
    expect(await criarMockConta().functions.invoke('outra')).toEqual({ error: null })
  })
})

describe('criarMockConta: refreshSession', () => {
  it('emite TOKEN_REFRESHED com token novo', async () => {
    const c = criarMockConta()
    await logar(c)
    const { eventos } = espiar(c)
    await tick()
    const r = await c.auth.refreshSession()
    expect(r.data?.session?.access_token).toBe('mock-token-2')
    await c.auth.refreshSession()
    expect(eventos.slice(1).map(([e, s]) => [e, s?.access_token])).toEqual([
      ['TOKEN_REFRESHED', 'mock-token-2'],
      ['TOKEN_REFRESHED', 'mock-token-3'],
    ])
  })

  it('sem sessão não emite', async () => {
    const c = criarMockConta()
    const { eventos } = espiar(c)
    await tick()
    const r = await c.auth.refreshSession()
    expect(r).toEqual({ data: { session: null }, error: null })
    expect(eventos).toHaveLength(1)
  })
})
