// Tarefa ui-03
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { Segmented } from './Segmented'

afterEach(() => cleanup())

describe('Segmented', () => {
  const opcoes = [
    { value: 'a', label: 'Alfa' },
    { value: 'b', label: 'Beta', disabled: true },
    { value: 'c', label: 'Gama' },
  ]

  it('chama onChange com o valor clicado', () => {
    const onChange = vi.fn()
    render(<Segmented options={opcoes} value="a" onChange={onChange} />)
    fireEvent.click(screen.getByRole('button', { name: 'Gama' }))
    expect(onChange).toHaveBeenCalledWith('c')
  })

  it('ignora opção desabilitada', () => {
    const onChange = vi.fn()
    render(<Segmented options={opcoes} value="a" onChange={onChange} />)
    fireEvent.click(screen.getByRole('button', { name: 'Beta' }))
    expect(onChange).not.toHaveBeenCalled()
  })

  it('marca zp-seg-active só na opção igual ao value', () => {
    render(<Segmented options={opcoes} value="c" onChange={() => {}} />)
    expect(screen.getByRole('button', { name: 'Gama' }).className).toContain('zp-seg-active')
    expect(screen.getByRole('button', { name: 'Alfa' }).className).not.toContain('zp-seg-active')
    expect(screen.getByRole('button', { name: 'Gama' }).className).toContain('zp-seg-item')
  })
})
