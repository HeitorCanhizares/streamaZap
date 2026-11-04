// Tarefa ui-27
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { mockSnapshot } from '../../bridge/mock'
import { useSnapshot } from '../../bridge/store'
import type { Snapshot } from '../../bridge/types'
import { DiagScreen } from './DiagScreen'

vi.mock('../../bridge/store', () => ({ useSnapshot: vi.fn() }))

const mocked = vi.mocked(useSnapshot)

// Snapshot do mock com campos sobrescritos.
function snap(over: Partial<Snapshot> = {}): Snapshot {
  return { ...mockSnapshot(), ...over }
}

beforeEach(() => {
  mocked.mockReturnValue(mockSnapshot())
})

afterEach(() => cleanup())

describe('DiagScreen', () => {
  it('renderiza_mock: com o mock aparece o título, Rede, Amigos na rede e Como ler', () => {
    render(<DiagScreen />)
    expect(screen.getByText('Tá tudo conectando?')).toBeTruthy()
    expect(screen.getByText('Rede')).toBeTruthy()
    expect(screen.getByText('Amigos na rede')).toBeTruthy()
    expect(screen.getByText(/Como ler/)).toBeTruthy()
  })

  it('veredito_ok: snapshot sem peers problemáticos mostra Tudo certo', () => {
    mocked.mockReturnValue(
      snap({ peers: mockSnapshot().peers.map((p) => ({ ...p, receives_you: true })) }),
    )
    render(<DiagScreen />)
    expect(screen.getByText('Tudo certo')).toBeTruthy()
  })

  it('veredito_atencao: um peer com receives_you=false mostra Atenção', () => {
    mocked.mockReturnValue(
      snap({
        peers: [{ name: 'Dudu', version: '3.0.0', ip: '26.57.210.9', receives_you: false }],
      }),
    )
    render(<DiagScreen />)
    expect(screen.getByText('Atenção')).toBeTruthy()
  })

  it('sem_rede: interfaces vazias mostra Sem rede', () => {
    mocked.mockReturnValue(snap({ network: { radmin_ip: null, interfaces: [] } }))
    render(<DiagScreen />)
    expect(screen.getByText('Sem rede')).toBeTruthy()
  })

  it('proximos: Servidor e Firewall mostram em breve', () => {
    render(<DiagScreen />)
    expect(screen.getByText('Servidor')).toBeTruthy()
    expect(screen.getByText('Firewall')).toBeTruthy()
    expect(screen.getAllByText(/em breve/).length).toBeGreaterThanOrEqual(2)
  })
})
