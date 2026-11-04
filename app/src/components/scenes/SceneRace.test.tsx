// Tarefa previa-02
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { SceneRace } from './SceneRace'

afterEach(() => cleanup())

function renderRace() {
  const { container } = render(
    <svg>
      <SceneRace />
    </svg>,
  )
  return container
}

describe('SceneRace', () => {
  it('mostra a velocidade 212 e a unidade KM/H', () => {
    const container = renderRace()
    expect(screen.getByText('212')).toBeTruthy()
    expect(screen.getByText('KM/H')).toBeTruthy()
    const textos = Array.from(container.querySelectorAll('text')).map((t) => t.textContent)
    expect(textos).toEqual(['212', 'KM/H'])
  })

  it('alinha o velocímetro pela direita e usa peso 600 no número', () => {
    const container = renderRace()
    const numero = container.querySelector('text[font-size="11"]')
    expect(numero?.getAttribute('text-anchor')).toBe('end')
    expect(numero?.getAttribute('font-weight')).toBe('600')
    expect(numero?.getAttribute('x')).toBe('152')
    expect(numero?.getAttribute('y')).toBe('82')
  })

  it('tem o céu #2A2140 cobrindo 160x90', () => {
    const container = renderRace()
    const fundo = container.querySelector('rect[fill="#2A2140"]')
    expect(fundo).not.toBeNull()
    expect(fundo?.getAttribute('width')).toBe('160')
    expect(fundo?.getAttribute('height')).toBe('90')
  })

  it('pinta as faixas vermelhas com opacity 0.7', () => {
    const container = renderRace()
    const faixas = container.querySelectorAll('polygon[fill="#FF5D6E"]')
    expect(faixas.length).toBe(2)
    faixas.forEach((f) => expect(f.getAttribute('opacity')).toBe('0.7'))
  })

  it('desenha o carro com rect fill #5CD6FF rx=3', () => {
    const container = renderRace()
    const carro = container.querySelector('rect[fill="#5CD6FF"]')
    expect(carro).not.toBeNull()
    expect(carro?.getAttribute('rx')).toBe('3')
    expect(carro?.getAttribute('x')).toBe('64')
    expect(carro?.getAttribute('y')).toBe('64')
    expect(carro?.getAttribute('width')).toBe('32')
    expect(carro?.getAttribute('height')).toBe('14')
  })
})
