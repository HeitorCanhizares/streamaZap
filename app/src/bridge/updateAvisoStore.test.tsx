import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from './api'
import {
  __pedirAplicadaParaTeste,
  dispensarAplicada,
  resetUpdateAviso,
  setUpdateAplicada,
  setUpdateAviso,
  useUpdateAplicada,
  useUpdateAviso,
} from './updateAvisoStore'

beforeEach(() => resetUpdateAviso())
afterEach(() => vi.restoreAllMocks())

describe('aviso de 30 s', () => {
  it('começa null, guarda e limpa com segundos 0', () => {
    const { result } = renderHook(() => useUpdateAviso())
    expect(result.current).toBeNull()
    const a = { versao: '3.4.0', segundos: 30 }
    act(() => setUpdateAviso(a))
    expect(result.current).toBe(a)
    act(() => setUpdateAviso({ versao: '3.4.0', segundos: 0 }))
    expect(result.current).toBeNull()
  })

  it('devolve a mesma referência sem mudança', () => {
    const a = { versao: '3.4.0', segundos: 30 }
    act(() => setUpdateAviso(a))
    const { result, rerender } = renderHook(() => useUpdateAviso())
    const primeira = result.current
    rerender()
    expect(result.current).toBe(primeira)
  })
})

describe('atualização aplicada', () => {
  const ap = { versao: '3.4.0', anterior: '3.3.0', notas: 'Novidades' }

  it('começa null, aparece e é dispensada', () => {
    const { result, rerender } = renderHook(() => useUpdateAplicada())
    expect(result.current).toBeNull()
    act(() => setUpdateAplicada(ap))
    expect(result.current).toBe(ap)
    rerender()
    expect(result.current).toBe(ap)
    act(() => dispensarAplicada())
    expect(result.current).toBeNull()
  })

  it('reset zera os dois estados', () => {
    const { result } = renderHook(() => ({ a: useUpdateAviso(), b: useUpdateAplicada() }))
    act(() => {
      setUpdateAviso({ versao: '3.4.0', segundos: 30 })
      setUpdateAplicada(ap)
    })
    expect(result.current.a).not.toBeNull()
    expect(result.current.b).not.toBeNull()
    act(() => resetUpdateAviso())
    expect(result.current.a).toBeNull()
    expect(result.current.b).toBeNull()
  })

  it('fora do Tauri não chama updateAppliedTake', () => {
    const spy = vi.spyOn(api, 'updateAppliedTake')
    renderHook(() => useUpdateAplicada())
    expect(spy).not.toHaveBeenCalled()
  })

  it('pedido guarda o resultado e só consulta o Rust uma vez', async () => {
    const r = { versao: '3.4.0', anterior: '3.3.0', notas: null }
    const spy = vi.spyOn(api, 'updateAppliedTake').mockResolvedValue(r)
    const { result } = renderHook(() => useUpdateAplicada())
    await act(async () => {
      await __pedirAplicadaParaTeste()
      await __pedirAplicadaParaTeste()
    })
    expect(spy).toHaveBeenCalledTimes(1)
    expect(result.current).toEqual(r)
  })

  it('pedido que resolve null mantém o estado null', async () => {
    vi.spyOn(api, 'updateAppliedTake').mockResolvedValue(null)
    const { result } = renderHook(() => useUpdateAplicada())
    await act(async () => {
      await __pedirAplicadaParaTeste()
    })
    expect(result.current).toBeNull()
  })
})
