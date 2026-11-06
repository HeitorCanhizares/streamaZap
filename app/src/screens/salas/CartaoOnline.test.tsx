// Tarefa front-03
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { CartaoOnline } from './CartaoOnline'

afterEach(() => cleanup())

describe('CartaoOnline', () => {
  it('renderiza a frase completa', () => {
    render(<CartaoOnline />)
    const texto = screen.getByTestId('cartao-online').textContent ?? ''
    expect(texto).toContain('Salas pela internet, sem Radmin')
    expect(texto).toContain('ative o modo Online em Configurações')
  })

  it('sem onAbrirConfig não há botão', () => {
    render(<CartaoOnline />)
    expect(screen.queryByRole('button')).toBeNull()
  })

  it('com onAbrirConfig, o botão chama a função uma vez', () => {
    const onAbrirConfig = vi.fn()
    render(<CartaoOnline onAbrirConfig={onAbrirConfig} />)
    const botao = screen.getByRole('button', { name: /ative o modo Online/ })
    fireEvent.click(botao)
    expect(onAbrirConfig).toHaveBeenCalledTimes(1)
  })
})
