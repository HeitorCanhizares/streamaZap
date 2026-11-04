// Tarefa salas-06
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { FirewallAviso } from './FirewallAviso'

afterEach(() => cleanup())

describe('FirewallAviso', () => {
  it('tem role alert', () => {
    render(<FirewallAviso onCorrigir={() => {}} />)
    expect(screen.getByRole('alert')).toBeTruthy()
  })

  it('contém o texto exato do aviso', () => {
    render(<FirewallAviso onCorrigir={() => {}} />)
    expect(
      screen.getByText('O Firewall do Windows está bloqueando quem tenta entrar nas suas salas.'),
    ).toBeTruthy()
  })

  it('clicar em "Corrigir firewall" chama onCorrigir uma vez', () => {
    const onCorrigir = vi.fn()
    render(<FirewallAviso onCorrigir={onCorrigir} />)
    fireEvent.click(screen.getByRole('button', { name: 'Corrigir firewall' }))
    expect(onCorrigir).toHaveBeenCalledTimes(1)
  })
})
