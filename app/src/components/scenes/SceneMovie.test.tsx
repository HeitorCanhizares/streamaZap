// Tarefa previa-03
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { SceneMovie } from './SceneMovie'

afterEach(() => cleanup())

function renderMovie() {
  const { container } = render(
    <svg>
      <SceneMovie />
    </svg>,
  )
  return container
}

describe('SceneMovie', () => {
  it('mostra a legenda centralizada', () => {
    const container = renderMovie()
    const texto = container.querySelector('text')
    expect(texto?.textContent).toBe('— Você viu aquilo lá em cima?')
    expect(texto?.getAttribute('text-anchor')).toBe('middle')
    expect(screen.getByText('— Você viu aquilo lá em cima?')).toBeTruthy()
  })

  it('tem 2 barras pretas (topo e base)', () => {
    const container = renderMovie()
    const pretas = container.querySelectorAll('rect[fill="#000000"]')
    expect(pretas.length).toBe(2)
    const alturas = Array.from(pretas).map((r) => r.getAttribute('height'))
    expect(alturas).toContain('11')
    expect(alturas).toContain('12')
  })

  it('desenha 8 prédios dentro do grupo #0B1828', () => {
    const container = renderMovie()
    const grupo = container.querySelector('g[fill="#0B1828"]')
    expect(grupo).not.toBeNull()
    expect(grupo?.querySelectorAll('rect').length).toBe(8)
  })
})
