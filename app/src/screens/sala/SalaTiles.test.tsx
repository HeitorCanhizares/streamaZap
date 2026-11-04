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
  it('renderiza_grupo_e_pessoas', () => {
    spies()
    render(<SalaTiles room={room} tiles={tiles} mainTile={null} onPick={() => {}} />)
    expect(screen.getByRole('group', { name: 'Transmissões na sala' })).toBeTruthy()
    expect(screen.getByText('Ju')).toBeTruthy()
    expect(screen.getByText('Rafa')).toBeTruthy()
    expect(screen.getAllByLabelText(/^Silenciar /)).toHaveLength(2)
  })

  it('mudo_chama_setvolume', () => {
    spies()
    render(<SalaTiles room={room} tiles={tiles} mainTile={null} onPick={() => {}} />)
    fireEvent.click(screen.getByLabelText('Silenciar Ju'))
    expect(api.setVolume).toHaveBeenCalledWith('p1', 0.8, true)
    expect(screen.getByLabelText('Ativar som de Ju')).toBeTruthy()
    expect(screen.getByText('mudo')).toBeTruthy()
  })

  it('range_atualiza_porcentagem', () => {
    spies()
    render(<SalaTiles room={room} tiles={[tiles[0]]} mainTile={null} onPick={() => {}} />)
    fireEvent.change(screen.getByLabelText('Volume de Ju'), { target: { value: '35' } })
    expect(screen.getByText('35%')).toBeTruthy()
    expect(api.setVolume).toHaveBeenCalledWith('p1', 0.35, false)
  })

  it('fechar_e_reabrir', () => {
    const { watch, unwatch } = spies()
    render(<SalaTiles room={room} tiles={[tiles[0]]} mainTile={null} onPick={() => {}} />)
    expect(watch).toHaveBeenCalledTimes(1)
    fireEvent.click(screen.getByLabelText('Fechar tela de Ju'))
    expect(screen.getByText('Tela de Ju fechada')).toBeTruthy()
    expect(unwatch).toHaveBeenCalledWith(1)
    fireEvent.click(screen.getByText('Reabrir'))
    expect(screen.queryByText('Tela de Ju fechada')).toBeNull()
    expect(watch).toHaveBeenCalledTimes(2)
  })

  it('principal_mostra_assistindo', () => {
    spies()
    render(<SalaTiles room={room} tiles={tiles} mainTile={1} onPick={() => {}} />)
    expect(screen.getAllByText('Assistindo')).toHaveLength(1)
  })

  it('voce_chama_callbacks', () => {
    spies()
    const cam = vi.fn(); const share = vi.fn()
    render(<SalaTiles room={room} tiles={[]} mainTile={null} onPick={() => {}} onToggleCam={cam} onToggleShare={share} />)
    expect(screen.getByText('Você')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Câmera' }))
    fireEvent.click(screen.getByRole('button', { name: 'Tela' }))
    expect(cam).toHaveBeenCalled()
    expect(share).toHaveBeenCalled()
  })

  it('clique_no_card_chama_onpick', () => {
    spies()
    const onPick = vi.fn()
    render(<SalaTiles room={room} tiles={tiles} mainTile={null} onPick={onPick} />)
    fireEvent.click(screen.getByLabelText(/^Assistir Rafa/))
    expect(onPick).toHaveBeenCalledWith(2)
  })

  it('controles_nao_selecionam', () => {
    spies()
    const onPick = vi.fn()
    render(<SalaTiles room={room} tiles={[tiles[0]]} mainTile={null} onPick={onPick} />)
    fireEvent.click(screen.getByLabelText('Silenciar Ju'))
    expect(onPick).not.toHaveBeenCalled()
  })

  it('watch_unwatch', () => {
    const { watch, unwatch } = spies()
    const { unmount } = render(<SalaTiles room={room} tiles={[tiles[0]]} mainTile={null} onPick={() => {}} />)
    expect(watch).toHaveBeenCalledWith('t1', 1, [320, 180], 2, expect.any(Function))
    unmount()
    expect(unwatch).toHaveBeenCalledWith(1)
  })

  it('volume inicial do tile vem do participante (Ju 60% mostra 60%)', () => {
    spies()
    const room60: RoomView = {
      ...room,
      participants: [
        { id: 'p-ju', name: 'Ju', color: '#7CF2C0', ping_ms: 10, sharing: true, camera: false, volume: 0.6 },
      ],
      tracks: [{ id: 'trk-3', owner_id: 'p-ju', kind: 'camera', title: 'Cam', number: 3 }],
    }
    render(<SalaTiles room={room60} tiles={[tv(1, 'trk-3')]} mainTile={null} onPick={() => {}} />)
    expect(screen.getByText('60%')).toBeTruthy()
  })

  it('participante muted inicia como mudo', () => {
    spies()
    const roomMudo: RoomView = {
      ...room,
      participants: [
        { id: 'p-rafa', name: 'Rafa', color: '#5CD6FF', ping_ms: 20, sharing: true, camera: false, muted: true },
      ],
      tracks: [{ id: 'trk-4', owner_id: 'p-rafa', kind: 'screen', title: 'Tela', number: 4 }],
    }
    render(<SalaTiles room={roomMudo} tiles={[tv(2, 'trk-4')]} mainTile={null} onPick={() => {}} />)
    expect(screen.getByText('mudo')).toBeTruthy()
    expect(screen.getByLabelText('Ativar som de Rafa')).toBeTruthy()
  })

  it('renderiza o cartao Você via SalaVoce', () => {
    const { watch } = spies()
    const cam = vi.fn(); const share = vi.fn()
    const roomMe: RoomView = {
      ...room,
      tracks: [...room.tracks, { id: 'trk-me', owner_id: 'me', kind: 'camera', title: 'Minha', number: 9 }],
    }
    render(<SalaTiles room={roomMe} tiles={[]} mainTile={null} onPick={() => {}} meId="me" camOn onToggleCam={cam} onToggleShare={share} />)
    expect(screen.getByText('Você')).toBeTruthy()
    expect(screen.getByText('sua câmera')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Câmera' }))
    fireEvent.click(screen.getByRole('button', { name: 'Tela' }))
    expect(cam).toHaveBeenCalled()
    expect(share).toHaveBeenCalled()
    expect(watch).toHaveBeenCalledWith('trk-me', 9001, [320, 180], 2, expect.any(Function))
  })
})
