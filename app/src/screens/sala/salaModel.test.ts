import { describe, expect, it } from 'vitest'
import { mockSnapshot } from '../../bridge/mock'
import type { RoomView } from '../../bridge/types'
import { inviteText, kindLabel, ownTracks, roomChip, watching } from './salaModel'

const sala = (): RoomView => mockSnapshot().room!
const semCodigo = (): RoomView => {
  const r = sala()
  delete r.code
  return r
}

describe('roomChip', () => {
  it('online com codigo mostra ONLINE e o codigo', () => {
    expect(roomChip(sala())).toBe('ONLINE · ZAP-7K2Q')
  })

  it('local sem codigo mostra LOCAL', () => {
    const r = { ...semCodigo(), mode: 'local' as const }
    expect(roomChip(r)).toBe('LOCAL')
  })

  it('online sem codigo mostra ONLINE', () => {
    expect(roomChip(semCodigo())).toBe('ONLINE')
  })

  it('local com codigo mostra LOCAL · codigo', () => {
    const r = { ...sala(), mode: 'local' as const }
    expect(roomChip(r)).toBe('LOCAL · ZAP-7K2Q')
  })
})

describe('kindLabel', () => {
  it('rotula os 3 tipos de trilha', () => {
    expect(kindLabel('screen')).toBe('Tela')
    expect(kindLabel('window')).toBe('Janela')
    expect(kindLabel('camera')).toBe('Webcam')
  })
})

describe('watching', () => {
  it('tile trk-1 mostra Marcão assistindo Tela', () => {
    const main = mockSnapshot().tiles.find((t) => t.track_id === 'trk-1') ?? null
    expect(watching(sala(), main)).toEqual({ name: 'Marcão', kind: 'Tela' })
  })

  it('trilha inexistente retorna null', () => {
    const main = { ...mockSnapshot().tiles[0], track_id: 'trk-nao-existe' }
    expect(watching(sala(), main)).toBeNull()
  })

  it('sem tile principal retorna null', () => {
    expect(watching(sala(), null)).toBeNull()
  })
})

describe('inviteText', () => {
  it('com codigo usa o codigo', () => {
    expect(inviteText(sala(), mockSnapshot().network)).toBe(
      'Entra na sala "Sexta do Rush" no streamaZap: código ZAP-7K2Q',
    )
  })

  it('sem codigo e com radmin_ip usa o IP', () => {
    const net = { radmin_ip: '26.14.7.88', interfaces: [] }
    expect(inviteText(semCodigo(), net)).toBe('Entra na sala "Sexta do Rush" no streamaZap: 26.14.7.88')
  })

  it('sem codigo e sem radmin_ip mostra so o nome', () => {
    const net = { radmin_ip: null, interfaces: [] }
    expect(inviteText(semCodigo(), net)).toBe('Entra na sala "Sexta do Rush" no streamaZap')
  })
})

describe('ownTracks', () => {
  it('meId p-ju acha a camera e nao acha tela', () => {
    const { screen, camera } = ownTracks(sala(), 'p-ju')
    expect(camera?.id).toBe('trk-3')
    expect(screen).toBeNull()
  })

  it('meId desconhecido retorna ambos null', () => {
    expect(ownTracks(sala(), 'p-ninguem')).toEqual({ screen: null, camera: null })
  })

  it('acha tela e janela do proprio usuario, pegando a primeira', () => {
    const r = sala()
    r.tracks.push({ id: 'trk-9', owner_id: 'p-ju', kind: 'window', title: 'x', number: 2 })
    r.tracks.unshift({ id: 'trk-8', owner_id: 'p-ju', kind: 'screen', title: 'y', number: 1 })
    expect(ownTracks(r, 'p-ju').screen?.id).toBe('trk-8')
  })
})
