// Tarefa salas-08
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { SalaPrevia } from './SalaPrevia'
import type { SalaVM } from './model'

const mocks = vi.hoisted(() => ({
  tauri: { ligado: false },
  hook: vi.fn((_codigo: string | null) => () => {}),
}))

vi.mock('../../bridge/api', () => ({ isTauri: () => mocks.tauri.ligado }))
vi.mock('../../bridge/useRoomPreview', () => ({ useRoomPreview: (codigo: string | null) => mocks.hook(codigo) }))

afterEach(() => cleanup())

beforeEach(() => {
  mocks.tauri.ligado = false
  mocks.hook.mockClear()
})

const base: SalaVM = {
  id: 'r1', nome: 'Sexta do Rush', host: 'Marcão', modo: 'online', addr: 'ZAP-7K2Q', trancada: false, versaoAviso: '',
  cena: 'game', camCena: 'cama', hora: '21:58', ultima: '', naoLidas: 0, digitando: false, ping: 38,
  qualidade: '1080p60', bitrate: '', assistindo: 7, compartilhando: [], pessoas: [],
}

describe('SalaPrevia', () => {
  it('com camCena mostra o PiP e duas cenas', () => {
    const { container } = render(<SalaPrevia sala={base} />)
    expect(screen.getByText('cam · Marcão')).toBeTruthy()
    const cenas = Array.from(container.querySelectorAll('svg[data-scene]')).map((s) => s.getAttribute('data-scene'))
    expect(cenas).toEqual(['game', 'cama'])
  })

  it('sem camCena não mostra o PiP', () => {
    render(<SalaPrevia sala={{ ...base, camCena: null }} />)
    expect(screen.queryByText(/cam ·/)).toBeNull()
  })

  it('mostra legenda, qualidade e AO VIVO', () => {
    render(<SalaPrevia sala={base} />)
    expect(screen.getByText('Prévia em baixa resolução, sem som. O áudio começa quando você entra.')).toBeTruthy()
    expect(screen.getByText('1080p60')).toBeTruthy()
    expect(screen.getByText('AO VIVO')).toBeTruthy()
  })

  it('com bitrate mostra qualidade · bitrate', () => {
    render(<SalaPrevia sala={{ ...base, bitrate: '6 Mbps' }} />)
    expect(screen.getByText('1080p60 · 6 Mbps')).toBeTruthy()
  })

  it('(a) fora do Tauri: sem canvas, hook com null e ScenePreview presente', () => {
    const { container } = render(<SalaPrevia sala={base} />)
    expect(screen.queryByTestId('previa-canvas')).toBeNull()
    expect(mocks.hook).toHaveBeenLastCalledWith(null)
    expect(container.querySelectorAll('svg[data-scene]').length).toBeGreaterThan(0)
  })

  it('(b) Tauri + sala online ZAP: canvas de previa, hook com o codigo e sem PiP', () => {
    mocks.tauri.ligado = true
    render(<SalaPrevia sala={base} />)
    expect(screen.getByTestId('previa-canvas')).toBeTruthy()
    expect(mocks.hook).toHaveBeenLastCalledWith('ZAP-7K2Q')
    expect(screen.queryByText(/cam ·/)).toBeNull()
  })

  it('(c) Tauri + sala local: sem canvas e hook com null', () => {
    mocks.tauri.ligado = true
    render(<SalaPrevia sala={{ ...base, modo: 'local', addr: '' }} />)
    expect(screen.queryByTestId('previa-canvas')).toBeNull()
    expect(mocks.hook).toHaveBeenLastCalledWith(null)
  })

  it('(d) AO VIVO e legenda presentes nos tres casos', () => {
    const casos: { tauri: boolean; sala: SalaVM }[] = [
      { tauri: false, sala: base },
      { tauri: true, sala: base },
      { tauri: true, sala: { ...base, modo: 'local', addr: '' } },
    ]
    for (const c of casos) {
      mocks.tauri.ligado = c.tauri
      const { unmount } = render(<SalaPrevia sala={c.sala} />)
      expect(screen.getByText('AO VIVO')).toBeTruthy()
      expect(screen.getByText('Prévia em baixa resolução, sem som. O áudio começa quando você entra.')).toBeTruthy()
      unmount()
    }
  })

  it('(e) sala com senha: sem prévia nenhuma, nem a real no Tauri', () => {
    mocks.tauri.ligado = true
    const { container } = render(<SalaPrevia sala={{ ...base, trancada: true }} />)
    expect(screen.getByTestId('previa-privada')).toBeTruthy()
    expect(screen.getByText('Sala com senha')).toBeTruthy()
    expect(screen.queryByTestId('previa-canvas')).toBeNull()
    expect(mocks.hook).toHaveBeenLastCalledWith(null)
    // Nem a cena ilustrativa, o AO VIVO ou a câmera no canto.
    expect(container.querySelectorAll('svg[data-scene]').length).toBe(0)
    expect(screen.queryByText('AO VIVO')).toBeNull()
    expect(screen.queryByText(/cam ·/)).toBeNull()
    expect(screen.getByText('Sala privada: sem prévia.')).toBeTruthy()
  })
})
