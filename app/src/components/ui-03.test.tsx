// Tarefa ui-03
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { Segmented } from './Segmented'
import { TitleBar } from './TitleBar'
import { NavRail } from './NavRail'

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

describe('TitleBar', () => {
  it('mostra streamaZap por padrão e tem região de arrasto', () => {
    const { container } = render(<TitleBar />)
    expect(screen.getByText('streamaZap')).toBeTruthy()
    const header = container.querySelector('header')
    expect(header?.hasAttribute('data-tauri-drag-region')).toBe(true)
  })

  it('botões de janela ficam fora da região de arrasto', () => {
    render(<TitleBar />)
    expect(screen.getByRole('button', { name: 'Fechar' }).hasAttribute('data-tauri-drag-region')).toBe(false)
  })

  it('botão Fechar chama onClose', () => {
    const onClose = vi.fn()
    render(<TitleBar onClose={onClose} />)
    fireEvent.click(screen.getByRole('button', { name: 'Fechar' }))
    expect(onClose).toHaveBeenCalledTimes(1)
  })
})

describe('NavRail', () => {
  it('marca aria-current no item ativo', () => {
    render(<NavRail active="salas" onNavigate={() => {}} />)
    expect(screen.getByRole('button', { name: 'Salas' }).getAttribute('aria-current')).toBe('page')
    expect(screen.getByRole('button', { name: 'Configuracoes' }).getAttribute('aria-current')).toBeNull()
  })

  it('clique em Configuracoes chama onNavigate("config")', () => {
    const onNavigate = vi.fn()
    render(<NavRail active="salas" onNavigate={onNavigate} />)
    fireEvent.click(screen.getByRole('button', { name: 'Configuracoes' }))
    expect(onNavigate).toHaveBeenCalledWith('config')
  })
})
