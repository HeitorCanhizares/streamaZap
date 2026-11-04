// Tarefa previa-04
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { SceneCode } from './SceneCode'

afterEach(() => cleanup())

function renderCode() {
  const { container } = render(
    <svg>
      <SceneCode />
    </svg>,
  )
  return container
}

describe('SceneCode', () => {
  it('mostra o comando $ python main.py', () => {
    const container = renderCode()
    const texto = container.querySelector('text')
    expect(texto?.textContent).toBe('$ python main.py')
    expect(screen.getByText('$ python main.py')).toBeTruthy()
  })

  it('tem o fundo #0F1621', () => {
    const container = renderCode()
    const fundo = container.querySelector('rect[fill="#0F1621"]')
    expect(fundo).not.toBeNull()
    expect(fundo?.getAttribute('width')).toBe('160')
    expect(fundo?.getAttribute('height')).toBe('90')
  })

  it('usa as cores de sintaxe do design (roxo e rosa)', () => {
    const container = renderCode()
    expect(container.querySelector('rect[fill="#B79CFF"]')).not.toBeNull()
    expect(container.querySelector('rect[fill="#FF8FB1"]')).not.toBeNull()
  })

  it('não usa mais o amarelo antigo #FFE37A', () => {
    const container = renderCode()
    expect(container.querySelector('[fill="#FFE37A"]')).toBeNull()
  })

  it('desenha 13 linhas de código (height 2.4), como no design', () => {
    const container = renderCode()
    const linhas = Array.from(container.querySelectorAll('rect')).filter(
      (r) => r.getAttribute('height') === '2.4',
    )
    expect(linhas.length).toBe(13)
  })
})
