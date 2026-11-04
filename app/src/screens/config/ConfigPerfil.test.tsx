// Tarefa config-01
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { ConfigPerfil } from './ConfigPerfil'
import { mockSettings } from '../../bridge/mock'

afterEach(() => cleanup())

describe('ConfigPerfil', () => {
  it('renderiza o título e o nome em um input', () => {
    const s = { ...mockSettings(), name: 'marcao' }
    render(<ConfigPerfil settings={s} onChange={vi.fn()} />)
    expect(screen.getByRole('heading', { name: 'Perfil' })).toBeTruthy()
    expect(screen.getByDisplayValue('marcao')).toBeTruthy()
  })

  it('avatar mostra a inicial maiúscula e ? com nome vazio', () => {
    const { rerender } = render(<ConfigPerfil settings={{ ...mockSettings(), name: 'marcao' }} onChange={vi.fn()} />)
    expect(screen.getByTestId('perfil-avatar').textContent).toBe('M')
    rerender(<ConfigPerfil settings={{ ...mockSettings(), name: '' }} onChange={vi.fn()} />)
    expect(screen.getByTestId('perfil-avatar').textContent).toBe('?')
  })

  it('trocar o nome chama onChange({name})', () => {
    const onChange = vi.fn()
    render(<ConfigPerfil settings={{ ...mockSettings(), name: 'ana' }} onChange={onChange} />)
    fireEvent.change(screen.getByDisplayValue('ana'), { target: { value: 'X' } })
    expect(onChange).toHaveBeenCalledWith({ name: 'X' })
  })

  it('6 botões de cor e clicar em Ciano chama onChange({color})', () => {
    const onChange = vi.fn()
    render(<ConfigPerfil settings={mockSettings()} onChange={onChange} />)
    for (const n of ['Verde zap', 'Ciano', 'Âmbar', 'Rosa', 'Lilás', 'Laranja']) {
      expect(screen.getByRole('radio', { name: n })).toBeTruthy()
    }
    expect(screen.getAllByRole('radio')).toHaveLength(6)
    fireEvent.click(screen.getByRole('radio', { name: 'Ciano' }))
    expect(onChange).toHaveBeenCalledWith({ color: '#5CD6FF' })
  })

  it('a cor selecionada tem aria-checked true', () => {
    render(<ConfigPerfil settings={{ ...mockSettings(), color: '#FFB547' }} onChange={vi.fn()} />)
    expect(screen.getByRole('radio', { name: 'Âmbar' }).getAttribute('aria-checked')).toBe('true')
    expect(screen.getByRole('radio', { name: 'Verde zap' }).getAttribute('aria-checked')).toBe('false')
  })

  it('não renderiza Sobre e atualizações', () => {
    render(<ConfigPerfil settings={mockSettings()} onChange={vi.fn()} />)
    expect(screen.queryByText('Sobre e atualizações')).toBeNull()
  })
})
