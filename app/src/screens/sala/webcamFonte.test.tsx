import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'

vi.mock('../../video/player', () => ({
  VideoPlayer: class { push() {} close() {} },
}))

import { SalaPrincipal } from './SalaPrincipal'
import { SalaTiles } from './SalaTiles'
import { SalaVoce } from './SalaVoce'
import { kindLabel, lugaresDasStreams, ownTracks, streamsDaSala, volumeInicial, type Stream } from './salaModel'
import type { RoomView } from '../../bridge/types'

afterEach(() => { cleanup(); vi.restoreAllMocks() })

const participantes = [
  { id: 'eu', name: 'Eu', color: '#fff', ping_ms: 1, sharing: true, camera: true },
  { id: 'p1', name: 'Ju', color: '#7CF2C0', ping_ms: 10, sharing: true, camera: true },
]
const sala = (tracks: RoomView['tracks']): RoomView => ({ id: 'r', name: 'Sexta', mode: 'online', chat: [], unread: 0, me_id: 'eu', participants: participantes, tracks })
const canvasDe = () => document.createElement('canvas')

describe('webcam como fonte (trilha camera)', () => {
  it('(1) só câmera minha: ownTracks, rótulo e cartão Você', () => {
    const room = sala([{ id: 't1', owner_id: 'eu', kind: 'camera', title: 'Webcam', number: 1 }])
    const own = ownTracks(room, 'eu')
    expect(own.camera?.id).toBe('t1')
    expect(own.screen).toBeNull()
    expect(kindLabel('camera')).toBe('Webcam')
    const cam = streamsDaSala(room, 'eu')[0]!
    render(<SalaVoce room={room} meId="eu" camOn sharing={false} cam={cam} lugares={{ t1: 'faixa' }} canvasDe={canvasDe} />)
    expect(screen.getByRole('img', { name: 'Prévia: sua câmera' })).toBeTruthy()
    expect(screen.queryByText('Você não está transmitindo')).toBeNull()
    expect(screen.queryByRole('img', { name: 'Prévia: sua tela' })).toBeNull()
  })

  it('(2) câmera de outra pessoa na faixa aparece como Webcam', () => {
    const room = sala([{ id: 't1', owner_id: 'p1', kind: 'camera', title: 'Webcam', number: 1 }])
    const streams = streamsDaSala(room, 'eu')
    render(
      <SalaTiles room={room} streams={streams} lugares={{ t1: 'faixa' }} canvasDe={canvasDe}
        volumeDe={(id) => volumeInicial(room.participants.find((p) => p.id === id))} onVolumePessoa={vi.fn()} onMover={vi.fn()} />,
    )
    expect((document.body.textContent ?? '')).toContain('Webcam')
    expect((document.body.textContent ?? '')).not.toContain('Tela')
  })

  it('(3) câmera ordena depois de tela/janela e, sozinha, vai para a principal', () => {
    const room = sala([
      { id: 'c', owner_id: 'p1', kind: 'camera', title: 'Webcam', number: 1 },
      { id: 's', owner_id: 'p1', kind: 'screen', title: 'Tela', number: 2 },
    ])
    expect(streamsDaSala(room, 'eu').map((s) => s.trackId)).toEqual(['s', 'c'])
    const so = streamsDaSala(sala([{ id: 'c', owner_id: 'p1', kind: 'camera', title: 'Webcam', number: 1 }]), 'eu')
    expect(lugaresDasStreams(so, {})).toEqual({ c: 'principal' })
  })

  it('(4) tecla C chama onToggleCamera uma vez', () => {
    const room = sala([{ id: 't1', owner_id: 'eu', kind: 'camera', title: 'Webcam', number: 1 }])
    const onToggleCamera = vi.fn()
    const s: Stream = { trackId: 't1', ownerId: 'eu', kind: 'camera', tile: 5, mine: true }
    render(
      <SalaPrincipal room={room} celulas={[{ stream: s, pip: null }]} tiles={[]} canvasDe={canvasDe} temOutras={false} share={null}
        smoothingAuto onSmoothingAuto={vi.fn()} onToggleCamera={onToggleCamera} onToggleShare={vi.fn()} onLeave={vi.fn()} onFullscreen={vi.fn()}
        onMover={vi.fn()} onVolumePessoa={vi.fn()} volumeDe={() => ({ vol: 80, muted: false })} />,
    )
    fireEvent.keyDown(window, { key: 'c' })
    expect(onToggleCamera).toHaveBeenCalledTimes(1)
  })

  it('(5) câmera + som do PC: sem trilha screen', () => {
    const room = sala([
      { id: 'c', owner_id: 'eu', kind: 'camera', title: 'Webcam', number: 1 },
      { id: 'a', owner_id: 'eu', kind: 'audio', title: 'Som', number: 2 },
    ])
    const own = ownTracks(room, 'eu')
    expect(own.screen).toBeNull()
    expect(own.camera?.id).toBe('c')
    expect(streamsDaSala(room, 'eu').map((s) => s.kind)).toEqual(['camera'])
    const cam = streamsDaSala(room, 'eu')[0]!
    render(<SalaVoce room={room} meId="eu" camOn sharing cam={cam} lugares={{ c: 'faixa' }} canvasDe={canvasDe} />)
    expect(screen.getByRole('img', { name: 'Prévia: sua câmera' })).toBeTruthy()
    expect(screen.queryByText('Transmitindo só o som')).toBeNull()
  })
})
