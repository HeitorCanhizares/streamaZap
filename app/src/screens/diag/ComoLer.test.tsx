// Tarefa diag-05
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { ComoLer } from './ComoLer'

afterEach(() => {
  cleanup()
})

describe('ComoLer', () => {
  it('titulo', () => {
    render(<ComoLer />)
    expect(screen.getByRole('heading', { name: 'Como ler isso' })).toBeTruthy()
  })

  it('tres_paragrafos', () => {
    const { container } = render(<ComoLer />)
    expect(container.querySelectorAll('p').length).toBe(3)
  })

  it('negritos', () => {
    const { container } = render(<ComoLer />)
    const textos = Array.from(container.querySelectorAll('b')).map((b) => b.textContent)
    expect(textos).toEqual(['tempo esgotado', 'dele', 'Corrigir firewall', 'Online'])
  })

  it('textos_exatos', () => {
    const { container } = render(<ComoLer />)
    const segundo = container.querySelectorAll('p')[1] as HTMLElement
    expect(segundo.textContent).toContain('desligue o Firewall do Windows por 1 minuto')
  })

  it('icone', () => {
    const { container } = render(<ComoLer />)
    const svg = container.querySelector('h2 svg') as SVGElement
    expect(svg).toBeTruthy()
    expect(svg.querySelector('circle')).toBeTruthy()
    expect(svg.getAttribute('aria-hidden')).toBe('true')
  })
})
