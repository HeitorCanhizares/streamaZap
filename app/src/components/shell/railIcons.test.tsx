// Tarefa shell-04
import type { ReactNode } from 'react'
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { IconConfig, IconCriar, IconDiag, IconSalaAtual, IconSalas } from './railIcons'

afterEach(() => cleanup())

describe('ícones do trilho', () => {
  it('cada ícone renderiza svg 22x22 com viewBox 0 0 24 24', () => {
    const icons = [IconSalas, IconCriar, IconDiag, IconConfig, IconSalaAtual]
    for (const Icon of icons) {
      const { container, unmount } = render(<Icon />)
      const svg = container.querySelector('svg')
      expect(svg).not.toBeNull()
      expect(svg!.getAttribute('width')).toBe('22')
      expect(svg!.getAttribute('height')).toBe('22')
      expect(svg!.getAttribute('viewBox')).toBe('0 0 24 24')
      unmount()
    }
  })

  it('paths de IconSalas, IconCriar, IconDiag e IconConfig batem com o design', () => {
    const cases: Array<[() => ReactNode, string]> = [
      [IconSalas, 'M21 12a8 8 0 0 1-11.6 7.1L4 20l1-4.6A8 8 0 1 1 21 12z'],
      [IconCriar, 'M12 5v14M5 12h14'],
      [IconDiag, 'M3 12h4l3-8 4 16 3-8h4'],
      [IconConfig, 'M4 7h9m4 0h3M4 17h3m4 0h9'],
    ]
    for (const [Icon, d] of cases) {
      const { container, unmount } = render(<Icon />)
      expect(container.querySelector('path')!.getAttribute('d')).toBe(d)
      unmount()
    }
  })

  it('IconSalaAtual: primeiro path é o pé do monitor e há um rect', () => {
    const { container } = render(<IconSalaAtual />)
    expect(container.querySelector('path')!.getAttribute('d')).toBe('M8 21h8M12 17v4')
    expect(container.querySelector('rect')).not.toBeNull()
  })

  it('IconSalaAtual tem role img e aria-label "Sala atual"', () => {
    const { container } = render(<IconSalaAtual />)
    const svg = container.querySelector('svg')!
    expect(svg.getAttribute('role')).toBe('img')
    expect(svg.getAttribute('aria-label')).toBe('Sala atual')
    expect(screen.getByRole('img', { name: 'Sala atual' })).toBeTruthy()
  })

  it('os outros 4 ícones são aria-hidden="true"', () => {
    const icons = [IconSalas, IconCriar, IconDiag, IconConfig]
    for (const Icon of icons) {
      const { container, unmount } = render(<Icon />)
      expect(container.querySelector('svg')!.getAttribute('aria-hidden')).toBe('true')
      unmount()
    }
  })

  it('IconConfig tem 2 circles', () => {
    const { container } = render(<IconConfig />)
    expect(container.querySelectorAll('circle')).toHaveLength(2)
  })
})
