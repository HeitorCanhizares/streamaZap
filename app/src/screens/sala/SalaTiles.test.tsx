import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'

vi.mock('../../video/player', () => ({
  VideoPlayer: class { push() {} close() {} },
}))

import { api } from '../../bridge/api'
import { SalaTiles, type SalaTilesProps } from './SalaTiles'
import { volumeInicial, type Lugar, type Stream, type VolumePessoa } from './salaModel'
import type { RoomView } from '../../bridge/types'

afterEach(() => { cleanup(); vi.restoreAllMocks() })

const room: RoomView = {
  id: 'r', name: 'Sexta', mode: 'online', chat: [], unread: 0,
  participants: [
    { id: 'p1', name: 'Ju', color: '#7CF2C0', ping_ms: 10, sharing: true, camera: false, volume: 0.6 },
    { id: 'p2', name: 'Rafa', color: '#5CD6FF', ping_ms: 20, sharing: true, camera: false, muted: true },
    { id: 'p3', name: 'Dudu', color: '#FF9E5C', ping_ms: 30, sharing: false, camera: false },
  ],
  tracks: [
    { id: 't1', owner_id: 'p1', kind: 'camera', title: 'Cam', number: 1 },
    { id: 't2', owner_id: 'p2', kind: 'screen', title: 'Tela', number: 2 },
    { id: 't3', owner_id: 'p3', kind: 'audio', title: 'Áudio', number: 3 },
  ],
}
const st = (trackId: string, ownerId: string, kind: Stream['kind'], tile: number): Stream => ({ trackId, ownerId, kind, tile })
const t1 = st('t1', 'p1', 'camera', 1)
const t2 = st('t2', 'p2', 'screen', 2)
const t3 = st('t3', 'p3', 'audio', 3)

function montar(over: Partial<SalaTilesProps> = {}) {
  const onMover = vi.fn<(trackId: string, lugar: Lugar) => void>()
  const onVolumePessoa = vi.fn<(ownerId: string, v: VolumePessoa) => void>()
  const props: SalaTilesProps = {
    room,
    streams: [t1, t2],
    lugares: { t1: 'faixa', t2: 'faixa', t3: 'faixa' },
    canvasDe: () => document.createElement('canvas'),
    volumeDe: (id) => volumeInicial(room.participants.find((p) => p.id === id)),
    onVolumePessoa,
    onMover,
    ...over,
  }
  render(<SalaTiles {...props} />)
  return { onMover, onVolumePessoa }
}

describe('SalaTiles', () => {
  it('mostra um cartão por transmissão com nome e volume', () => {
    montar()
    expect(screen.getByRole('group', { name: 'Transmissões na sala' })).toBeTruthy()
    expect(screen.getAllByTestId('cartao-faixa')).toHaveLength(2)
    expect(screen.getByText('Ju')).toBeTruthy()
    expect(screen.getByText('Rafa')).toBeTruthy()
  })

  it('volume inicial vem do participante: Ju 60%, Rafa muda', () => {
    montar()
    expect(screen.getByText('60%')).toBeTruthy()
    expect(screen.getByText('mudo')).toBeTruthy()
    expect(screen.getByLabelText('Ativar som de Rafa')).toBeTruthy()
  })

  it('mudo e régua repassam o volume da pessoa', () => {
    const { onVolumePessoa } = montar()
    fireEvent.click(screen.getByLabelText('Silenciar Ju'))
    expect(onVolumePessoa).toHaveBeenCalledWith('p1', { vol: 60, muted: true })
    fireEvent.change(screen.getByLabelText('Volume de Ju'), { target: { value: '35' } })
    expect(onVolumePessoa).toHaveBeenCalledWith('p1', { vol: 35, muted: false })
  })

  it('pôr na tela principal: botão e clique na miniatura', () => {
    const { onMover } = montar()
    fireEvent.click(screen.getByLabelText('Pôr tela de Rafa na tela principal'))
    expect(onMover).toHaveBeenCalledWith('t2', 'principal')
    fireEvent.click(screen.getByLabelText('Assistir Ju, Webcam'))
    expect(onMover).toHaveBeenCalledWith('t1', 'principal')
  })

  it('fechar e reabrir', () => {
    const { onMover } = montar()
    fireEvent.click(screen.getByLabelText('Fechar tela de Ju'))
    expect(onMover).toHaveBeenCalledWith('t1', 'fechada')
    cleanup()
    const r = montar({ lugares: { t1: 'fechada', t2: 'faixa' } })
    expect(screen.getByText('Tela de Ju fechada')).toBeTruthy()
    fireEvent.click(screen.getByText('Reabrir'))
    expect(r.onMover).toHaveBeenCalledWith('t1', 'faixa')
  })

  it('o canvas da transmissão vai para o cartão', () => {
    const canvas = document.createElement('canvas')
    montar({ streams: [t2], canvasDe: () => canvas })
    expect(screen.getByRole('img', { name: 'Tela de Rafa' }).contains(canvas)).toBe(true)
  })

  it('só o som: cartão com a pessoa e parar de ouvir', () => {
    const { onMover } = montar({ streams: [t3] })
    expect(screen.getByText('Dudu · só o som')).toBeTruthy()
    fireEvent.click(screen.getByLabelText('Parar de ouvir Dudu'))
    expect(onMover).toHaveBeenCalledWith('t3', 'fechada')
  })

  it('sem transmissões embaixo e sem transmitir: a faixa some', () => {
    montar({ streams: [] })
    expect(screen.queryByRole('group', { name: 'Transmissões na sala' })).toBeNull()
  })

  it('cartão Você só quando estou transmitindo', () => {
    vi.spyOn(api, 'watch').mockResolvedValue(undefined)
    const cam = vi.fn()
    const share = vi.fn()
    const roomMe: RoomView = {
      ...room,
      tracks: [...room.tracks, { id: 'trk-me', owner_id: 'me', kind: 'camera', title: 'Minha', number: 9 }],
    }
    montar({ room: roomMe, streams: [], mostrarVoce: true, meId: 'me', camOn: true, onToggleCam: cam, onToggleShare: share })
    expect(screen.getByText('Você')).toBeTruthy()
    expect(screen.getByText('sua câmera')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Câmera' }))
    fireEvent.click(screen.getByRole('button', { name: 'Tela' }))
    expect(cam).toHaveBeenCalled()
    expect(share).toHaveBeenCalled()
    expect(api.watch).toHaveBeenCalledWith('trk-me', 9001, [320, 180], 2, expect.any(Function))
  })
})
