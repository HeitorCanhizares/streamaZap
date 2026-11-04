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
  tracks: [{ id: 't1', owner_id: 'p1', kind: 'screen', title: 'Tela', number: 1 }],
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
    expect(t).toContain('200 ms')
    expect(t).toContain('camada 2')
    expect(t).toContain('8.2 Mbps')
  })

  it('sem_main', () => {
    montar({ main: null })
    expect(screen.getByText('Ninguém transmitindo ainda')).toBeTruthy()
  })

  it('botoes', () => {
    const f = montar()
    fireEvent.click(screen.getByText('Sair'))
    expect(f.onLeave).toHaveBeenCalled()
    fireEvent.click(screen.getByText('Compartilhar tela'))
    expect(f.onToggleShare).toHaveBeenCalled()
  })

  it('botao_parar', () => {
    montar({ share: { source: 's', audio: 'none', preset: '720p30', encoder: 'x', layers: [], viewers_per_layer: [], encode_ms: 1 } })
    expect(screen.getByText('Parar de compartilhar')).toBeTruthy()
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
})
