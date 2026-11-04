// Tarefa criar-05
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { CriarSom, type CriarSomProps } from './CriarSom'
import type { SourceItem } from '../../bridge/types'

afterEach(() => cleanup())

const APPS: SourceItem[] = [
  { id: 'cs2', kind: 'app', name: 'cs2.exe' },
  { id: 'chrome', kind: 'app', name: 'chrome.exe' },
  { id: 'spotify', kind: 'app', name: 'Spotify.exe' },
]

function props(over: Partial<CriarSomProps> = {}): CriarSomProps {
  return { apps: APPS, sound: 'none', onSound: vi.fn(), marcados: [], onMarcados: vi.fn(), ...over }
}

describe('CriarSom', () => {
  it('titulo: mostra "05" e "Som"', () => {
    render(<CriarSom {...props()} />)
    expect(screen.getByText('05')).toBeTruthy()
    expect(screen.getByText('Som')).toBeTruthy()
  })

  it('tres radios com aria-checked conforme sound', () => {
    const { rerender } = render(<CriarSom {...props({ sound: 'none' })} />)
    const radio = (nome: RegExp) => screen.getByRole('radio', { name: nome })
    expect(radio(/Sem som/).getAttribute('aria-checked')).toBe('true')
    expect(radio(/Todo o PC/).getAttribute('aria-checked')).toBe('false')
    expect(radio(/Só alguns apps/).getAttribute('aria-checked')).toBe('false')
    rerender(<CriarSom {...props({ sound: 'all' })} />)
    expect(radio(/Sem som/).getAttribute('aria-checked')).toBe('false')
    expect(radio(/Todo o PC/).getAttribute('aria-checked')).toBe('true')
    rerender(<CriarSom {...props({ sound: 'apps' })} />)
    expect(radio(/Só alguns apps/).getAttribute('aria-checked')).toBe('true')
    expect(radio(/Todo o PC/).getAttribute('aria-checked')).toBe('false')
  })

  it('clicar em "Todo o PC" chama onSound("all")', () => {
    const onSound = vi.fn()
    render(<CriarSom {...props({ onSound })} />)
    fireEvent.click(screen.getByRole('radio', { name: /Todo o PC/ }))
    expect(onSound).toHaveBeenLastCalledWith('all')
  })

  it('pilulas dos apps aparecem so com sound "apps"', () => {
    const { rerender } = render(<CriarSom {...props({ sound: 'all' })} />)
    expect(screen.queryByRole('button', { name: 'cs2.exe' })).toBeNull()
    rerender(<CriarSom {...props({ sound: 'none' })} />)
    expect(screen.queryByRole('button', { name: 'chrome.exe' })).toBeNull()
    rerender(<CriarSom {...props({ sound: 'apps' })} />)
    expect(screen.getByRole('button', { name: 'cs2.exe' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'chrome.exe' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Spotify.exe' })).toBeTruthy()
  })

  it('clicar em pilula nao marcada chama onMarcados com array novo incluindo o id', () => {
    const onMarcados = vi.fn()
    const marcados = ['cs2']
    render(<CriarSom {...props({ sound: 'apps', marcados, onMarcados })} />)
    fireEvent.click(screen.getByRole('button', { name: 'chrome.exe' }))
    expect(onMarcados).toHaveBeenLastCalledWith(['cs2', 'chrome'])
    expect(onMarcados.mock.calls[0][0]).not.toBe(marcados)
    expect(marcados).toEqual(['cs2'])
  })

  it('clicar em pilula marcada chama onMarcados sem o id, com array novo', () => {
    const onMarcados = vi.fn()
    const marcados = ['cs2', 'chrome']
    render(<CriarSom {...props({ sound: 'apps', marcados, onMarcados })} />)
    fireEvent.click(screen.getByRole('button', { name: 'cs2.exe' }))
    expect(onMarcados).toHaveBeenLastCalledWith(['chrome'])
    expect(onMarcados.mock.calls[0][0]).not.toBe(marcados)
    expect(marcados).toEqual(['cs2', 'chrome'])
  })

  it('pilula marcada tem aria-pressed=true e as nao marcadas false', () => {
    render(<CriarSom {...props({ sound: 'apps', marcados: ['spotify'] })} />)
    expect(screen.getByRole('button', { name: 'Spotify.exe' }).getAttribute('aria-pressed')).toBe('true')
    expect(screen.getByRole('button', { name: 'cs2.exe' }).getAttribute('aria-pressed')).toBe('false')
  })

  it('mostra a legenda do rodape', () => {
    render(<CriarSom {...props()} />)
    expect(screen.getByText('O som do próprio streamaZap nunca vai junto.')).toBeTruthy()
  })
})
