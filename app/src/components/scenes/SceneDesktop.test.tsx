// Tarefa previa-05
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render } from '@testing-library/react'
import { SceneDesktop } from './SceneDesktop'

afterEach(() => cleanup())

function renderDesktop() {
  const { container } = render(
    <svg>
      <SceneDesktop />
    </svg>,
  )
  return container
}

describe('SceneDesktop', () => {
  it('renderiza dentro de <svg> sem lançar', () => {
    expect(() => renderDesktop()).not.toThrow()
  })

  it('tem o fundo #14324A', () => {
    const container = renderDesktop()
    const fundo = container.querySelector('rect[fill="#14324A"]')
    expect(fundo).not.toBeNull()
    expect(fundo?.getAttribute('width')).toBe('160')
    expect(fundo?.getAttribute('height')).toBe('90')
  })

  it('tem os 3 quadradinhos da barra de tarefas com as cores certas', () => {
    const container = renderDesktop()
    const quadradinhos = [
      { x: '74', fill: '#3BF08A' },
      { x: '80', fill: '#5CD6FF' },
      { x: '86', fill: '#FFB547' },
    ]
    for (const { x, fill } of quadradinhos) {
      const rect = container.querySelector(`rect[x="${x}"][y="84"][fill="${fill}"]`)
      expect(rect).not.toBeNull()
      expect(rect?.getAttribute('width')).toBe('4')
      expect(rect?.getAttribute('height')).toBe('4')
      expect(rect?.getAttribute('rx')).toBe('1')
    }
    expect(container.querySelectorAll('rect[y="84"][width="4"][height="4"]').length).toBe(3)
  })

  it('a janela clara tem rx=3 e width=78', () => {
    const container = renderDesktop()
    const janela = container.querySelector('rect[fill="#E6F4EC"]')
    expect(janela).not.toBeNull()
    expect(janela?.getAttribute('rx')).toBe('3')
    expect(janela?.getAttribute('width')).toBe('78')
  })
})
