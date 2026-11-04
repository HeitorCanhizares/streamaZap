import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, renderHook } from '@testing-library/react'
import { api } from '../../bridge/api'
import type { ServerTestResult } from '../../bridge/types'
import { useServerTest } from './useServerTest'

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

const ok: ServerTestResult = { ok: true, name: 'Sala', ping_ms: 30, code: null, message: 'Conectado' }

// Promessa controlada pelo teste, para observar o estado "testando".
function adiada<T>() {
  let resolve!: (v: T) => void
  let reject!: (e: unknown) => void
  const promessa = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promessa, resolve, reject }
}

describe('useServerTest', () => {
  it('sucesso preenche resultado e testando passa por true e volta a false', async () => {
    const d = adiada<ServerTestResult>()
    vi.spyOn(api, 'serverTest').mockReturnValue(d.promessa)
    const { result } = renderHook(() => useServerTest())

    let chamada!: Promise<void>
    act(() => {
      chamada = result.current.testar('127.0.0.1:47800', '')
    })
    expect(result.current.testando).toBe(true)
    expect(result.current.resultado).toBeNull()

    await act(async () => {
      d.resolve(ok)
      await chamada
    })
    expect(result.current.resultado).toEqual(ok)
    expect(result.current.testando).toBe(false)
  })

  it('endereço vazio não chama a API e devolve mensagem de preenchimento', async () => {
    const spy = vi.spyOn(api, 'serverTest')
    const { result } = renderHook(() => useServerTest())

    await act(async () => {
      await result.current.testar('   ', '')
    })
    expect(spy).not.toHaveBeenCalled()
    expect(result.current.resultado?.ok).toBe(false)
    expect(result.current.resultado?.message).toContain('Preencha o endereço do servidor')
    expect(result.current.testando).toBe(false)
  })

  it('rejeição vira resultado ok:false com a mensagem do erro', async () => {
    vi.spyOn(api, 'serverTest').mockRejectedValue(new Error('servidor fora do ar'))
    const { result } = renderHook(() => useServerTest())

    await act(async () => {
      await result.current.testar('host:1', '')
    })
    expect(result.current.resultado).toEqual({
      ok: false,
      name: null,
      ping_ms: null,
      code: 'erro',
      message: 'servidor fora do ar',
    })
    expect(result.current.testando).toBe(false)
  })

  it('limpar durante o teste descarta a resposta tardia', async () => {
    const d = adiada<ServerTestResult>()
    vi.spyOn(api, 'serverTest').mockReturnValue(d.promessa)
    const { result } = renderHook(() => useServerTest())

    let chamada!: Promise<void>
    act(() => {
      chamada = result.current.testar('host:1', '')
    })
    act(() => {
      result.current.limpar()
    })
    expect(result.current.testando).toBe(false)

    await act(async () => {
      d.resolve(ok)
      await chamada
    })
    expect(result.current.resultado).toBeNull()
    expect(result.current.testando).toBe(false)
  })

  it('envia o endereço sem espaços nas bordas', async () => {
    const spy = vi.spyOn(api, 'serverTest').mockResolvedValue(ok)
    const { result } = renderHook(() => useServerTest())

    await act(async () => {
      await result.current.testar('  192.168.0.5:47800  ', 'senha')
    })
    expect(spy).toHaveBeenCalledWith('192.168.0.5:47800', 'senha')
  })
})
