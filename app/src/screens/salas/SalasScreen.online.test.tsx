// Tarefa front-04: tela Salas com o modo Online desligado/ligado.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen, within } from '@testing-library/react'
import { resetOnlineEnabled, setOnlineEnabled } from '../../bridge/onlineStore'
import { resetSnapshotForTests } from '../../bridge/store'
import { SalasScreen } from './SalasScreen'

vi.mock('./AtualizadoCard', () => ({ AtualizadoCard: () => <div data-testid="atualizado-card" /> }))

afterEach(() => {
  cleanup()
  resetOnlineEnabled()
  resetSnapshotForTests()
})

describe('SalasScreen com Online desligado', () => {
  beforeEach(() => setOnlineEnabled(false))

  it('sem abas, sem código e sem sala online', () => {
    render(<SalasScreen onCreate={() => {}} onEntered={() => {}} />)
    expect(screen.queryAllByRole('tab')).toHaveLength(0)
    expect(screen.queryByRole('button', { name: /online/i })).toBeNull()
    expect(screen.queryByText(/^Online$/)).toBeNull()
    expect(screen.queryByText('Entrar com código')).toBeNull()
    expect(screen.getByText('Entrar por IP')).toBeTruthy()
    expect(screen.queryByText(/ZAP-/)).toBeNull()
  })

  it('texto "servidor" só dentro do cartão', () => {
    render(<SalasScreen onCreate={() => {}} onEntered={() => {}} />)
    const cartao = screen.getByTestId('cartao-online')
    for (const el of screen.queryAllByText(/servidor/i)) expect(cartao.contains(el)).toBe(true)
    expect(within(cartao).getByText(/ative o modo Online em Configurações/)).toBeTruthy()
    expect(screen.getByTestId('atualizado-card')).toBeTruthy()
  })
})

describe('SalasScreen com Online ligado', () => {
  beforeEach(() => setOnlineEnabled(true))

  it('mostra abas e não mostra o cartão', () => {
    render(<SalasScreen onCreate={() => {}} onEntered={() => {}} />)
    expect(screen.getAllByRole('tab').length).toBe(2)
    expect(screen.queryByTestId('cartao-online')).toBeNull()
    expect(screen.getByTestId('atualizado-card')).toBeTruthy()
  })
})
