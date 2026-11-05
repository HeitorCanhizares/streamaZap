import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'

vi.mock('../../video/player', () => ({
  VideoPlayer: class { push() {} close() {} },
}))

import { api } from '../../bridge/api'
import { SalaPrincipal } from './SalaPrincipal'
import type { RoomView, TileView } from '../../bridge/types'

afterEach(() => { cleanup(); vi.restoreAllMocks() })

const room: RoomView = {
  id: 'r', name: 'Sexta', mode: 'online', chat: [], unread: 0,
  participants: [{ id: 'p1', name: 'Ana', color: '#fff', ping_ms: 10, sharing: true, camera: false }],
  tracks: [
    { id: 't1', owner_id: 'p1', kind: 'screen', title: 'Tela', number: 1 },
    { id: 't2', owner_id: 'p1', kind: 'camera', title: 'Câmera', number: 2 },
  ],
}
const main: TileView = { tile: 7, track_id: 't1', layer: 1, width: 1920, height: 1080, fps: 60, kbps: 8200, ping_ms: 42, smoothing_ms: 200, reason: '' }

function montar(over: Partial<Parameters<typeof SalaPrincipal>[0]> = {}) {
  const fns = { onSmoothingAuto: vi.fn(), onToggleCamera: vi.fn(), onToggleShare: vi.fn(), onLeave: vi.fn(), onFullscreen: vi.fn() }
  render(<SalaPrincipal room={room} main={main} share={null} smoothingAuto={true} {...fns} {...over} />)
  return fns
}

describe('SalaPrincipal', () => {
  it('titulo_e_metricas', () => {
    montar()
    const t = document.body.textContent ?? ''
    expect(t).toContain('42 ms')
    expect(screen.getByLabelText('Suavização automática, 200 milissegundos')).toBeTruthy()
    expect(t).toContain('camada 2')
  })

  it('sem_main', () => {
    montar({ main: null })
    expect(screen.getByText('Nenhuma transmissão aberta')).toBeTruthy()
  })

  it('botoes', () => {
    const f = montar()
    fireEvent.click(screen.getByText('Sair'))
    expect(f.onLeave).toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: /Compartilhar tela/ }))
    expect(f.onToggleShare).toHaveBeenCalled()
  })

  it('botao_parar', () => {
    montar({ share: { source: 's', audio: 'none', preset: '720p30', encoder: 'x', layers: [], viewers_per_layer: [], encode_ms: 1, conversion: [] } })
    expect(screen.getAllByRole('button', { name: 'Parar' }).length).toBeGreaterThan(0)
  })

  it('atalhos', () => {
    const f = montar()
    fireEvent.keyDown(window, { key: 'c' })
    expect(f.onToggleCamera).toHaveBeenCalled()
    fireEvent.keyDown(window, { key: 'F11' })
    expect(f.onFullscreen).toHaveBeenCalled()
  })

  it('watch', () => {
    const w = vi.spyOn(api, 'watch').mockResolvedValue(undefined)
    const u = vi.spyOn(api, 'unwatch').mockResolvedValue(undefined)
    montar()
    expect(w).toHaveBeenCalledWith('t1', 7, expect.any(Array), null, expect.any(Function))
    cleanup()
    expect(u).toHaveBeenCalledWith(7)
  })

  it('estrutura', () => {
    montar()
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('Sexta')
    expect(screen.getByText(/AO VIVO/)).toBeTruthy()
    expect(screen.getByRole('toolbar', { name: 'Controles da sala' })).toBeTruthy()
  })

  it('fechar_tela', () => {
    const onCloseMain = vi.fn()
    montar({ onCloseMain })
    fireEvent.click(screen.getByRole('button', { name: 'Fechar tela de Ana' }))
    expect(onCloseMain).toHaveBeenCalled()
  })

  it('pip', () => {
    const onTogglePip = vi.fn()
    const { rerender } = render(<SalaPrincipal room={room} main={main} share={null} smoothingAuto onSmoothingAuto={vi.fn()} onToggleCamera={vi.fn()} onToggleShare={vi.fn()} onLeave={vi.fn()} onFullscreen={vi.fn()} onTogglePip={onTogglePip} />)
    fireEvent.click(screen.getByRole('button', { name: 'Fechar câmera de Ana' }))
    expect(onTogglePip).toHaveBeenCalled()
    rerender(<SalaPrincipal room={room} main={main} share={null} smoothingAuto onSmoothingAuto={vi.fn()} onToggleCamera={vi.fn()} onToggleShare={vi.fn()} onLeave={vi.fn()} onFullscreen={vi.fn()} onTogglePip={onTogglePip} pipHidden />)
    expect(screen.queryByRole('button', { name: 'Fechar câmera de Ana' })).toBeNull()
    expect(screen.getByText('Mostrar câmera')).toBeTruthy()
  })

  it('repassa invite ao cabecalho', () => {
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true })
    montar({ invite: 'Entra na sala "Sexta" no streamaZap: código ZAP-7K2Q' })
    fireEvent.click(screen.getByRole('button', { name: 'Copiar convite' }))
    expect(writeText).toHaveBeenCalledWith('Entra na sala "Sexta" no streamaZap: código ZAP-7K2Q')
  })

  it('tecla_m', () => {
    const s = vi.spyOn(api, 'setVolume').mockResolvedValue(undefined)
    montar()
    fireEvent.keyDown(window, { key: 'M' })
    expect(s).toHaveBeenCalledWith('p1', 1, true)
  })
})
