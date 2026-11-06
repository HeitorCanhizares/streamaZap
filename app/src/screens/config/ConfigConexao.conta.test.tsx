import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { ConfigConexao } from './ConfigConexao'
import { mockSettings, mockSnapshot } from '../../bridge/mock'
import { resetSnapshotForTests, setSnapshot } from '../../bridge/store'
import type { AccountView, NetworkStatus, Settings } from '../../bridge/types'

vi.mock('./ConfigConta', () => ({ ConfigConta: () => <div data-testid="config-conta" /> }))

const rede: NetworkStatus = { radmin_ip: null, interfaces: ['Ethernet'] }

function ajustes(enabled: boolean): Settings {
  return { ...mockSettings(), server: { enabled, address: '', password: '', use_relay: true } }
}

function montar(enabled: boolean, account: AccountView | undefined, onChange = vi.fn()) {
  const snap = { ...mockSnapshot(), room: null }
  setSnapshot(account ? { ...snap, account } : { ...snap, account: undefined })
  render(<ConfigConexao settings={ajustes(enabled)} onChange={onChange} network={rede} />)
  return onChange
}

beforeEach(() => resetSnapshotForTests())
afterEach(() => {
  cleanup()
  resetSnapshotForTests()
})

describe('ConfigConexao: conta e senha condicional', () => {
  it('requires_password true: senha presente e Conta vem antes dela', () => {
    montar(true, { requires_account: false, requires_password: true })
    const senha = screen.getByLabelText('Senha do servidor')
    const conta = screen.getByTestId('config-conta')
    expect(conta.compareDocumentPosition(senha) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
  })

  it('requires_password false: senha ausente, mas Testar conexão continua', () => {
    montar(true, { requires_account: false, requires_password: false })
    expect(screen.queryByLabelText('Senha do servidor')).toBeNull()
    expect(screen.getByText('Testar conexão')).toBeTruthy()
    expect(screen.getByTestId('config-conta')).toBeTruthy()
  })

  it('snapshot sem account: senha ausente', () => {
    montar(true, undefined)
    expect(screen.queryByLabelText('Senha do servidor')).toBeNull()
  })

  it('Online desligado: nem conta nem senha', () => {
    montar(false, { requires_account: true, requires_password: true })
    expect(screen.queryByTestId('config-conta')).toBeNull()
    expect(screen.queryByLabelText('Senha do servidor')).toBeNull()
  })

  it('requires_account true mostra "Precisa de conta"', () => {
    montar(true, { requires_account: true, requires_password: false })
    expect(screen.getByText(/Precisa de conta\. Salas pela internet/)).toBeTruthy()
  })

  it('requires_account false ou ausente não mostra "Precisa de conta"', () => {
    montar(true, { requires_account: false, requires_password: false })
    expect(screen.queryByText(/Precisa de conta/)).toBeNull()
    cleanup()
    montar(true, undefined)
    expect(screen.queryByText(/Precisa de conta/)).toBeNull()
  })

  it('digitar na senha chama onChange com server.password', () => {
    const onChange = montar(true, { requires_account: false, requires_password: true })
    fireEvent.change(screen.getByLabelText('Senha do servidor'), { target: { value: 'abc' } })
    expect(onChange).toHaveBeenCalledWith({ server: { enabled: true, address: '', password: 'abc', use_relay: true } })
  })
})
