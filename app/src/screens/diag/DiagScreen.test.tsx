// Tarefa diag-06
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
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

afterEach(() => {
  cleanup()
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

describe('DiagScreen', () => {
  it('renderiza_mock', () => {
    render(<DiagScreen />)
    for (const t of ['Tá tudo conectando?', 'Rede', 'Amigos com o app aberto', 'Salas encontradas', 'Servidor online', 'Firewall do Windows', 'Como ler isso', 'Copiar relatório']) {
      expect(screen.getAllByText(t).length).toBeGreaterThan(0)
    }
  })

  it('cinco_cards', () => {
    const { container } = render(<DiagScreen />)
    expect(container.querySelectorAll('section[data-estado]').length).toBe(5)
  })

  it('veredito_ok', () => {
    const s = mockSnapshot()
    mocked.mockReturnValue(
      snap({
        peers: s.peers.map((p) => ({ ...p, receives_you: true })),
        network: { ...s.network, radmin_ip: s.network.radmin_ip ?? '26.1.2.3' },
      }),
    )
    render(<DiagScreen />)
    expect(screen.getByText('Tudo certo pra transmitir')).toBeTruthy()
  })

  it('veredito_atencao', () => {
    mocked.mockReturnValue(
      snap({ peers: [{ name: 'Dudu', version: '3.0.0', ip: '26.57.210.9', receives_you: false }] }),
    )
    render(<DiagScreen />)
    expect(screen.getByText(/Quase lá · 1 aviso/)).toBeTruthy()
    expect(screen.getByText('não recebe')).toBeTruthy()
  })

  it('sem_rede', () => {
    mocked.mockReturnValue(snap({ network: { radmin_ip: null, interfaces: [] } }))
    render(<DiagScreen />)
    expect(screen.getByText('Sem rede')).toBeTruthy()
  })

  it('soon', () => {
    const { container } = render(<DiagScreen />)
    expect(screen.getAllByText(/em breve/).length).toBeGreaterThanOrEqual(2)
    expect(container.querySelectorAll('section[data-estado="soon"]').length).toBe(2)
  })

  it('verificando', () => {
    vi.useFakeTimers()
    render(<DiagScreen />)
    fireEvent.click(screen.getByText('Verificar de novo'))
    expect(screen.getByText('Verificando sua conexão…')).toBeTruthy()
    act(() => {
      vi.advanceTimersByTime(1600)
    })
    expect(screen.queryByText('Verificando sua conexão…')).toBeNull()
  })

  it('copiar', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined)
    vi.stubGlobal('navigator', { ...navigator, clipboard: { writeText } })
    render(<DiagScreen />)
    fireEvent.click(screen.getByText('Copiar relatório'))
    expect(await screen.findByText('Copiado!')).toBeTruthy()
    expect(writeText).toHaveBeenCalledWith(expect.stringContaining('## Rede'))
  })
})
