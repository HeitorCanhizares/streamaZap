// Tarefa shell-06
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { NavRail } from '../NavRail'
import type { NavTarget } from './types'

afterEach(() => cleanup())

describe('NavRail', () => {
  it('fora da sala: mostra Salas, Criar sala, Diagnóstico e Configurações, sem Sala atual', () => {
    render(<NavRail active="salas" onNavigate={() => {}} />)
    expect(screen.getByRole('button', { name: 'Salas' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Criar sala' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Diagnóstico de rede' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Configurações' })).toBeTruthy()
    expect(screen.queryByRole('button', { name: 'Sala atual' })).toBeNull()
  })

  it('marca o item ativo com aria-current e zp-rail-active, e só ele', () => {
    const casos: { active: NavTarget; nome: string }[] = [
      { active: 'salas', nome: 'Salas' },
      { active: 'criar', nome: 'Criar sala' },
      { active: 'diag', nome: 'Diagnóstico de rede' },
      { active: 'config', nome: 'Configurações' },
    ]
    for (const { active, nome } of casos) {
      const { unmount } = render(<NavRail active={active} onNavigate={() => {}} />)
      const ativo = screen.getByRole('button', { name: nome })
      expect(ativo.getAttribute('aria-current')).toBe('page')
      expect(ativo.classList.contains('zp-rail-active')).toBe(true)
      for (const outro of screen.getAllByRole('button')) {
        if (outro === ativo) continue
        expect(outro.getAttribute('aria-current')).toBeNull()
        expect(outro.classList.contains('zp-rail-active')).toBe(false)
      }
      unmount()
    }
  })

  it('em sala: mostra Sala atual com indicador piscando, sem Criar sala', () => {
    const { container } = render(<NavRail active="sala" inRoom onNavigate={() => {}} />)
    expect(screen.getByRole('button', { name: 'Sala atual' })).toBeTruthy()
    expect(screen.queryByRole('button', { name: 'Criar sala' })).toBeNull()
    expect(container.querySelector('.zp-blink')).not.toBeNull()
  })

  it('em sala com active=sala: Sala atual tem aria-current page', () => {
    render(<NavRail active="sala" inRoom onNavigate={() => {}} />)
    const atual = screen.getByRole('button', { name: 'Sala atual' })
    expect(atual.getAttribute('aria-current')).toBe('page')
    expect(atual.classList.contains('zp-rail-active')).toBe(true)
  })

  it('em sala com active=config: Sala atual continua presente, sem aria-current', () => {
    render(<NavRail active="config" inRoom onNavigate={() => {}} />)
    const atual = screen.getByRole('button', { name: 'Sala atual' })
    expect(atual.getAttribute('aria-current')).toBeNull()
    expect(atual.classList.contains('zp-rail-active')).toBe(false)
  })

  it('cada botão chama onNavigate com o destino certo', () => {
    const onNavigate = vi.fn()
    render(<NavRail active="salas" inRoom onNavigate={onNavigate} />)

    fireEvent.click(screen.getByRole('button', { name: 'Salas' }))
    fireEvent.click(screen.getByRole('button', { name: 'Sala atual' }))
    fireEvent.click(screen.getByRole('button', { name: 'Diagnóstico de rede' }))
    fireEvent.click(screen.getByRole('button', { name: 'Configurações' }))
    expect(onNavigate.mock.calls.map((c) => c[0])).toEqual(['salas', 'sala', 'diag', 'config'])

    onNavigate.mockClear()
    cleanup()
    render(<NavRail active="salas" onNavigate={onNavigate} />)
    fireEvent.click(screen.getByRole('button', { name: 'Criar sala' }))
    expect(onNavigate).toHaveBeenCalledTimes(1)
    expect(onNavigate).toHaveBeenCalledWith('criar')
  })

  it('perfil: mostra a inicial em maiúscula e navega para config', () => {
    const onNavigate = vi.fn()
    render(<NavRail active="salas" onNavigate={onNavigate} profileName="heitor" />)
    const perfil = screen.getByRole('button', { name: 'Seu perfil: heitor' })
    expect(perfil.textContent).toBe('H')
    fireEvent.click(perfil)
    expect(onNavigate).toHaveBeenCalledWith('config')
  })

  it('perfil vazio: mostra ? e aria-label sem nome', () => {
    render(<NavRail active="salas" onNavigate={() => {}} />)
    const perfil = screen.getByRole('button', { name: 'Seu perfil:' })
    expect(perfil.textContent).toBe('?')
  })

  it('logo: img com alt streamaZap e src /icone.png; nav com aria-label', () => {
    const { container } = render(<NavRail active="salas" onNavigate={() => {}} />)
    const img = screen.getByAltText('streamaZap')
    expect(img.getAttribute('src')).toBe('/icone.png')
    expect(img.tagName).toBe('IMG')
    const nav = screen.getByRole('navigation', { name: 'Navegação principal' })
    expect(nav.classList.contains('zp-navrail')).toBe(true)
    expect(container.querySelector('nav')).toBe(nav)
  })
})
