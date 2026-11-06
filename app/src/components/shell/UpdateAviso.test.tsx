import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '../../bridge/api'
import { resetUpdateAviso, setUpdateAviso } from '../../bridge/updateAvisoStore'
import { UpdateAviso } from './UpdateAviso'

describe('UpdateAviso', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })
  afterEach(() => {
    cleanup()
    act(() => resetUpdateAviso())
    vi.restoreAllMocks()
    vi.useRealTimers()
  })

  it('sem aviso não renderiza nada', () => {
    render(<UpdateAviso />)
    expect(screen.queryByTestId('update-aviso')).toBeNull()
  })

  it('mostra o texto e o botão', () => {
    render(<UpdateAviso />)
    act(() => setUpdateAviso({ versao: '3.4.0', segundos: 30 }))
    expect(screen.getByTestId('update-aviso').textContent).toContain(
      'Atualizando para a v3.4.0 em 30 s',
    )
    expect(screen.getByRole('button', { name: 'Adiar 1 h' })).toBeTruthy()
  })

  it('a contagem regride', () => {
    render(<UpdateAviso />)
    act(() => setUpdateAviso({ versao: '3.4.0', segundos: 30 }))
    act(() => {
      vi.advanceTimersByTime(5000)
    })
    expect(screen.getByTestId('update-aviso').textContent).toContain('em 25 s')
  })

  it('não fica negativa', () => {
    render(<UpdateAviso />)
    act(() => setUpdateAviso({ versao: '3.4.0', segundos: 30 }))
    act(() => {
      vi.advanceTimersByTime(40000)
    })
    expect(screen.getByTestId('update-aviso').textContent).toContain('em 0 s')
  })

  it('adiar chama a api e fecha o aviso', async () => {
    const spy = vi.spyOn(api, 'updatePostpone').mockResolvedValue()
    render(<UpdateAviso />)
    act(() => setUpdateAviso({ versao: '3.4.0', segundos: 30 }))
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Adiar 1 h' }))
    })
    expect(spy).toHaveBeenCalledTimes(1)
    expect(screen.queryByTestId('update-aviso')).toBeNull()
  })

  it('erro ao adiar mostra alerta e mantém o aviso', async () => {
    vi.spyOn(api, 'updatePostpone').mockRejectedValue(new Error('falhou feio'))
    render(<UpdateAviso />)
    act(() => setUpdateAviso({ versao: '3.4.0', segundos: 30 }))
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Adiar 1 h' }))
    })
    expect(screen.getByRole('alert').textContent).toBe('falhou feio')
    expect(screen.getByTestId('update-aviso')).toBeTruthy()
  })

  it('versão com v não duplica', () => {
    render(<UpdateAviso />)
    act(() => setUpdateAviso({ versao: 'v3.4.0', segundos: 30 }))
    const t = screen.getByTestId('update-aviso').textContent ?? ''
    expect(t).toContain('para a v3.4.0')
    expect(t).not.toContain('vv3.4.0')
  })
})
