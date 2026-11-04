// Tarefa ui-02
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { Button } from './Button'
import { Card } from './Card'
import { Toggle } from './Toggle'

afterEach(() => cleanup())

describe('Button', () => {
  it('dispara onClick ao clicar', () => {
    const onClick = vi.fn()
    render(<Button onClick={onClick}>Ok</Button>)
    fireEvent.click(screen.getByRole('button', { name: 'Ok' }))
    expect(onClick).toHaveBeenCalledTimes(1)
  })

  it('não dispara onClick quando disabled', () => {
    const onClick = vi.fn()
    render(<Button disabled onClick={onClick}>Ok</Button>)
    fireEvent.click(screen.getByRole('button', { name: 'Ok' }))
    expect(onClick).not.toHaveBeenCalled()
  })

  it('variant danger usa a classe zp-btn-danger', () => {
    render(<Button variant="danger">Sair</Button>)
    const btn = screen.getByRole('button', { name: 'Sair' })
    expect(btn.className).toContain('zp-btn-danger')
    expect(btn.className).toContain('zp-btn')
  })

  it('usa primary por padrão e tem type="button"', () => {
    render(<Button>Ok</Button>)
    const btn = screen.getByRole('button', { name: 'Ok' })
    expect(btn.className).toContain('zp-btn-primary')
    expect(btn.getAttribute('type')).toBe('button')
  })
})

describe('Toggle', () => {
  it('chama onChange(true) ao clicar quando checked=false', () => {
    const onChange = vi.fn()
    render(<Toggle checked={false} onChange={onChange} />)
    fireEvent.click(screen.getByRole('switch'))
    expect(onChange).toHaveBeenCalledWith(true)
  })

  it('aria-checked reflete checked', () => {
    const { rerender } = render(<Toggle checked={false} onChange={() => {}} />)
    expect(screen.getByRole('switch').getAttribute('aria-checked')).toBe('false')
    rerender(<Toggle checked={true} onChange={() => {}} />)
    expect(screen.getByRole('switch').getAttribute('aria-checked')).toBe('true')
    expect(screen.getByRole('switch').className).toContain('zp-toggle-on')
  })

  it('com label, o texto fica dentro de um <label> e clicar nele alterna', () => {
    const onChange = vi.fn()
    render(<Toggle checked={false} onChange={onChange} label="Câmera" />)
    expect(screen.getByText('Câmera').closest('label')).not.toBeNull()
    fireEvent.click(screen.getByRole('switch'))
    expect(onChange).toHaveBeenCalledWith(true)
  })
})

describe('Card', () => {
  it('renderiza children', () => {
    render(<Card className="extra"><p>conteúdo</p></Card>)
    expect(screen.getByText('conteúdo')).toBeTruthy()
    expect(screen.getByText('conteúdo').closest('.zp-card')?.className).toContain('extra')
  })
})
