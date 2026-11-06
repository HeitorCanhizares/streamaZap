// Tarefa front-08
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { CriarModo, type CriarModoProps } from './CriarModo'
import { resetOnlineEnabled, setOnlineEnabled } from '../../bridge/onlineStore'

beforeEach(() => setOnlineEnabled(false))
afterEach(() => {
  cleanup()
  resetOnlineEnabled()
})

function props(over: Partial<CriarModoProps> = {}): CriarModoProps {
  return { modo: 'local', onModo: vi.fn(), ...over }
}

describe('CriarModo com Online desligado', () => {
  it('mostra só o cartão Local', () => {
    render(<CriarModo {...props()} />)
    const radios = screen.getAllByRole('radio')
    expect(radios).toHaveLength(1)
    expect(radios[0].getAttribute('aria-label') ?? radios[0].textContent).toMatch(/Local/)
  })

  it('não mostra nenhum texto "online" nem "servidor"', () => {
    render(<CriarModo {...props()} />)
    expect(screen.queryByText(/online/i)).toBeNull()
    expect(screen.queryByText(/servidor/i)).toBeNull()
  })

  it('mantém o título da seção', () => {
    render(<CriarModo {...props()} />)
    expect(screen.getByText('Onde a sala vai rodar')).toBeTruthy()
  })

  it('com Online ligado mostra os dois cartões', () => {
    setOnlineEnabled(true)
    render(<CriarModo {...props({ modo: 'online' })} />)
    const radios = screen.getAllByRole('radio')
    expect(radios).toHaveLength(2)
    expect(screen.getByRole('radio', { name: /Online/ })).toBeTruthy()
    expect(screen.getByRole('radio', { name: /Local/ })).toBeTruthy()
  })
})
