// Tarefa previa-01
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { SceneGame } from './SceneGame'

afterEach(() => cleanup())

function renderGame() {
  const { container } = render(
    <svg>
      <SceneGame />
    </svg>,
  )
  return container
}

describe('SceneGame', () => {
  it('mostra o contador 24/90', () => {
    const container = renderGame()
    const texto = container.querySelector('text')
    expect(texto?.textContent).toBe('24/90')
    expect(screen.getByText('24/90')).toBeTruthy()
  })

  it('tem o fundo #1B2A3A', () => {
    const container = renderGame()
    const fundo = container.querySelector('rect[fill="#1B2A3A"]')
    expect(fundo).not.toBeNull()
    expect(fundo?.getAttribute('width')).toBe('160')
    expect(fundo?.getAttribute('height')).toBe('90')
  })

  it('desenha 4 linhas da cruz de mira', () => {
    const container = renderGame()
    expect(container.querySelectorAll('line').length).toBe(4)
  })

  it('desenha 2 círculos com r=1.3 e r=0.9', () => {
    const container = renderGame()
    const circulos = container.querySelectorAll('circle')
    // o anel externo (r=8) também é circle; conferimos os dois pontos pedidos
    const raios = Array.from(circulos).map((c) => c.getAttribute('r'))
    expect(raios).toContain('1.3')
    expect(raios).toContain('0.9')
    expect(container.querySelector('circle[r="1.3"]')).not.toBeNull()
    expect(container.querySelector('circle[r="0.9"]')).not.toBeNull()
  })

  it('desenha o polígono do carro com os pontos do design', () => {
    const container = renderGame()
    const carro = container.querySelector('polygon[fill="#0E1820"]')
    expect(carro?.getAttribute('points')).toBe('108,90 120,70 138,64 152,70 148,90')
  })
})
