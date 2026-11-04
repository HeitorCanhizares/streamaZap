// Tarefa criar-02
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { CriarModo, type CriarModoProps } from './CriarModo'

afterEach(() => cleanup())

function props(over: Partial<CriarModoProps> = {}): CriarModoProps {
  return { modo: 'online', onModo: vi.fn(), ...over }
}

describe('CriarModo', () => {
  it('titulo: mostra "01" e "Onde a sala vai rodar"', () => {
    render(<CriarModo {...props()} />)
    expect(screen.getByText('01')).toBeTruthy()
    expect(screen.getByText('Onde a sala vai rodar')).toBeTruthy()
  })

  it('radios: renderiza Online e Local com hint e tech', () => {
    render(<CriarModo {...props()} />)
    expect(screen.getByRole('radio', { name: /Online/ })).toBeTruthy()
    expect(screen.getByRole('radio', { name: /Local/ })).toBeTruthy()
    expect(screen.getByText('servidor sa-east · 42 ms')).toBeTruthy()
    expect(screen.getByText('P2P · porta 47800')).toBeTruthy()
  })

  it('aria-checked reflete o modo atual', () => {
    const { rerender } = render(<CriarModo {...props({ modo: 'online' })} />)
    expect(screen.getByRole('radio', { name: /Online/ }).getAttribute('aria-checked')).toBe('true')
    expect(screen.getByRole('radio', { name: /Local/ }).getAttribute('aria-checked')).toBe('false')
    rerender(<CriarModo {...props({ modo: 'local' })} />)
    expect(screen.getByRole('radio', { name: /Online/ }).getAttribute('aria-checked')).toBe('false')
    expect(screen.getByRole('radio', { name: /Local/ }).getAttribute('aria-checked')).toBe('true')
  })

  it('clique no radio nao ativo chama onModo com o valor', () => {
    const onModo = vi.fn()
    render(<CriarModo {...props({ modo: 'online', onModo })} />)
    fireEvent.click(screen.getByRole('radio', { name: /Local/ }))
    expect(onModo).toHaveBeenLastCalledWith('local')
    fireEvent.click(screen.getByRole('radio', { name: /Online/ }))
    expect(onModo).toHaveBeenLastCalledWith('online')
  })
})
