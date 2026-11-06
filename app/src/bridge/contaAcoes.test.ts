import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from './api'
import { clienteAtual } from './contaClient'
import {
  cancelarCodigo,
  confirmarCodigo,
  entrarEmail,
  entrarOAuth,
  REDIRECT,
  sair,
  tratarDeepLink,
} from './contaAcoes'
import { getContaEstado, resetConta, setContaEstado } from './contaStore'
import type { ContaClient, ContaErro } from './contaTipos'

vi.mock('./contaClient', () => ({ clienteAtual: vi.fn() }))

function fake() {
  const auth = {
    signInWithOAuth: vi.fn(async () => ({
      data: { url: 'https://login.exemplo.com/x' as string | null },
      error: null as ContaErro | null,
    })),
    signInWithOtp: vi.fn(async () => ({ error: null as ContaErro | null })),
    verifyOtp: vi.fn(async (_a: unknown) => ({ error: null as ContaErro | null })),
    exchangeCodeForSession: vi.fn(async (_c: string) => ({ error: null as ContaErro | null })),
    signOut: vi.fn(async () => ({ error: null as ContaErro | null })),
  }
  return { auth, cliente: { auth } as unknown as ContaClient }
}

let f: ReturnType<typeof fake>
let abrir: ReturnType<typeof vi.spyOn>

beforeEach(() => {
  resetConta()
  f = fake()
  vi.mocked(clienteAtual).mockReturnValue(f.cliente)
  abrir = vi.spyOn(api, 'abrirUrl').mockResolvedValue(undefined)
})

describe('entrarOAuth', () => {
  it('chama signInWithOAuth e abre a URL https', async () => {
    await entrarOAuth('google')
    expect(f.auth.signInWithOAuth).toHaveBeenCalledWith({
      provider: 'google',
      options: { redirectTo: REDIRECT, skipBrowserRedirect: true },
    })
    expect(REDIRECT).toBe('streamazap://auth/callback')
    expect(abrir).toHaveBeenCalledWith('https://login.exemplo.com/x')
    expect(getContaEstado().ocupado).toBe(false)
  })

  it('azure manda scopes email', async () => {
    await entrarOAuth('azure')
    expect(f.auth.signInWithOAuth).toHaveBeenCalledWith({
      provider: 'azure',
      options: { redirectTo: REDIRECT, skipBrowserRedirect: true, scopes: 'email' },
    })
  })

  it('URL não-https vira aviso', async () => {
    f.auth.signInWithOAuth.mockResolvedValueOnce({ data: { url: 'file:///c:/x' }, error: null })
    await entrarOAuth('google')
    expect(abrir).not.toHaveBeenCalled()
    expect(getContaEstado().aviso).toBe('Não foi possível abrir o login.')
  })

  it('erro do supabase vira aviso', async () => {
    f.auth.signInWithOAuth.mockResolvedValueOnce({ data: { url: null }, error: { message: 'falhou' } })
    await entrarOAuth('google')
    expect(getContaEstado().aviso).toBe('Não foi possível abrir o login.')
    expect(getContaEstado().ocupado).toBe(false)
  })

  it('exceção vira aviso e nunca lança', async () => {
    f.auth.signInWithOAuth.mockRejectedValueOnce(new Error('rede caiu'))
    await expect(entrarOAuth('google')).resolves.toBeUndefined()
    expect(getContaEstado().aviso).toBe('Não foi possível entrar.')
    expect(getContaEstado().ocupado).toBe(false)
  })

  it('sem cliente avisa', async () => {
    vi.mocked(clienteAtual).mockReturnValue(null)
    await entrarOAuth('google')
    expect(getContaEstado().aviso).toBe('Login indisponível neste servidor.')
    expect(abrir).not.toHaveBeenCalled()
  })
})

describe('e-mail e código', () => {
  it('e-mail vazio avisa sem rede', async () => {
    await entrarEmail('   ')
    expect(getContaEstado().aviso).toBe('Digite seu e-mail.')
    expect(f.auth.signInWithOtp).not.toHaveBeenCalled()
  })

  it('e-mail ok guarda otpEmail', async () => {
    await entrarEmail('  a@b.com ')
    expect(f.auth.signInWithOtp).toHaveBeenCalledWith({ email: 'a@b.com' })
    expect(getContaEstado().otpEmail).toBe('a@b.com')
  })

  it('confirma código só com dígitos', async () => {
    setContaEstado({ otpEmail: 'a@b.com' })
    await confirmarCodigo('12 34 56')
    expect(f.auth.verifyOtp).toHaveBeenCalledWith({ email: 'a@b.com', token: '123456', type: 'email' })
    expect(getContaEstado().otpEmail).toBeNull()
  })

  it('5 dígitos avisa sem rede', async () => {
    setContaEstado({ otpEmail: 'a@b.com' })
    await confirmarCodigo('12345')
    expect(getContaEstado().aviso).toBe('Digite o código de 6 dígitos.')
    expect(f.auth.verifyOtp).not.toHaveBeenCalled()
  })

  it('erro do verifyOtp mantém otpEmail', async () => {
    setContaEstado({ otpEmail: 'a@b.com' })
    f.auth.verifyOtp.mockResolvedValueOnce({ error: { message: 'Token has expired or is invalid', code: 'otp_expired' } })
    await confirmarCodigo('123456')
    expect(getContaEstado().aviso).toBe('O código expirou ou está errado. Peça um código novo.')
    expect(getContaEstado().otpEmail).toBe('a@b.com')
  })

  it('cancelarCodigo limpa', () => {
    setContaEstado({ otpEmail: 'a@b.com', aviso: 'x' })
    cancelarCodigo()
    expect(getContaEstado().otpEmail).toBeNull()
    expect(getContaEstado().aviso).toBeNull()
  })
})

describe('sair e deep link', () => {
  it('sair chama signOut', async () => {
    await sair()
    expect(f.auth.signOut).toHaveBeenCalled()
  })

  it('deep link com code troca a sessão', async () => {
    await tratarDeepLink('streamazap://auth/callback?code=abc')
    expect(f.auth.exchangeCodeForSession).toHaveBeenCalledWith('abc')
  })

  it('deep link com erro vira aviso, sem cliente', async () => {
    vi.mocked(clienteAtual).mockReturnValue(null)
    await tratarDeepLink('streamazap://auth/callback?error=access_denied')
    expect(getContaEstado().aviso).toBe('Login cancelado.')
  })

  it('URL alheia não faz nada', async () => {
    await tratarDeepLink('https://exemplo.com/?code=abc')
    expect(f.auth.exchangeCodeForSession).not.toHaveBeenCalled()
    expect(getContaEstado().aviso).toBeNull()
  })
})
