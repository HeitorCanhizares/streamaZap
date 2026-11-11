// Tarefa telas-12: Entrar sem plano mostra o convite e não chama a ponte.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { api } from '../../bridge/api'
import { mockSnapshotComPlano } from '../../bridge/mock'
import { PRECOS_MOCK } from '../../bridge/mockConta'
import { resetOnlineEnabled, setOnlineEnabled } from '../../bridge/onlineStore'
import { resetPlano, setPlanoEstado } from '../../bridge/planoStore'
import { resetSnapshotForTests, setSnapshot } from '../../bridge/store'
import { SalasScreen } from './SalasScreen'

vi.mock('./AtualizadoCard', () => ({ AtualizadoCard: () => <div data-testid="atualizado-card" /> }))

const CONVITE_COBRANCA = 'Assine o Espectador (R$ 3,90/mês ou R$ 39/ano) para assistir'
const CONVITE_SEM = 'Sua conta não tem um plano para entrar em salas Online'

function cobranca(ativa: boolean): void {
  act(() => {
    setPlanoEstado({ config: { cobrancaAtiva: ativa, planoPadrao: null, precos: ativa ? PRECOS_MOCK : null } })
  })
}

function montar(plano: 'nenhum' | 'espectador'): { onEntered: ReturnType<typeof vi.fn> } {
  setSnapshot(mockSnapshotComPlano(plano))
  const onEntered = vi.fn()
  render(<SalasScreen onCreate={() => {}} onEntered={onEntered} />)
  return { onEntered }
}

beforeEach(() => setOnlineEnabled(true))
afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
  resetOnlineEnabled()
  resetPlano()
  resetSnapshotForTests()
})

describe('SalasScreen sem plano para entrar', () => {
  it('com cobrança mostra o convite e Entrar não chama roomJoin', () => {
    const spy = vi.spyOn(api, 'roomJoin').mockResolvedValue()
    cobranca(true)
    montar('nenhum')
    expect(screen.getByText(CONVITE_COBRANCA)).toBeTruthy()
    fireEvent.click(screen.getByText('Entrar na sala'))
    expect(spy).not.toHaveBeenCalled()
  })

  it('sem cobrança mostra só o texto, sem preço nem Ver planos', () => {
    cobranca(false)
    montar('nenhum')
    expect(screen.getByText(CONVITE_SEM)).toBeTruthy()
    expect(screen.queryByText('Ver planos')).toBeNull()
    expect(document.body.textContent).not.toContain('R$')
  })

  it('entrar por código também não chama roomJoin', () => {
    const spy = vi.spyOn(api, 'roomJoin').mockResolvedValue()
    cobranca(true)
    montar('nenhum')
    fireEvent.click(screen.getByText('Entrar com código'))
    fireEvent.change(screen.getByLabelText('Código da sala'), { target: { value: 'ZAP-7K2Q' } })
    fireEvent.click(screen.getAllByRole('button', { name: 'Entrar' })[0])
    expect(spy).not.toHaveBeenCalled()
    expect(screen.getByText(CONVITE_COBRANCA)).toBeTruthy()
  })

  it('plano espectador entra normalmente', async () => {
    const spy = vi.spyOn(api, 'roomJoin').mockResolvedValue()
    cobranca(true)
    const { onEntered } = montar('espectador')
    expect(screen.queryByText(CONVITE_COBRANCA)).toBeNull()
    fireEvent.click(screen.getByText('Entrar na sala'))
    await waitFor(() => expect(onEntered).toHaveBeenCalledTimes(1))
    expect(spy).toHaveBeenCalledWith({ room_id: 'zap-7k2q' })
  })

  it('modo Local com a mesma conta entra normalmente', async () => {
    const spy = vi.spyOn(api, 'roomJoin').mockResolvedValue()
    cobranca(true)
    const { onEntered } = montar('nenhum')
    fireEvent.click(screen.getByRole('tab', { name: /Local/ }))
    expect(screen.queryByText(CONVITE_COBRANCA)).toBeNull()
    fireEvent.click(screen.getByText('Entrar na sala'))
    await waitFor(() => expect(onEntered).toHaveBeenCalledTimes(1))
    expect(spy).toHaveBeenCalledWith({ room_id: 'zap-b5tf' })
  })
})
