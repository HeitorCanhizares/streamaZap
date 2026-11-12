import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { act } from 'react'
import { ContaEntrar } from './ContaEntrar'
import * as acoes from '../../bridge/contaAcoes'
import { resetConta, setContaEstado } from '../../bridge/contaStore'

vi.mock('../../bridge/contaAcoes')

afterEach(() => {
  cleanup()
  resetConta()
  vi.clearAllMocks()
})

describe('ContaEntrar', () => {
  it('botões OAuth chamam entrarOAuth', () => {
    render(<ContaEntrar />)
    fireEvent.click(screen.getByText('Entrar com Google'))
    fireEvent.click(screen.getByText('Entrar com Microsoft'))
    expect(acoes.entrarOAuth).toHaveBeenNthCalledWith(1, 'google')
    expect(acoes.entrarOAuth).toHaveBeenNthCalledWith(2, 'azure')
  })

  it('só mostra os provedores ligados', () => {
    render(<ContaEntrar />)
    act(() => setContaEstado({ provedores: ['google'] }))
    expect(screen.getByText('Entrar com Google')).toBeTruthy()
    expect(screen.queryByText('Entrar com Microsoft')).toBeNull()
    act(() => setContaEstado({ provedores: [] }))
    expect(screen.queryByText('Entrar com Google')).toBeNull()
    expect(screen.getByText('Enviar código')).toBeTruthy()
  })

  it('envia o e-mail digitado', () => {
    render(<ContaEntrar />)
    expect(screen.queryByLabelText('Código de 6 dígitos')).toBeNull()
    fireEvent.change(screen.getByLabelText('E-mail'), { target: { value: 'a@b.c' } })
    fireEvent.click(screen.getByText('Enviar código'))
    expect(acoes.entrarEmail).toHaveBeenCalledWith('a@b.c')
  })

  describe('com otpEmail', () => {
    function abrir(): void {
      render(<ContaEntrar />)
      act(() => setContaEstado({ otpEmail: 'a@b.c' }))
    }

    it('mostra o texto e troca os campos', () => {
      abrir()
      expect(screen.getByText('Enviamos um código de 6 dígitos para a@b.c.')).toBeTruthy()
      expect(screen.getByLabelText('Código de 6 dígitos')).toBeTruthy()
      expect(screen.queryByLabelText('E-mail')).toBeNull()
      expect(screen.queryByText('Enviar código')).toBeNull()
    })

    it('filtra só dígitos e confirma', () => {
      abrir()
      const campo = screen.getByLabelText('Código de 6 dígitos') as HTMLInputElement
      fireEvent.change(campo, { target: { value: '12a34b56' } })
      expect(campo.value).toBe('123456')
      fireEvent.click(screen.getByText('Confirmar'))
      expect(acoes.confirmarCodigo).toHaveBeenCalledWith('123456')
    })

    it('Cancelar chama cancelarCodigo', () => {
      abrir()
      fireEvent.click(screen.getByText('Cancelar'))
      expect(acoes.cancelarCodigo).toHaveBeenCalled()
    })
  })

  it('aviso aparece em role alert', () => {
    render(<ContaEntrar />)
    act(() => setContaEstado({ aviso: 'Deu ruim' }))
    expect(screen.getByRole('alert').textContent).toBe('Deu ruim')
  })

  it('ocupado desabilita os botões', () => {
    render(<ContaEntrar />)
    act(() => setContaEstado({ ocupado: true }))
    for (const b of screen.getAllByRole('button')) expect((b as HTMLButtonElement).disabled).toBe(true)
  })
})
