// Tarefa telas-01
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { SeloPlano } from './SeloPlano'

afterEach(() => cleanup())

describe('SeloPlano', () => {
  it('fullhd sem cadeado mostra "Full HD" e nenhum svg', () => {
    const { container } = render(<SeloPlano plano="fullhd" />)
    expect(screen.getByTestId('selo-plano').textContent).toBe('Full HD')
    expect(container.querySelector('svg')).toBeNull()
  })

  it('hd com cadeado mostra "HD" e um svg aria-hidden', () => {
    const { container } = render(<SeloPlano plano="hd" cadeado />)
    expect(screen.getByTestId('selo-plano').textContent).toBe('HD')
    const svg = container.querySelector('svg')
    expect(svg).not.toBeNull()
    expect(svg?.getAttribute('aria-hidden')).toBe('true')
  })

  it('espectador mostra "Espectador"', () => {
    render(<SeloPlano plano="espectador" />)
    expect(screen.getByTestId('selo-plano').textContent).toBe('Espectador')
  })

  it('nunca mostra preço', () => {
    const casos: Array<[Parameters<typeof SeloPlano>[0]['plano'], boolean]> = [
      ['nenhum', false],
      ['espectador', true],
      ['hd', false],
      ['fullhd', true],
    ]
    for (const [plano, cadeado] of casos) {
      const { unmount } = render(<SeloPlano plano={plano} cadeado={cadeado} />)
      expect(screen.getByTestId('selo-plano').textContent).not.toContain('R$')
      unmount()
    }
  })
})
