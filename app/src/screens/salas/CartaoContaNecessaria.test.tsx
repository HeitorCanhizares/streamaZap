// Tarefa front-15
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { CartaoContaNecessaria } from './CartaoContaNecessaria'

vi.mock('../config/ContaEntrar', () => ({
  ContaEntrar: () => <div data-testid="conta-entrar" />,
}))

afterEach(() => cleanup())

describe('CartaoContaNecessaria', () => {
  it('mostra o aviso para entrar com a conta', () => {
    render(<CartaoContaNecessaria />)
    expect(screen.getByText('Entre com sua conta para usar o Online')).toBeTruthy()
  })

  it('tem o data-testid do cartão', () => {
    render(<CartaoContaNecessaria />)
    expect(screen.getByTestId('cartao-conta-necessaria')).toBeTruthy()
  })

  it('renderiza o ContaEntrar dentro do cartão', () => {
    render(<CartaoContaNecessaria />)
    const cartao = screen.getByTestId('cartao-conta-necessaria')
    expect(cartao.querySelector('[data-testid="conta-entrar"]')).not.toBeNull()
  })

  it('não mostra preço, Assinar nem plano', () => {
    render(<CartaoContaNecessaria />)
    const texto = screen.getByTestId('cartao-conta-necessaria').textContent ?? ''
    expect(texto).not.toContain('Assinar')
    expect(texto).not.toContain('R$')
  })
})
