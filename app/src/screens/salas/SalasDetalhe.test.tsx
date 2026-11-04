// Tarefa ui-13
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import type { PeerListing, RoomListing } from '../../bridge/types'
import { SalasDetalhe } from './SalasDetalhe'

afterEach(() => cleanup())

const sala: RoomListing = {
  room_id: 'r1',
  name: 'Sexta do Rush',
  host: 'Ana',
  participants: 4,
  has_password: false,
  mode: 'local',
  live_title: 'Rush final',
  scene: 'race',
}

const peers: PeerListing[] = [
  { name: 'Bruno', version: '3.0.0', ip: '192.168.0.10', receives_you: true },
  { name: 'Carla', version: '3.0.0', ip: '192.168.0.11', receives_you: false },
]

function renderDetalhe(over: Partial<Parameters<typeof SalasDetalhe>[0]> = {}) {
  const props = {
    room: sala,
    peers,
    onEnter: vi.fn(),
    onEnterManual: vi.fn(),
    ...over,
  }
  render(<SalasDetalhe {...props} />)
  return props
}

describe('SalasDetalhe', () => {
  it('estado_vazio: room=null mostra Escolha uma sala', () => {
    renderDetalhe({ room: null })
    expect(screen.getByText('Escolha uma sala na lista')).toBeTruthy()
    expect(screen.queryByText('Entrar na sala')).toBeNull()
  })

  it('com_sala: mostra o nome da sala e o botão Entrar na sala', () => {
    renderDetalhe()
    expect(screen.getByText('Sexta do Rush')).toBeTruthy()
    expect(screen.getByText('Entrar na sala')).toBeTruthy()
    expect(screen.getByText('Prévia em miniatura · sem áudio')).toBeTruthy()
    expect(screen.getByText('Rush final')).toBeTruthy()
  })

  it('clique_entrar: chama onEnter com a sala', () => {
    const props = renderDetalhe()
    fireEvent.click(screen.getByText('Entrar na sala'))
    expect(props.onEnter).toHaveBeenCalledTimes(1)
    expect(props.onEnter).toHaveBeenCalledWith(sala)
  })

  it('clique_manual: chama onEnterManual', () => {
    const props = renderDetalhe()
    fireEvent.click(screen.getByText('Entrar com código/IP'))
    expect(props.onEnterManual).toHaveBeenCalledTimes(1)
  })

  it('peers: lista nomes dos peers e recebe você', () => {
    renderDetalhe()
    expect(screen.getByText('Bruno')).toBeTruthy()
    expect(screen.getByText('Carla')).toBeTruthy()
    expect(screen.getByText('192.168.0.10')).toBeTruthy()
    expect(screen.getByText('recebe você')).toBeTruthy()
    expect(screen.getByText('não recebe')).toBeTruthy()
  })

  it('peers vazio: mostra Ninguém mais por aqui', () => {
    renderDetalhe({ peers: [] })
    expect(screen.getByText('Ninguém mais por aqui')).toBeTruthy()
  })
})
