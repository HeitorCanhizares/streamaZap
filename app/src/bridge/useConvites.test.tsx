import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, renderHook } from '@testing-library/react'
import { api } from './api'
import { resetConta, setContaEstado } from './contaStore'
import { ESPERA_CONTA_MS, useConvites } from './useConvites'

let ouvir: ((alvo: string) => void) | null = null
const desfazer = vi.fn()

beforeEach(() => {
  vi.useFakeTimers()
  ouvir = null
  desfazer.mockReset()
  resetConta()
  vi.spyOn(api, 'escutarConvite').mockImplementation(async (cb) => {
    ouvir = cb
    return desfazer
  })
  vi.spyOn(api, 'convitesPendentes').mockResolvedValue([])
})

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
  vi.useRealTimers()
  resetConta()
})

// Deixa as promessas do efeito (escutar, pendentes) resolverem.
async function soltar(): Promise<void> {
  await act(async () => {
    await Promise.resolve()
    await Promise.resolve()
  })
}

describe('useConvites', () => {
  it('com a conta carregada, entrega o convite do evento na hora', async () => {
    setContaEstado({ sessao: 'fora' })
    const { result } = renderHook(() => useConvites())
    await soltar()
    act(() => ouvir?.('ZAP-7K2Q'))
    expect(result.current.pedido).toEqual({ alvo: 'ZAP-7K2Q', seq: 1 })
  })

  it('com a conta carregando, espera a sessão sair de carregando', async () => {
    const { result } = renderHook(() => useConvites())
    await soltar()
    act(() => ouvir?.('ZAP-7K2Q'))
    expect(result.current.pedido).toBeNull()
    act(() => setContaEstado({ sessao: 'dentro' }))
    expect(result.current.pedido?.alvo).toBe('ZAP-7K2Q')
  })

  it('sem conta nenhuma, entrega depois da espera máxima', async () => {
    const { result } = renderHook(() => useConvites())
    await soltar()
    act(() => ouvir?.('26.14.7.88:47800'))
    expect(result.current.pedido).toBeNull()
    act(() => vi.advanceTimersByTime(ESPERA_CONTA_MS))
    expect(result.current.pedido?.alvo).toBe('26.14.7.88:47800')
  })

  it('pega o último convite que chegou com o app fechado', async () => {
    setContaEstado({ sessao: 'fora' })
    vi.mocked(api.convitesPendentes).mockResolvedValue(['ZAP-2345', 'ZAP-7K2Q'])
    const { result } = renderHook(() => useConvites())
    await soltar()
    expect(result.current.pedido?.alvo).toBe('ZAP-7K2Q')
  })

  it('limpar zera; o mesmo alvo de novo vira um pedido novo (seq muda)', async () => {
    setContaEstado({ sessao: 'fora' })
    const { result } = renderHook(() => useConvites())
    await soltar()
    act(() => ouvir?.('ZAP-7K2Q'))
    act(() => result.current.limpar())
    expect(result.current.pedido).toBeNull()
    act(() => ouvir?.('ZAP-7K2Q'))
    expect(result.current.pedido).toEqual({ alvo: 'ZAP-7K2Q', seq: 2 })
  })

  it('desmontar para de escutar', async () => {
    const { unmount } = renderHook(() => useConvites())
    await soltar()
    unmount()
    expect(desfazer).toHaveBeenCalled()
  })
})
