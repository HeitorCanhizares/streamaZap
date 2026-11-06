import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { ContaLogada } from './ContaLogada'
import * as acoes from '../../bridge/contaAcoes'
import * as vinculo from '../../bridge/contaVinculo'
import type { ContaIdentidade } from '../../bridge/contaTipos'
import { resetConta, setContaEstado } from '../../bridge/contaStore'

vi.mock('../../bridge/contaAcoes', () => ({ sair: vi.fn() }))
vi.mock('../../bridge/contaVinculo', () => ({
  vincular: vi.fn(),
  desvincular: vi.fn(),
  excluirConta: vi.fn(),
}))

const google = { identity_id: 'g1', provider: 'google', email: 'g@x.com' }
const email = { identity_id: 'e1', provider: 'email', email: 'voce@exemplo.com' }
const azure = { identity_id: 'a1', provider: 'azure' }

function abrir(identidades: ContaIdentidade[] = [google, email], plano?: Parameters<typeof ContaLogada>[0]['plano']): void {
  setContaEstado({ sessao: 'dentro', email: 'voce@exemplo.com', identidades })
  render(<ContaLogada plano={plano} />)
}

afterEach(() => {
  cleanup()
  resetConta()
  vi.clearAllMocks()
})

describe('ContaLogada', () => {
  it('mostra o e-mail e não mostra plano sem a prop', () => {
    abrir()
    expect(screen.getByText('voce@exemplo.com', { selector: 'b' })).toBeTruthy()
    expect(screen.getByText(/Conectado como/)).toBeTruthy()
    expect(screen.queryByText(/Plano/)).toBeNull()
  })

  it('mostra o plano sem preço nem Assinar', () => {
    abrir([google, email], { id: 'fullhd', pode_entrar: true })
    expect(screen.getByText('Plano: Transmissor Full HD')).toBeTruthy()
    expect(screen.queryByText(/Assinar/)).toBeNull()
    expect(screen.queryByText(/R\$/)).toBeNull()
  })

  it('lista as identidades', () => {
    abrir()
    expect(screen.getByText('Google · g@x.com')).toBeTruthy()
    expect(screen.getByText('E-mail · voce@exemplo.com')).toBeTruthy()
  })

  it('com 1 identidade o Desvincular fica desabilitado e a dica aparece', () => {
    abrir([google])
    expect((screen.getByLabelText('Desvincular Google') as HTMLButtonElement).disabled).toBe(true)
    expect(screen.getByText('Mantenha ao menos uma forma de entrar.')).toBeTruthy()
  })

  it('com 2 identidades desvincula a certa', () => {
    abrir()
    const botao = screen.getByLabelText('Desvincular E-mail') as HTMLButtonElement
    expect(botao.disabled).toBe(false)
    expect(screen.queryByText('Mantenha ao menos uma forma de entrar.')).toBeNull()
    fireEvent.click(botao)
    expect(vinculo.desvincular).toHaveBeenCalledWith(email)
  })

  it('Vincular Microsoft só aparece sem azure e chama vincular', () => {
    abrir()
    expect(screen.queryByText('Vincular Google')).toBeNull()
    fireEvent.click(screen.getByText('Vincular Microsoft'))
    expect(vinculo.vincular).toHaveBeenCalledWith('azure')
    cleanup()
    abrir([google, azure])
    expect(screen.queryByText('Vincular Microsoft')).toBeNull()
  })

  it('Sair chama sair', () => {
    abrir()
    fireEvent.click(screen.getByText('Sair'))
    expect(acoes.sair).toHaveBeenCalled()
  })

  it('Excluir conta pede confirmação', () => {
    abrir()
    expect(screen.queryByRole('alertdialog')).toBeNull()
    fireEvent.click(screen.getByText('Excluir conta'))
    expect(screen.getByRole('alertdialog')).toBeTruthy()
    expect(screen.getByText('Isso apaga sua conta e não pode ser desfeito.')).toBeTruthy()
    expect(vinculo.excluirConta).not.toHaveBeenCalled()
    fireEvent.click(screen.getByText('Cancelar'))
    expect(screen.queryByRole('alertdialog')).toBeNull()
    fireEvent.click(screen.getByText('Excluir conta'))
    fireEvent.click(screen.getByText('Excluir de vez'))
    expect(vinculo.excluirConta).toHaveBeenCalledTimes(1)
  })

  it('aviso aparece em role alert', () => {
    abrir()
    act(() => setContaEstado({ aviso: 'Deu ruim' }))
    expect(screen.getByRole('alert').textContent).toBe('Deu ruim')
  })
})
