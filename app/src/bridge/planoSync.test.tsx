import { act } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('./planoAcoes', () => ({
  carregarConfig: vi.fn(async () => undefined),
  carregarAssinatura: vi.fn(async () => undefined),
  atualizarDepoisDePagar: vi.fn(async () => undefined),
}))
vi.mock('./contaClient', () => ({
  obterCliente: vi.fn(),
  descartarCliente: vi.fn(async () => undefined),
}))
vi.mock('./contaAcoes', () => ({ tratarDeepLink: vi.fn(async () => undefined) }))
vi.mock('./contaVinculo', () => ({ carregarIdentidades: vi.fn(async () => undefined) }))

import { api } from './api'
import { carregarConfig, carregarAssinatura, atualizarDepoisDePagar } from './planoAcoes'
import { obterCliente } from './contaClient'
import { resetConta } from './contaStore'
import { iniciarConta } from './contaSync'
import { ESPERA_FOCO_MS, INTERVALO_PLANOS_MS, iniciarPlanos } from './planoSync'
import { estadoInicialPlano, getPlanoEstado, resetPlano, setPlanoEstado } from './planoStore'
import type { AccountProviderView } from './types'

const focar = () => window.dispatchEvent(new Event('focus'))

beforeEach(() => {
  vi.useFakeTimers()
  vi.setSystemTime(new Date('2026-01-01T00:00:00Z'))
  vi.mocked(carregarConfig).mockClear()
  vi.mocked(carregarAssinatura).mockClear()
  vi.mocked(atualizarDepoisDePagar).mockClear()
  resetPlano()
})

afterEach(() => {
  vi.useRealTimers()
  vi.restoreAllMocks()
  resetPlano()
})

describe('iniciarPlanos', () => {
  it('carrega a config na hora e a cada hora; assinatura só com sessão', () => {
    const p = iniciarPlanos()
    expect(carregarConfig).toHaveBeenCalledTimes(1)
    vi.advanceTimersByTime(INTERVALO_PLANOS_MS)
    expect(carregarConfig).toHaveBeenCalledTimes(2)
    expect(carregarAssinatura).not.toHaveBeenCalled()
    p.aoEntrar()
    expect(carregarAssinatura).toHaveBeenCalledTimes(1)
    vi.advanceTimersByTime(INTERVALO_PLANOS_MS)
    expect(carregarAssinatura).toHaveBeenCalledTimes(2)
    p.parar()
  })

  it('foco só atualiza enquanto espera o pagamento, no máximo a cada 10 s', () => {
    const p = iniciarPlanos()
    focar()
    expect(atualizarDepoisDePagar).not.toHaveBeenCalled()
    setPlanoEstado({ aguardandoPagamento: true })
    focar()
    expect(atualizarDepoisDePagar).toHaveBeenCalledTimes(1)
    vi.advanceTimersByTime(5_000)
    focar()
    expect(atualizarDepoisDePagar).toHaveBeenCalledTimes(1)
    vi.advanceTimersByTime(ESPERA_FOCO_MS - 5_000)
    focar()
    expect(atualizarDepoisDePagar).toHaveBeenCalledTimes(2)
    p.parar()
  })

  it('aoSair zera o que é da conta', () => {
    const p = iniciarPlanos()
    setPlanoEstado({ aguardandoPagamento: true, telaAberta: true, destaque: 'pro' as never })
    p.aoEntrar()
    p.aoSair()
    const e = getPlanoEstado()
    expect(e.assinatura).toBeNull()
    expect(e.aguardandoPagamento).toBe(false)
    expect(e.telaAberta).toBe(false)
    expect(e.destaque).toBeNull()
    vi.advanceTimersByTime(INTERVALO_PLANOS_MS)
    expect(carregarAssinatura).toHaveBeenCalledTimes(1)
    p.parar()
  })

  it('parar desliga o intervalo e o foco (idempotente)', () => {
    const p = iniciarPlanos()
    setPlanoEstado({ aguardandoPagamento: true })
    p.parar()
    p.parar()
    vi.advanceTimersByTime(2 * INTERVALO_PLANOS_MS)
    focar()
    expect(carregarConfig).toHaveBeenCalledTimes(1)
    expect(atualizarDepoisDePagar).not.toHaveBeenCalled()
  })
})

describe('integração com iniciarConta', () => {
  const conta: AccountProviderView = { provider: 'supabase', url: 'https://x.supabase.co', key: 'k' }
  type Cb = (e: string, s: { access_token: string; user?: { email?: string } } | null) => void
  let cb: Cb | null

  beforeEach(() => {
    cb = null
    vi.mocked(obterCliente).mockReset()
    vi.mocked(obterCliente).mockResolvedValue({
      auth: {
        onAuthStateChange: (c: Cb) => {
          cb = c
          return { data: { subscription: { unsubscribe: vi.fn() } } }
        },
      },
    } as never)
    vi.spyOn(api, 'onlineSetAccount').mockResolvedValue(undefined)
    vi.spyOn(api, 'escutarDeepLink').mockResolvedValue((() => undefined) as never)
    vi.spyOn(api, 'deepLinkPendentes').mockResolvedValue([])
    resetConta()
  })

  afterEach(() => resetConta())

  it('desligado: não chama config nem cliente', async () => {
    iniciarConta(conta, false)()
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10)
    })
    expect(carregarConfig).not.toHaveBeenCalled()
    expect(obterCliente).not.toHaveBeenCalled()
  })

  it('ligado: config ao iniciar, assinatura ao entrar, zera ao sair e limpa ao parar', async () => {
    const limpar = iniciarConta(conta, true)
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10)
    })
    expect(carregarConfig).toHaveBeenCalledTimes(1)
    expect(cb).not.toBeNull()
    act(() => cb?.('SIGNED_IN', { access_token: 't', user: { email: 'a@b.c' } }))
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1)
    })
    expect(carregarAssinatura).toHaveBeenCalledTimes(1)

    setPlanoEstado({ assinatura: { plano: 'pro' } as never })
    act(() => cb?.('SIGNED_OUT', null))
    expect(getPlanoEstado().assinatura).toBeNull()

    setPlanoEstado({ telaAberta: true })
    limpar()
    expect(getPlanoEstado()).toEqual(estadoInicialPlano())
    vi.mocked(carregarConfig).mockClear()
    vi.advanceTimersByTime(2 * INTERVALO_PLANOS_MS)
    expect(carregarConfig).not.toHaveBeenCalled()
  })
})
