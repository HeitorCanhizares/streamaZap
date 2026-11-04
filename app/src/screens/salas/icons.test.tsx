// Tarefa salas-01
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render } from '@testing-library/react'
import type { ReactNode } from 'react'
import {
  IconeAviso,
  IconeCadeado,
  IconeCadeadoAberto,
  IconeChave,
  IconeEscudo,
  IconeGlobo,
  IconeLupa,
  IconeMais,
  IconeOlho,
  IconeSeta,
  IconeWifi,
} from './icons'

afterEach(() => cleanup())

const todos: [string, () => ReactNode][] = [
  ['IconeGlobo', () => <IconeGlobo />],
  ['IconeWifi', () => <IconeWifi />],
  ['IconeMais', () => <IconeMais />],
  ['IconeLupa', () => <IconeLupa />],
  ['IconeCadeado', () => <IconeCadeado />],
  ['IconeCadeadoAberto', () => <IconeCadeadoAberto />],
  ['IconeOlho', () => <IconeOlho />],
  ['IconeEscudo', () => <IconeEscudo />],
  ['IconeAviso', () => <IconeAviso />],
  ['IconeSeta', () => <IconeSeta />],
  ['IconeChave', () => <IconeChave />],
]

function svgDe(el: ReactNode): SVGSVGElement {
  const { container, unmount } = render(<>{el}</>)
  const svg = container.querySelector('svg')
  if (!svg) throw new Error('sem <svg>')
  unmount()
  return svg as SVGSVGElement
}

describe('ícones da tela Salas', () => {
  it.each(todos)('%s renderiza um <svg> com viewBox 0 0 24 24', (_nome, make) => {
    const svg = svgDe(make())
    expect(svg.tagName.toLowerCase()).toBe('svg')
    expect(svg.getAttribute('viewBox')).toBe('0 0 24 24')
  })

  const comSize: [string, (size: number) => ReactNode][] = [
    ['IconeGlobo', (size) => <IconeGlobo size={size} />],
    ['IconeWifi', (size) => <IconeWifi size={size} />],
    ['IconeMais', (size) => <IconeMais size={size} />],
    ['IconeLupa', (size) => <IconeLupa size={size} />],
    ['IconeCadeado', (size) => <IconeCadeado size={size} />],
    ['IconeCadeadoAberto', (size) => <IconeCadeadoAberto size={size} />],
    ['IconeOlho', (size) => <IconeOlho size={size} />],
    ['IconeEscudo', (size) => <IconeEscudo size={size} />],
    ['IconeAviso', (size) => <IconeAviso size={size} />],
    ['IconeSeta', (size) => <IconeSeta size={size} />],
    ['IconeChave', (size) => <IconeChave size={size} />],
  ]

  it.each(comSize)('%s com size=30 gera width/height "30"', (_nome, make) => {
    const { container } = render(<>{make(30)}</>)
    const svg = container.querySelector('svg')!
    expect(svg.getAttribute('width')).toBe('30')
    expect(svg.getAttribute('height')).toBe('30')
  })

  it('cor="#FF0000" vira stroke do svg', () => {
    const { container } = render(<IconeLupa cor="#FF0000" />)
    expect(container.querySelector('svg')!.getAttribute('stroke')).toBe('#FF0000')
  })

  it('sem cor, o stroke padrão é currentColor', () => {
    const { container } = render(<IconeSeta />)
    expect(container.querySelector('svg')!.getAttribute('stroke')).toBe('currentColor')
  })

  it('IconeCadeado tem aria-label "com senha" e role img', () => {
    const { container } = render(<IconeCadeado />)
    const svg = container.querySelector('svg')!
    expect(svg.getAttribute('aria-label')).toBe('com senha')
    expect(svg.getAttribute('role')).toBe('img')
    expect(svg.getAttribute('aria-hidden')).toBeNull()
  })

  it.each(todos.filter(([n]) => n !== 'IconeCadeado'))('%s é aria-hidden="true"', (_nome, make) => {
    const { container } = render(<>{make()}</>)
    const svg = container.querySelector('svg')!
    expect(svg.getAttribute('aria-hidden')).toBe('true')
    cleanup()
  })
})
