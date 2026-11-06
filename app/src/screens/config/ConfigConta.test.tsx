import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, render, screen } from '@testing-library/react'
import { ConfigConta } from './ConfigConta'
import { resetConta, setContaEstado } from '../../bridge/contaStore'
import { resetSnapshotForTests, setSnapshot } from '../../bridge/store'
import { mockSnapshot } from '../../bridge/mock'
import type { AccountView } from '../../bridge/types'

vi.mock('./ContaEntrar', () => ({ ContaEntrar: () => <div data-testid="stub-entrar" /> }))
vi.mock('./ContaLogada', () => ({
  ContaLogada: ({ plano }: { plano?: { id: string } }) => <div data-testid="stub-logada">{plano?.id}</div>,
}))

const valida: AccountView = {
  account_provider: { provider: 'supabase', url: 'https://x.supabase.co', key: 'k' },
  requires_account: false,
  requires_password: false,
  plan: { id: 'hd', pode_entrar: true },
} as AccountView

function comConta(account: AccountView | undefined): void {
  setSnapshot({ ...mockSnapshot(), account })
}

afterEach(() => {
  cleanup()
  resetConta()
  resetSnapshotForTests()
})

describe('ConfigConta', () => {
  it('sem account não renderiza nada', () => {
    comConta(undefined)
    const { container } = render(<ConfigConta />)
    expect(container.innerHTML).toBe('')
  })

  it('provider outro não renderiza nada', () => {
    comConta({ ...valida, account_provider: { provider: 'outro', url: 'https://x', key: 'k' } })
    const { container } = render(<ConfigConta />)
    expect(container.innerHTML).toBe('')
  })

  it('estado inicial mostra Carregando conta…', () => {
    comConta(valida)
    render(<ConfigConta />)
    expect(screen.getByText('Carregando conta…')).toBeTruthy()
    expect(screen.getByTestId('config-conta')).toBeTruthy()
  })

  it('sessao fora mostra ContaEntrar', () => {
    comConta(valida)
    setContaEstado({ sessao: 'fora' })
    render(<ConfigConta />)
    expect(screen.getByTestId('stub-entrar')).toBeTruthy()
  })

  it('sessao dentro mostra ContaLogada com o plano', () => {
    comConta(valida)
    render(<ConfigConta />)
    act(() => setContaEstado({ sessao: 'dentro' }))
    expect(screen.getByTestId('stub-logada').textContent).toBe('hd')
  })

  it('o mock padrão mostra o bloco', () => {
    setSnapshot(mockSnapshot())
    render(<ConfigConta />)
    expect(screen.getByTestId('config-conta')).toBeTruthy()
  })
})
