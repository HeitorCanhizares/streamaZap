// Tarefa front-18: cartões de senha do servidor e de conta na tela Salas.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { mockSnapshot } from '../../bridge/mock'
import { resetOnlineEnabled, setOnlineEnabled } from '../../bridge/onlineStore'
import { resetSnapshotForTests, setSnapshot } from '../../bridge/store'
import type { AccountView } from '../../bridge/types'
import { SalasScreen } from './SalasScreen'

vi.mock('./AtualizadoCard', () => ({ AtualizadoCard: () => <div data-testid="atualizado-card" /> }))
vi.mock('./CartaoSenhaServidor', () => ({ CartaoSenhaServidor: () => <div data-testid="cartao-senha-servidor" /> }))
vi.mock('./CartaoContaNecessaria', () => ({ CartaoContaNecessaria: () => <div data-testid="cartao-conta-necessaria" /> }))

function conta(login_error?: string): AccountView {
  return { requires_account: false, requires_password: false, ...(login_error ? { login_error } : {}) }
}

function montar(login_error?: string): void {
  setSnapshot({ ...mockSnapshot(), account: conta(login_error) })
  render(<SalasScreen onCreate={() => {}} onEntered={() => {}} />)
}

beforeEach(() => setOnlineEnabled(true))
afterEach(() => {
  cleanup()
  resetOnlineEnabled()
  resetSnapshotForTests()
})

describe('SalasScreen: cartões de login', () => {
  it('senha_servidor mostra só o cartão de senha', () => {
    montar('senha_servidor')
    expect(screen.getByTestId('cartao-senha-servidor')).toBeTruthy()
    expect(screen.queryByTestId('cartao-conta-necessaria')).toBeNull()
  })

  it('conta_necessaria mostra só o cartão de conta', () => {
    montar('conta_necessaria')
    expect(screen.getByTestId('cartao-conta-necessaria')).toBeTruthy()
    expect(screen.queryByTestId('cartao-senha-servidor')).toBeNull()
  })

  it('sem login_error não mostra cartão', () => {
    montar()
    expect(screen.queryByTestId('cartao-senha-servidor')).toBeNull()
    expect(screen.queryByTestId('cartao-conta-necessaria')).toBeNull()
  })

  it('Online desligado não mostra cartão', () => {
    setOnlineEnabled(false)
    montar('senha_servidor')
    expect(screen.queryByTestId('cartao-senha-servidor')).toBeNull()
    expect(screen.queryByTestId('cartao-conta-necessaria')).toBeNull()
  })

  it('aba Local não mostra cartão', () => {
    montar('conta_necessaria')
    fireEvent.click(screen.getByRole('tab', { name: /Local/ }))
    expect(screen.queryByTestId('cartao-senha-servidor')).toBeNull()
    expect(screen.queryByTestId('cartao-conta-necessaria')).toBeNull()
  })
})
