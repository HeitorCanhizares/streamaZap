// Tarefa ui-16
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { mockSources } from '../../bridge/mock'
import { CriarOpcoes, type CriarOpcoesProps } from './CriarOpcoes'

afterEach(() => cleanup())

function props(over: Partial<CriarOpcoesProps> = {}): CriarOpcoesProps {
  return {
    sources: mockSources(),
    sound: 'none',
    onSound: vi.fn(),
    apps: [],
    onApps: vi.fn(),
    preset: '720p30',
    onPreset: vi.fn(),
    encoder: 'auto',
    onEncoder: vi.fn(),
    ...over,
  }
}

describe('CriarOpcoes', () => {
  it('som_troca: clicar em Todo o PC chama onSound("all")', () => {
    const onSound = vi.fn()
    render(<CriarOpcoes {...props({ onSound })} />)
    fireEvent.click(screen.getByRole('button', { name: 'Todo o PC' }))
    expect(onSound).toHaveBeenCalledWith('all')
  })

  it('apps_visiveis_so_no_modo_apps: com sound="apps" aparecem os apps do mock; com "all" não', () => {
    const { unmount } = render(<CriarOpcoes {...props({ sound: 'apps' })} />)
    expect(screen.getByText('Discord')).toBeTruthy()
    expect(screen.getByText('Chrome')).toBeTruthy()
    expect(screen.getByText('Jogo')).toBeTruthy()
    unmount()

    render(<CriarOpcoes {...props({ sound: 'all' })} />)
    expect(screen.queryByText('Discord')).toBeNull()
    expect(screen.queryByText('Jogo')).toBeNull()
  })

  it('toggle_app: ligar um app chama onApps com [..apps, id]; desligar remove', () => {
    const onApps = vi.fn()
    const { rerender } = render(<CriarOpcoes {...props({ sound: 'apps', apps: ['app:discord'], onApps })} />)

    // Chrome estava desligado: ligar acrescenta o id ao final.
    fireEvent.click(screen.getByRole('switch', { name: 'Chrome' }))
    expect(onApps).toHaveBeenLastCalledWith(['app:discord', 'app:chrome'])

    // Discord estava ligado: desligar remove só ele.
    fireEvent.click(screen.getByRole('switch', { name: 'Discord' }))
    expect(onApps).toHaveBeenLastCalledWith([])

    // A lista original não foi mutada.
    const original = ['app:discord']
    rerender(<CriarOpcoes {...props({ sound: 'apps', apps: original, onApps })} />)
    fireEvent.click(screen.getByRole('switch', { name: 'Jogo' }))
    expect(original).toEqual(['app:discord'])
    expect(onApps).toHaveBeenLastCalledWith(['app:discord', 'app:jogo'])
  })

  it('preset: clicar no cartão "1080p · 60" chama onPreset("1080p60")', () => {
    const onPreset = vi.fn()
    render(<CriarOpcoes {...props({ onPreset })} />)
    fireEvent.click(screen.getByRole('button', { name: '1080p · 60' }))
    expect(onPreset).toHaveBeenCalledWith('1080p60')
  })

  it('preset: o cartão selecionado tem aria-pressed="true"', () => {
    render(<CriarOpcoes {...props({ preset: '720p30' })} />)
    expect(screen.getByRole('button', { name: '720p · 30' }).getAttribute('aria-pressed')).toBe('true')
    expect(screen.getByRole('button', { name: '1080p · 60' }).getAttribute('aria-pressed')).toBe('false')
  })

  it('encoder: renderiza "Automático" como opção', () => {
    render(<CriarOpcoes {...props()} />)
    expect(screen.getByRole('option', { name: 'Automático' })).toBeTruthy()
  })
})
