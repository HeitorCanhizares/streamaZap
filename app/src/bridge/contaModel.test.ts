import { describe, it, expect } from 'vitest'
import { rotuloProvedor, nomePlano, podeDesvincular, temLogin, provedorOAuth } from './contaModel'
import type { AccountView, PlanView } from './types'
import type { ContaIdentidade } from './contaTipos'

describe('rotuloProvedor', () => {
  it.each([
    ['google', 'Google'],
    ['azure', 'Microsoft'],
    ['email', 'E-mail'],
    ['github', 'github'],
  ])('%s -> %s', (entrada, esperado) => {
    expect(rotuloProvedor(entrada)).toBe(esperado)
  })
})

describe('nomePlano', () => {
  it.each([
    ['nenhum', 'Sem plano'],
    ['espectador', 'Espectador'],
    ['hd', 'Transmissor HD'],
    ['fullhd', 'Transmissor Full HD'],
  ] as const)('%s -> %s', (id, esperado) => {
    const plano: PlanView = { id, pode_entrar: true }
    expect(nomePlano(plano)).toBe(esperado)
  })
})

describe('podeDesvincular', () => {
  const ident = (n: number): ContaIdentidade[] =>
    Array.from({ length: n }, (_, i) => ({ identity_id: `i${i}`, provider: 'google' }))

  it.each([
    [0, false],
    [1, false],
    [2, true],
    [3, true],
  ])('com %i identidades -> %s', (n, esperado) => {
    expect(podeDesvincular(ident(n))).toBe(esperado)
  })
})

describe('temLogin', () => {
  const base: AccountView = { requires_account: false, requires_password: false }

  it('sem conta: false', () => {
    expect(temLogin(undefined)).toBe(false)
  })
  it('sem account_provider: false', () => {
    expect(temLogin(base)).toBe(false)
  })
  it('provider diferente de supabase: false', () => {
    expect(temLogin({ ...base, account_provider: { provider: 'outro', url: 'https://x', key: 'k' } })).toBe(false)
  })
  it('url vazia: false', () => {
    expect(temLogin({ ...base, account_provider: { provider: 'supabase', url: '', key: 'k' } })).toBe(false)
  })
  it('supabase com url e key: true', () => {
    expect(temLogin({ ...base, account_provider: { provider: 'supabase', url: 'https://x', key: 'k' } })).toBe(true)
  })
})

describe('provedorOAuth', () => {
  it('google -> google', () => {
    expect(provedorOAuth('google')).toBe('google')
  })
  it('microsoft -> azure', () => {
    expect(provedorOAuth('microsoft')).toBe('azure')
  })
})
