// Tarefa shell-02
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { ServerStatus } from './ServerStatus'

afterEach(() => cleanup())

describe('ServerStatus', () => {
  it('mostra ping e IP do Radmin quando ambos existem', () => {
    render(<ServerStatus serverMs={42} radminIp="26.12.34.56" />)
    expect(screen.getByText('Servidor online · 42 ms')).toBeTruthy()
    expect(screen.getByText('Radmin · 26.12.34.56')).toBeTruthy()
  })

  it('sem ping (undefined ou null) mostra "Servidor · não configurado"', () => {
    const { unmount } = render(<ServerStatus radminIp="1.2.3.4" />)
    expect(screen.getByText('Servidor · não configurado')).toBeTruthy()
    unmount()
    render(<ServerStatus serverMs={null} radminIp="1.2.3.4" />)
    expect(screen.getByText('Servidor · não configurado')).toBeTruthy()
  })

  it('sem IP (null) mostra "Radmin · não detectado"', () => {
    render(<ServerStatus serverMs={10} radminIp={null} />)
    expect(screen.getByText('Radmin · não detectado')).toBeTruthy()
  })

  it('arredonda o ping para inteiro', () => {
    render(<ServerStatus serverMs={41.6} radminIp={null} />)
    expect(screen.getByText('Servidor online · 42 ms')).toBeTruthy()
  })

  it('bolinhas são aria-hidden; a do servidor usa verde', () => {
    const { container } = render(<ServerStatus serverMs={5} radminIp="1.2.3.4" />)
    const bolinhas = container.querySelectorAll('[aria-hidden=true]')
    expect(bolinhas).toHaveLength(2)
    const primeiro = bolinhas[0] as HTMLElement
    expect(primeiro.style.background).toContain('var(--verde)')
  })
})
