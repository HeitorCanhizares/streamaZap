import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from './api'
import { REDIRECT, sair } from './contaAcoes'
import { clienteAtual } from './contaClient'
import { carregarIdentidades, desvincular, excluirConta, vincular } from './contaVinculo'
import { getContaEstado, resetConta, setContaEstado } from './contaStore'
import type { ContaClient, ContaErro, ContaIdentidade } from './contaTipos'

vi.mock('./contaClient', () => ({ clienteAtual: vi.fn() }))
vi.mock('./contaAcoes', async (orig) => ({
  ...(await orig<typeof import('./contaAcoes')>()),
  sair: vi.fn(async () => {}),
}))

const g: ContaIdentidade = { identity_id: '1', provider: 'google', email: 'a@b.c' }
const m: ContaIdentidade = { identity_id: '2', provider: 'azure' }

function fake() {
  const auth = {
    getUserIdentities: vi.fn(async () => ({
      data: { identities: [g, m] } as { identities: ContaIdentidade[] } | null,
      error: null as ContaErro | null,
    })),
    linkIdentity: vi.fn(async (_a: unknown) => ({
      data: { url: 'https://login.exemplo.com/x' as string | null },
      error: null as ContaErro | null,
    })),
    unlinkIdentity: vi.fn(async (_i: ContaIdentidade) => ({ error: null as ContaErro | null })),
  }
  const functions = { invoke: vi.fn(async (_n: string) => ({ error: null as ContaErro | null })) }
  return { auth, functions, cliente: { auth, functions } as unknown as ContaClient }
}

let f: ReturnType<typeof fake>
let abrir: ReturnType<typeof vi.spyOn>

beforeEach(() => {
  resetConta()
  vi.mocked(sair).mockClear()
  f = fake()
  vi.mocked(clienteAtual).mockReturnValue(f.cliente)
  abrir = vi.spyOn(api, 'abrirUrl').mockResolvedValue(undefined)
})

describe('carregarIdentidades', () => {
  it('grava a lista sem ligar ocupado', async () => {
    await carregarIdentidades()
    expect(getContaEstado().identidades).toEqual([g, m])
    expect(getContaEstado().ocupado).toBe(false)
  })

  it('erro mantém a lista e põe aviso', async () => {
    setContaEstado({ identidades: [g] })
    f.auth.getUserIdentities.mockResolvedValueOnce({ data: null, error: { message: 'falhou' } })
    await carregarIdentidades()
    expect(getContaEstado().identidades).toEqual([g])
    expect(getContaEstado().aviso).toBe('Não foi possível carregar as contas vinculadas.')
  })

  it('sem cliente: aviso', async () => {
    vi.mocked(clienteAtual).mockReturnValue(null)
    await carregarIdentidades()
    expect(getContaEstado().aviso).toBe('Login indisponível neste servidor.')
  })
})

describe('vincular', () => {
  it('chama linkIdentity e abre a URL', async () => {
    await vincular('google')
    expect(f.auth.linkIdentity).toHaveBeenCalledWith({
      provider: 'google',
      options: { redirectTo: REDIRECT, skipBrowserRedirect: true },
    })
    expect(abrir).toHaveBeenCalledWith('https://login.exemplo.com/x')
  })

  it('url http:// vira aviso', async () => {
    f.auth.linkIdentity.mockResolvedValueOnce({ data: { url: 'http://x' }, error: null })
    await vincular('azure')
    expect(abrir).not.toHaveBeenCalled()
    expect(getContaEstado().aviso).toBe('Não foi possível abrir o login.')
  })

  it('erro vira aviso', async () => {
    f.auth.linkIdentity.mockResolvedValueOnce({ data: { url: null }, error: { message: 'ruim' } })
    await vincular('google')
    expect(getContaEstado().aviso).toBe('Não foi possível abrir o login.')
  })

  it('sem cliente: aviso', async () => {
    vi.mocked(clienteAtual).mockReturnValue(null)
    await vincular('google')
    expect(getContaEstado().aviso).toBe('Login indisponível neste servidor.')
  })
})

describe('desvincular', () => {
  it('com 1 identidade: aviso e não chama a rede', async () => {
    setContaEstado({ identidades: [g] })
    await desvincular(g)
    expect(f.auth.unlinkIdentity).not.toHaveBeenCalled()
    expect(getContaEstado().aviso).toBe('Mantenha ao menos uma forma de entrar.')
  })

  it('com 2: desvincula e recarrega a lista', async () => {
    setContaEstado({ identidades: [g, m] })
    f.auth.getUserIdentities.mockResolvedValueOnce({ data: { identities: [g] }, error: null })
    await desvincular(m)
    expect(f.auth.unlinkIdentity).toHaveBeenCalledWith(m)
    expect(getContaEstado().identidades).toEqual([g])
  })

  it('erro no unlink: aviso e não recarrega', async () => {
    setContaEstado({ identidades: [g, m] })
    f.auth.unlinkIdentity.mockResolvedValueOnce({ error: { message: 'x' } })
    await desvincular(m)
    expect(f.auth.getUserIdentities).not.toHaveBeenCalled()
    expect(getContaEstado().aviso).toBe('Não foi possível desvincular.')
  })
})

describe('excluirConta', () => {
  it('invoca a função e depois sai', async () => {
    await excluirConta()
    expect(f.functions.invoke).toHaveBeenCalledWith('excluir-conta')
    expect(sair).toHaveBeenCalledTimes(1)
  })

  it('falha: aviso e não sai', async () => {
    f.functions.invoke.mockResolvedValueOnce({ error: { message: 'x' } })
    await excluirConta()
    expect(sair).not.toHaveBeenCalled()
    expect(getContaEstado().aviso).toBe('Não foi possível excluir a conta.')
    expect(getContaEstado().ocupado).toBe(false)
  })

  it('falha com a mensagem do servidor: mostra a mensagem', async () => {
    const context = new Response(JSON.stringify({ erro: 'cobranca', mensagem: 'Não deu para cancelar a assinatura agora.' }), {
      status: 502,
    })
    f.functions.invoke.mockResolvedValueOnce({ error: { message: 'Edge Function returned a non-2xx status code', context } })
    await excluirConta()
    expect(sair).not.toHaveBeenCalled()
    expect(getContaEstado().aviso).toBe('Não deu para cancelar a assinatura agora.')
  })

  it('exceção: aviso e não sai', async () => {
    f.functions.invoke.mockRejectedValueOnce(new Error('rede'))
    await excluirConta()
    expect(sair).not.toHaveBeenCalled()
    expect(getContaEstado().aviso).toBe('Não foi possível excluir a conta.')
  })

  it('sem cliente: aviso', async () => {
    vi.mocked(clienteAtual).mockReturnValue(null)
    await excluirConta()
    expect(getContaEstado().aviso).toBe('Login indisponível neste servidor.')
    expect(sair).not.toHaveBeenCalled()
  })
})
