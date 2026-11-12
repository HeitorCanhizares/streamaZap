import { act, cleanup, render, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  estadoInicialConta,
  getContaEstado,
  resetConta,
  setContaEstado,
  useContaEstado,
} from './contaStore'

beforeEach(() => {
  resetConta()
})

afterEach(() => {
  cleanup()
  resetConta()
  vi.restoreAllMocks()
})

describe('store da conta', () => {
  it('estado inicial exato', () => {
    expect(estadoInicialConta()).toEqual({
      sessao: 'carregando',
      email: null,
      identidades: [],
      aviso: null,
      otpEmail: null,
      ocupado: false,
      provedores: ['google', 'azure'],
    })
    expect(getContaEstado()).toEqual(estadoInicialConta())
  })

  it('estadoInicialConta devolve objeto novo a cada chamada', () => {
    expect(estadoInicialConta()).not.toBe(estadoInicialConta())
  })

  it('getContaEstado é estável (mesma referência) entre chamadas', () => {
    const a = getContaEstado()
    const b = getContaEstado()
    expect(a).toBe(b)
  })

  it('setContaEstado muda só os campos do patch e cria objeto novo', () => {
    const antes = getContaEstado()
    const identidadesAntes = antes.identidades
    setContaEstado({ email: 'a@b.c' })
    const depois = getContaEstado()
    expect(depois).not.toBe(antes)
    expect(depois.email).toBe('a@b.c')
    expect(depois.sessao).toBe('carregando')
    expect(depois.identidades).toBe(identidadesAntes)
    expect(depois.aviso).toBeNull()
    expect(depois.otpEmail).toBeNull()
    expect(depois.ocupado).toBe(false)
    // O objeto anterior não é mutado.
    expect(antes.email).toBeNull()
  })

  it('patch igual não troca a referência e não avisa', () => {
    setContaEstado({ email: 'x@y.z' })
    const antes = getContaEstado()
    const listener = vi.fn()
    const { unmount } = renderHook(() => {
      listener()
      return useContaEstado()
    })
    const rendersAntes = listener.mock.calls.length
    setContaEstado({ email: 'x@y.z', ocupado: false, sessao: 'carregando' })
    expect(getContaEstado()).toBe(antes)
    expect(listener.mock.calls.length).toBe(rendersAntes)
    unmount()
  })

  it('arrays são comparados por referência (novo array com mesmo conteúdo avisa)', () => {
    const antes = getContaEstado()
    setContaEstado({ identidades: [] })
    expect(getContaEstado()).not.toBe(antes)
  })

  it('componente com useContaEstado re-renderiza após act(setContaEstado)', () => {
    const { result } = renderHook(() => useContaEstado())
    expect(result.current.email).toBeNull()
    act(() => {
      setContaEstado({ email: 'a@b.c' })
    })
    expect(result.current.email).toBe('a@b.c')
    expect(getContaEstado().email).toBe('a@b.c')
  })

  it('resetConta volta ao inicial', () => {
    setContaEstado({ sessao: 'dentro', email: 'a@b.c', ocupado: true, aviso: 'oi' })
    resetConta()
    expect(getContaEstado()).toEqual(estadoInicialConta())
  })

  it('resetConta avisa os inscritos', () => {
    setContaEstado({ sessao: 'fora' })
    const { result } = renderHook(() => useContaEstado())
    act(() => {
      resetConta()
    })
    expect(result.current.sessao).toBe('carregando')
  })

  it('unsubscribe: após desmontar o componente o listener sai (sem erro ao setar)', () => {
    function Teste() {
      useContaEstado()
      return null
    }
    const { unmount } = render(<Teste />)
    unmount()
    expect(() => {
      act(() => {
        setContaEstado({ email: 'depois@desmontar.com' })
      })
    }).not.toThrow()
    expect(getContaEstado().email).toBe('depois@desmontar.com')
  })
})
