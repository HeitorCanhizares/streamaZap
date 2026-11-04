// Tarefa ui-04
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { Avatar } from './Avatar'
import { Badge } from './Badge'
import { Ping } from './Ping'
import { Slider } from './Slider'

afterEach(() => cleanup())

describe('Avatar', () => {
  it('mostra a inicial maiúscula do nome', () => {
    render(<Avatar name="marcao" color="#3bf08a" />)
    expect(screen.getByText('M')).toBeTruthy()
  })

  it('mostra ? quando o nome está vazio', () => {
    render(<Avatar name="" color="#3bf08a" />)
    expect(screen.getByText('?')).toBeTruthy()
  })

  it('ignora espaços iniciais e usa size padrão 32', () => {
    const { container } = render(<Avatar name="  ana" color="#fff" />)
    const el = container.querySelector('.zp-avatar') as HTMLElement
    expect(el.textContent).toBe('A')
    expect(el.style.width).toBe('32px')
    expect(el.style.height).toBe('32px')
  })
})

describe('Badge', () => {
  it('aplica a classe do tom informado', () => {
    const { container } = render(<Badge tone="red">ao vivo</Badge>)
    const el = container.querySelector('span') as HTMLElement
    expect(el.className).toContain('zp-badge')
    expect(el.className).toContain('zp-badge-red')
    expect(el.textContent).toBe('ao vivo')
  })

  it('usa neutral por padrão', () => {
    const { container } = render(<Badge>x</Badge>)
    expect(container.querySelector('span')?.className).toContain('zp-badge-neutral')
  })
})

describe('Ping', () => {
  it('45 ms é verde', () => {
    const { container } = render(<Ping ms={45} />)
    const el = container.querySelector('span') as HTMLElement
    expect(el.getAttribute('data-tone')).toBe('green')
    expect(el.textContent).toBe('45 ms')
  })

  it('80 ms é amarelo', () => {
    const { container } = render(<Ping ms={80} />)
    expect(container.querySelector('span')?.getAttribute('data-tone')).toBe('yellow')
  })

  it('150 ms é vermelho', () => {
    const { container } = render(<Ping ms={150} />)
    expect(container.querySelector('span')?.getAttribute('data-tone')).toBe('red')
  })
})

describe('Slider', () => {
  it('dispara onChange com número', () => {
    const onChange = vi.fn()
    const { container } = render(<Slider value={0.5} onChange={onChange} />)
    const input = container.querySelector('input') as HTMLInputElement
    expect(input.type).toBe('range')
    expect(input.className).toContain('zp-slider')
    fireEvent.change(input, { target: { value: '0.75' } })
    expect(onChange).toHaveBeenCalledWith(0.75)
  })
})
