// Tarefa criar-05
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { CriarSom, gruposDeApps, type CriarSomProps } from './CriarSom'
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

  const COM_ESTADO: SourceItem[] = [
    { id: 'app:1', kind: 'app', name: 'Spotify', sound: 'playing' },
    { id: 'app:2', kind: 'app', name: 'Discord', sound: 'ready' },
    { id: 'app:3', kind: 'app', name: 'Jogo', sound: 'silent' },
  ]

  it('separa os apps com som dos que so tem janela', () => {
    expect(gruposDeApps(COM_ESTADO)).toEqual({ comSom: [COM_ESTADO[0], COM_ESTADO[1]], outros: [COM_ESTADO[2]] })
    // Sem estado (lista antiga): conta como com som.
    expect(gruposDeApps(APPS).outros).toEqual([])
    render(<CriarSom {...props({ apps: COM_ESTADO, sound: 'apps' })} />)
    expect(screen.getByText('Com som')).toBeTruthy()
    expect(screen.getByText(/Outros apps abertos/)).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Jogo' })).toBeTruthy()
  })

  it('marca quem esta tocando agora', () => {
    render(<CriarSom {...props({ apps: COM_ESTADO, sound: 'apps' })} />)
    expect(screen.getByRole('button', { name: 'Spotify' }).getAttribute('title')).toBe('Tocando agora')
    expect(screen.getAllByTestId('tocando')).toHaveLength(1)
    expect(screen.getByRole('button', { name: 'Discord' }).getAttribute('title')).toBeNull()
  })

  it('atualiza a lista ao escolher "So alguns apps" e pelo botao', () => {
    const onRefresh = vi.fn()
    const { rerender } = render(<CriarSom {...props({ onRefresh })} />)
    fireEvent.click(screen.getByRole('radio', { name: /Só alguns apps/ }))
    expect(onRefresh).toHaveBeenCalledTimes(1)
    rerender(<CriarSom {...props({ onRefresh, sound: 'apps' })} />)
    // Já escolhido: clicar de novo não recarrega.
    fireEvent.click(screen.getByRole('radio', { name: /Só alguns apps/ }))
    expect(onRefresh).toHaveBeenCalledTimes(1)
    fireEvent.click(screen.getByRole('button', { name: 'Atualizar lista' }))
    expect(onRefresh).toHaveBeenCalledTimes(2)
  })

  it('lista vazia explica o que fazer', () => {
    render(<CriarSom {...props({ apps: [], sound: 'apps', onRefresh: vi.fn() })} />)
    expect(screen.getByText('Nenhum app tocou nada ainda.')).toBeTruthy()
    expect(screen.getByText(/Abra o app que vai tocar/)).toBeTruthy()
  })
})
