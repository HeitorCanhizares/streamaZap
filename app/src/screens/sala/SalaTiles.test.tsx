import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'

vi.mock('../../video/player', () => ({
  VideoPlayer: class { push() {} close() {} },
}))

import { api } from '../../bridge/api'
import { SalaTiles } from './SalaTiles'
import type { RoomView, TileView } from '../../bridge/types'

afterEach(() => { cleanup(); vi.restoreAllMocks() })

const room: RoomView = {
  id: 'r', name: 'Sexta', mode: 'online', chat: [], unread: 0,
  participants: [
    { id: 'p1', name: 'Ju', color: '#7CF2C0', ping_ms: 10, sharing: true, camera: false },
    { id: 'p2', name: 'Rafa', color: '#5CD6FF', ping_ms: 20, sharing: true, camera: false },
  ],
  tracks: [
    { id: 't1', owner_id: 'p1', kind: 'camera', title: 'Cam', number: 1 },
    { id: 't2', owner_id: 'p2', kind: 'screen', title: 'Tela', number: 2 },
  ],
}
const tv = (tile: number, track_id: string): TileView => ({ tile, track_id, layer: 2, width: 320, height: 180, fps: 30, kbps: 500, ping_ms: 10, smoothing_ms: 100, reason: '' })
const tiles = [tv(1, 't1'), tv(2, 't2')]

function spies() {
  const watch = vi.spyOn(api, 'watch').mockResolvedValue(undefined as never)
  const unwatch = vi.spyOn(api, 'unwatch').mockResolvedValue(undefined as never)
  vi.spyOn(api, 'setVolume').mockResolvedValue(undefined as never)
  return { watch, unwatch }
}

describe('SalaTiles', () => {
  it('renderiza_voce', () => {
    spies()
    render(<SalaTiles room={room} tiles={[]} mainTile={null} onPick={() => {}} />)
    expect(screen.getByText('Você')).toBeTruthy()
    expect(screen.getByText('Câmera')).toBeTruthy()
    expect(screen.getByText('Tela')).toBeTruthy()
  })

  it('renderiza_tiles', () => {
    spies()
    render(<SalaTiles room={room} tiles={tiles} mainTile={null} onPick={() => {}} />)
    expect(screen.getByText('Ju')).toBeTruthy()
    expect(screen.getByText('Rafa')).toBeTruthy()
  })

  it('exclui_principal', () => {
    spies()
    render(<SalaTiles room={room} tiles={tiles} mainTile={1} onPick={() => {}} />)
    expect(screen.queryByText('Ju')).toBeNull()
    expect(screen.getByText('Rafa')).toBeTruthy()
  })

  it('clique', () => {
    spies()
    const onPick = vi.fn()
    render(<SalaTiles room={room} tiles={tiles} mainTile={null} onPick={onPick} />)
    fireEvent.click(screen.getByText('Rafa'))
    expect(onPick).toHaveBeenCalledWith(2)
  })

  it('controles_nao_selecionam', () => {
    spies()
    const onPick = vi.fn()
    render(<SalaTiles room={room} tiles={[tiles[0]]} mainTile={null} onPick={onPick} />)
    fireEvent.click(screen.getByLabelText('Mutar'))
    expect(onPick).not.toHaveBeenCalled()
  })

  it('watch_unwatch', () => {
    const { watch, unwatch } = spies()
    const { unmount } = render(<SalaTiles room={room} tiles={[tiles[0]]} mainTile={null} onPick={() => {}} />)
    expect(watch).toHaveBeenCalledWith('t1', 1, [320, 180], 2, expect.any(Function))
    unmount()
    expect(unwatch).toHaveBeenCalledWith(1)
  })
})
