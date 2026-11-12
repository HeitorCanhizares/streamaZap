import { describe, expect, it } from 'vitest'
import { mockSnapshot } from '../../bridge/mock'
import type { CameraLayout, Corner, RoomView } from '../../bridge/types'
import {
  celulasDaPrincipal,
  inviteText,
  kindLabel,
  lugaresDasStreams,
  ownTracks,
  pedidoDoLugar,
  roomChip,
  streamsDaSala,
  tileDaTrilha,
  watching,
  type Stream,
} from './salaModel'

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
    expect(kindLabel('audio')).toBe('Só o som')
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
      'Entra na sala "Sexta do Rush" no streamaZap: https://heitorcanhizares.github.io/streamaZap/sala/#ZAP-7K2Q (código ZAP-7K2Q)',
    )
  })

  it('sem codigo e com radmin_ip usa o IP', () => {
    const net = { radmin_ip: '26.14.7.88', interfaces: [] }
    expect(inviteText(semCodigo(), net)).toBe('Entra na sala "Sexta do Rush" no streamaZap: https://heitorcanhizares.github.io/streamaZap/sala/#26.14.7.88 (IP 26.14.7.88)')
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

// Sala com tela + câmera da Ana, janela da Bia, só o som do Caio e as minhas trilhas.
const salaCheia = (): RoomView => ({
  id: 'r', name: 'S', mode: 'local', chat: [], unread: 0,
  participants: [],
  tracks: [
    { id: 'cam-ana', owner_id: 'ana', kind: 'camera', title: 'Cam', number: 2 },
    { id: 'tela-ana', owner_id: 'ana', kind: 'screen', title: 'Tela', number: 1 },
    { id: 'som-ana', owner_id: 'ana', kind: 'audio', title: 'Áudio', number: 3 },
    { id: 'jan-bia', owner_id: 'bia', kind: 'window', title: 'Jogo', number: 4 },
    { id: 'som-caio', owner_id: 'caio', kind: 'audio', title: 'Áudio', number: 5 },
    { id: 'minha', owner_id: 'eu', kind: 'screen', title: 'Minha', number: 6 },
  ],
})

describe('streamsDaSala', () => {
  it('vídeos dos outros (tela antes da câmera), o som só de quem não tem vídeo e os meus no fim', () => {
    const s = streamsDaSala(salaCheia(), 'eu')
    expect(s.map((x) => x.trackId)).toEqual(['tela-ana', 'jan-bia', 'cam-ana', 'som-caio', 'minha'])
    expect(s.filter((x) => x.mine).map((x) => x.trackId)).toEqual(['minha'])
  })

  it('o tile é fixo por trilha', () => {
    const a = streamsDaSala(salaCheia(), 'eu')
    const b = streamsDaSala(salaCheia(), 'eu')
    expect(a.map((s) => s.tile)).toEqual(b.map((s) => s.tile))
    expect(tileDaTrilha('tela-ana')).toBe(a[0].tile)
    expect(new Set(a.map((s) => s.tile)).size).toBe(a.length)
  })
})

describe('lugaresDasStreams', () => {
  const streams = (): Stream[] => streamsDaSala(salaCheia(), 'eu')

  it('padrão: a primeira tela na principal com a câmera do dono; o resto embaixo', () => {
    expect(lugaresDasStreams(streams(), {})).toEqual({
      'tela-ana': 'principal', 'cam-ana': 'principal', 'jan-bia': 'faixa', 'som-caio': 'faixa', minha: 'faixa',
    })
  })

  it('a escolha vale; sem nada na principal, a próxima sem escolha sobe', () => {
    const l = lugaresDasStreams(streams(), { 'tela-ana': 'faixa' })
    expect(l['tela-ana']).toBe('faixa')
    expect(l['jan-bia']).toBe('principal')
    expect(l['cam-ana']).toBe('faixa')
  })

  it('várias na principal e som nunca vai para a principal', () => {
    const l = lugaresDasStreams(streams(), { 'jan-bia': 'principal', 'som-caio': 'principal' })
    expect(l['jan-bia']).toBe('principal')
    expect(l['som-caio']).toBe('faixa')
    // Já tem algo na principal: a tela da Ana fica embaixo (sem escolha).
    expect(l['tela-ana']).toBe('faixa')
  })

  it('só a minha transmissão: ela vai sozinha para a principal (com a minha câmera no canto)', () => {
    const r: RoomView = {
      ...salaCheia(),
      tracks: [
        { id: 'minha-cam', owner_id: 'eu', kind: 'camera', title: 'Cam', number: 8 },
        { id: 'minha', owner_id: 'eu', kind: 'screen', title: 'Minha', number: 6 },
        { id: 'som-caio', owner_id: 'caio', kind: 'audio', title: 'Áudio', number: 5 },
      ],
    }
    const s = streamsDaSala(r, 'eu')
    const l = lugaresDasStreams(s, {})
    expect(l).toEqual({ minha: 'principal', 'minha-cam': 'principal', 'som-caio': 'faixa' })
    expect(celulasDaPrincipal(s, l).map((c) => [c.stream.trackId, c.pip?.trackId ?? null])).toEqual([['minha', 'minha-cam']])
  })

  it('alguém começa a transmitir vídeo: a dele vai para a principal e a minha desce', () => {
    const s = streamsDaSala(salaCheia(), 'eu')
    const l = lugaresDasStreams(s, {})
    expect(l.minha).toBe('faixa')
    expect(l['tela-ana']).toBe('principal')
  })

  it('fechada fica fechada', () => {
    expect(lugaresDasStreams(streams(), { 'tela-ana': 'fechada' })['tela-ana']).toBe('fechada')
  })
})

describe('celulasDaPrincipal', () => {
  it('câmera do dono vai no canto da tela dele, não em outra célula', () => {
    const s = streamsDaSala(salaCheia(), 'eu')
    const c = celulasDaPrincipal(s, lugaresDasStreams(s, { 'jan-bia': 'principal' }))
    expect(c.map((x) => [x.stream.trackId, x.pip?.trackId ?? null])).toEqual([['jan-bia', null]])
    const c2 = celulasDaPrincipal(s, lugaresDasStreams(s, { 'jan-bia': 'principal', 'tela-ana': 'principal' }))
    expect(c2.map((x) => [x.stream.trackId, x.pip?.trackId ?? null])).toEqual([
      ['tela-ana', 'cam-ana'],
      ['jan-bia', null],
    ])
  })

  it('câmera sozinha na principal vira célula', () => {
    const s = streamsDaSala(salaCheia(), 'eu')
    const c = celulasDaPrincipal(s, { 'cam-ana': 'principal', 'tela-ana': 'faixa', 'jan-bia': 'faixa', 'som-caio': 'faixa' })
    expect(c).toEqual([{ stream: s.find((x) => x.trackId === 'cam-ana'), pip: null }])
  })
})

describe('layout da câmera (aceite B4)', () => {
  const sala2 = (layout?: CameraLayout, comTela = true): RoomView => ({
    ...salaCheia(),
    tracks: [
      ...(comTela ? [{ id: 'tela-z', owner_id: 'zeca', kind: 'screen' as const, title: 'T', number: 1 }] : []),
      { id: 'cam-z', owner_id: 'zeca', kind: 'camera' as const, title: 'C', number: 2, ...(layout ? { camera_layout: layout } : {}) },
    ],
  })
  const monta = (layout?: CameraLayout, jeitos?: Record<string, 'canto' | 'solta'>, comTela = true) => {
    const s = streamsDaSala(sala2(layout, comTela), 'eu')
    const l = lugaresDasStreams(s, {})
    return { s, l, c: celulasDaPrincipal(s, l, jeitos) }
  }

  for (const canto of ['sup_esq', 'sup_dir', 'inf_esq', 'inf_dir'] as Corner[]) {
    it(`canto ${canto}: uma célula com a câmera no canto`, () => {
      const { c } = monta({ modo: 'canto', canto })
      expect(c).toHaveLength(1)
      expect(c[0].pip?.trackId).toBe('cam-z')
      expect(c[0].corner).toBe(canto)
    })
  }

  it('solta: duas células, a câmera logo depois da tela e com podeCanto', () => {
    const { c } = monta({ modo: 'solta' })
    expect(c.map((x) => [x.stream.trackId, x.pip])).toEqual([['tela-z', null], ['cam-z', null]])
    expect(c[0].podeCanto).toBeUndefined()
    expect(c[1].podeCanto).toBe(true)
  })

  it('câmera sem layout: canto inf_dir', () => {
    const { c } = monta()
    expect(c).toHaveLength(1)
    expect(c[0].pip?.trackId).toBe('cam-z')
    expect(c[0].corner).toBe('inf_dir')
  })

  it('a escolha de quem assiste vale mais que a de quem transmite', () => {
    const a = monta({ modo: 'canto', canto: 'sup_esq' }, { 'cam-z': 'solta' })
    expect(a.c.map((x) => x.stream.trackId)).toEqual(['tela-z', 'cam-z'])
    expect(a.c.every((x) => x.pip === null)).toBe(true)
    const b = monta({ modo: 'solta' }, { 'cam-z': 'canto' })
    expect(b.c).toHaveLength(1)
    expect(b.c[0].pip?.trackId).toBe('cam-z')
    expect(b.c[0].corner).toBe('inf_dir')
  })

  it('câmera de dono sem tela na principal: célula própria no fim, sem podeCanto', () => {
    const { c } = monta({ modo: 'solta' }, undefined, false)
    expect(c).toHaveLength(1)
    expect(c[0].stream.trackId).toBe('cam-z')
    expect(c[0].pip).toBeNull()
    expect(c[0].podeCanto).toBeUndefined()
  })

  it('streamsDaSala copia camera_layout só nas câmeras e só quando existe', () => {
    const r: RoomView = {
      ...salaCheia(),
      tracks: [
        { id: 'tela-z', owner_id: 'zeca', kind: 'screen', title: 'T', number: 1, camera_layout: { modo: 'solta' } },
        { id: 'cam-z', owner_id: 'zeca', kind: 'camera', title: 'C', number: 2, camera_layout: { modo: 'solta' } },
        { id: 'cam-w', owner_id: 'wil', kind: 'camera', title: 'C', number: 3 },
      ],
    }
    const s = streamsDaSala(r, 'eu')
    const por = (id: string) => s.find((x) => x.trackId === id)!
    expect(por('cam-z').cameraLayout).toEqual({ modo: 'solta' })
    expect('cameraLayout' in por('tela-z')).toBe(false)
    expect('cameraLayout' in por('cam-w')).toBe(false)
  })

  it('lugaresDasStreams: câmera solta de quem tem a tela na principal também fica na principal', () => {
    const { l } = monta({ modo: 'solta' })
    expect(l['tela-z']).toBe('principal')
    expect(l['cam-z']).toBe('principal')
  })
})

describe('pedidoDoLugar', () => {
  it('faixa pede a camada pequena; mais células pedem menos', () => {
    expect(pedidoDoLugar('faixa', 1)).toEqual({ size: [320, 180], maxLayer: 2 })
    expect(pedidoDoLugar('principal', 1).size).toEqual([1920, 1080])
    expect(pedidoDoLugar('principal', 2).size).toEqual([960, 540])
    expect(pedidoDoLugar('canto', 1).maxLayer).toBe(1)
    // A minha na principal pede o tamanho médio (o núcleo não liga a melhor camada só para mim).
    expect(pedidoDoLugar('principal', 1, true).size).toEqual([1280, 720])
    expect(pedidoDoLugar('faixa', 1, true).size).toEqual([320, 180])
  })
})
