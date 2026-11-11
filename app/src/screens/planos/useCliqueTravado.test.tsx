import { act, renderHook } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const cobranca = vi.hoisted(() => ({ valor: true }))
vi.mock('../../bridge/usePlano', () => ({ useCobrancaAtiva: () => cobranca.valor }))
vi.mock('../../bridge/planoAcoes', () => ({ abrirPlanos: vi.fn() }))

import { abrirPlanos } from '../../bridge/planoAcoes'
import { useCliqueTravado } from './useCliqueTravado'

describe('useCliqueTravado', () => {
  beforeEach(() => {
    vi.mocked(abrirPlanos).mockClear()
    cobranca.valor = true
  })

  it('needs fullhd abre planos em fullhd', () => {
    const { result } = renderHook(() => useCliqueTravado())
    act(() => result.current.aoClicar({ preset: '1080p60', needs: 'fullhd' }))
    expect(abrirPlanos).toHaveBeenCalledWith('fullhd')
    expect(result.current.explicacao).toBeNull()
  })

  it('needs hd abre planos em hd', () => {
    const { result } = renderHook(() => useCliqueTravado())
    act(() => result.current.aoClicar({ preset: '720p60', needs: 'hd' }))
    expect(abrirPlanos).toHaveBeenCalledWith('hd')
  })

  it('sem needs cai em hd', () => {
    const { result } = renderHook(() => useCliqueTravado())
    act(() => result.current.aoClicar({ preset: '720p60' }))
    expect(abrirPlanos).toHaveBeenCalledWith('hd')
  })

  it('sem cobrança só explica', () => {
    cobranca.valor = false
    const { result } = renderHook(() => useCliqueTravado())
    act(() => result.current.aoClicar({ preset: '1080p60', needs: 'fullhd' }))
    expect(abrirPlanos).not.toHaveBeenCalled()
    expect(result.current.explicacao).toBe('Seu plano não permite essa qualidade')
  })
})
