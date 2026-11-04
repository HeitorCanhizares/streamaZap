// Tarefa shell-01
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { WindowControls } from './WindowControls'

afterEach(() => cleanup())

describe('WindowControls', () => {
  it('renderiza tres botões: Minimizar, Maximizar e Fechar', () => {
    render(<WindowControls />)
    expect(screen.getByRole('button', { name: 'Minimizar' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Maximizar' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Fechar' })).toBeTruthy()
    expect(screen.getAllByRole('button')).toHaveLength(3)
  })

  it('cada clique chama só o seu callback, uma vez', () => {
    const onMinimize = vi.fn()
    const onMaximize = vi.fn()
    const onClose = vi.fn()
    render(<WindowControls onMinimize={onMinimize} onMaximize={onMaximize} onClose={onClose} />)

    fireEvent.click(screen.getByRole('button', { name: 'Minimizar' }))
    expect(onMinimize).toHaveBeenCalledTimes(1)
    expect(onMaximize).not.toHaveBeenCalled()
    expect(onClose).not.toHaveBeenCalled()

    fireEvent.click(screen.getByRole('button', { name: 'Maximizar' }))
    expect(onMaximize).toHaveBeenCalledTimes(1)
    expect(onMinimize).toHaveBeenCalledTimes(1)
    expect(onClose).not.toHaveBeenCalled()

    fireEvent.click(screen.getByRole('button', { name: 'Fechar' }))
    expect(onClose).toHaveBeenCalledTimes(1)
    expect(onMinimize).toHaveBeenCalledTimes(1)
    expect(onMaximize).toHaveBeenCalledTimes(1)
  })

  it('clicar sem callbacks não lança', () => {
    render(<WindowControls />)
    expect(() => {
      fireEvent.click(screen.getByRole('button', { name: 'Minimizar' }))
      fireEvent.click(screen.getByRole('button', { name: 'Maximizar' }))
      fireEvent.click(screen.getByRole('button', { name: 'Fechar' }))
    }).not.toThrow()
  })

  it('Fechar usa a classe zp-close; os outros usam zp-win', () => {
    render(<WindowControls />)
    expect(screen.getByRole('button', { name: 'Fechar' }).className).toBe('zp-close')
    expect(screen.getByRole('button', { name: 'Minimizar' }).className).toBe('zp-win')
    expect(screen.getByRole('button', { name: 'Maximizar' }).className).toBe('zp-win')
  })

  it('nenhum botão tem data-tauri-drag-region', () => {
    const { container } = render(<WindowControls />)
    expect(container.querySelectorAll('[data-tauri-drag-region]')).toHaveLength(0)
  })

  it('o svg de Fechar contém o path do X', () => {
    render(<WindowControls />)
    const svg = screen.getByRole('button', { name: 'Fechar' }).querySelector('svg')
    expect(svg).not.toBeNull()
    const paths = Array.from(svg!.querySelectorAll('path')).map((p) => p.getAttribute('d'))
    expect(paths).toContain('M6 6l12 12M18 6 6 18')
  })
})
