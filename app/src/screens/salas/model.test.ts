import { describe, expect, it } from 'vitest'
import type { RoomListing } from '../../bridge/types'
import {
  contar, corPessoa, corPing, estiloAvatar, filtrarSalas, paraSalaVM, salasDoModo,
} from './model'

const minima: RoomListing = {
  room_id: 'r1', name: 'Sexta do Rush', host: 'Marcão', participants: 3,
  has_password: false, mode: 'online',
}

const trancada: RoomListing = { ...minima, room_id: 'r2', name: 'Corrida', host: 'Rafa', has_password: true, mode: 'online' }
const local: RoomListing = { ...minima, room_id: 'r3', name: 'Filme', host: 'Bia', mode: 'local' }

describe('corPessoa', () => {
  it('usa a paleta do design e cai no verde para nomes desconhecidos', () => {
    expect(corPessoa('Ju')).toBe('#7CF2C0')
    expect(corPessoa('Desconhecido')).toBe('#3BF08A')
  })
})

describe('corPing', () => {
  it('muda de cor nas fronteiras 60 e 100', () => {
    expect(corPing(59)).toBe('#3BF08A')
    expect(corPing(60)).toBe('#FFB547')
    expect(corPing(99)).toBe('#FFB547')
    expect(corPing(100)).toBe('#FF8592')
  })
})

describe('estiloAvatar', () => {
  it('sem anel não tem boxShadow', () => {
    const est = estiloAvatar('Ju', 32, false)
    expect(est.boxShadow).toBeUndefined()
    expect(est.width).toBe(32)
    expect(est.fontSize).toBe('13px')
    expect(est.background).toBe('#7CF2C024')
  })

  it('com anel tem boxShadow com a cor da pessoa', () => {
    const est = estiloAvatar('Ju', 32, true)
    expect(String(est.boxShadow)).toContain('#0F1916')
    expect(String(est.boxShadow)).toContain('#7CF2C0')
  })
})

describe('paraSalaVM', () => {
  it('aplica defaults numa sala só com campos obrigatórios', () => {
    const vm = paraSalaVM(minima)
    expect(vm).toMatchObject({
      id: 'r1', nome: 'Sexta do Rush', host: 'Marcão', modo: 'online', addr: '',
      trancada: false, versaoAviso: '', cena: 'game', camCena: null, hora: '', ultima: '',
      naoLidas: 0, digitando: false, ping: 0, qualidade: '720p30', bitrate: '',
      assistindo: 2,
    })
    expect(vm.compartilhando).toEqual([{ name: 'Marcão', kind: 'Tela', scene: 'game' }])
    expect(vm.pessoas).toEqual([{ name: 'Marcão', role: 'host', ping_ms: 0 }])
  })

  it('respeita campos opcionais quando presentes', () => {
    const vm = paraSalaVM({
      ...minima, addr: 'ZAP-7K2Q', scene: 'race', cam_scene: 'camc', last_time: '22:14',
      last_message: 'oi', unread: 3, typing: true, ping_ms: 38, quality: '1080p60',
      bitrate: '6,2 Mbps', viewers: 4, version_warn: '0.0.9',
      sharers: [{ name: 'Ju', kind: 'Webcam', scene: 'camb' }],
      people: [{ name: 'Ju', role: 'webcam', ping_ms: 12 }],
    })
    expect(vm).toMatchObject({
      addr: 'ZAP-7K2Q', cena: 'race', camCena: 'camc', hora: '22:14', ultima: 'oi',
      naoLidas: 3, digitando: true, ping: 38, qualidade: '1080p60', bitrate: '6,2 Mbps',
      assistindo: 4, versaoAviso: '0.0.9',
    })
    expect(vm.compartilhando).toEqual([{ name: 'Ju', kind: 'Webcam', scene: 'camb' }])
    expect(vm.pessoas).toEqual([{ name: 'Ju', role: 'webcam', ping_ms: 12 }])
  })

  it('calcula assistindo a partir de participantes quando viewers falta', () => {
    expect(paraSalaVM({ ...minima, participants: 0 }).assistindo).toBe(0)
  })
})

describe('salasDoModo', () => {
  it('separa online e local', () => {
    const rooms = [minima, trancada, local]
    expect(salasDoModo(rooms, 'online').map((s) => s.id)).toEqual(['r1', 'r2'])
    expect(salasDoModo(rooms, 'local').map((s) => s.id)).toEqual(['r3'])
  })
})

describe('contar', () => {
  it('conta todas, abertas e com senha', () => {
    const salas = salasDoModo([minima, trancada, local], 'online')
    expect(contar(salas)).toEqual({ todas: 2, abertas: 1, senha: 1 })
    expect(contar([])).toEqual({ todas: 0, abertas: 0, senha: 0 })
  })
})

describe('filtrarSalas', () => {
  const salas = salasDoModo([minima, trancada], 'online')

  it('filtra por abertas e com senha', () => {
    expect(filtrarSalas(salas, 'todas', '').map((s) => s.id)).toEqual(['r1', 'r2'])
    expect(filtrarSalas(salas, 'abertas', '').map((s) => s.id)).toEqual(['r1'])
    expect(filtrarSalas(salas, 'senha', '').map((s) => s.id)).toEqual(['r2'])
  })

  it('busca sem acento e sem caixa no nome ou host', () => {
    expect(filtrarSalas(salas, 'todas', 'marcao').map((s) => s.id)).toEqual(['r1'])
    expect(filtrarSalas(salas, 'todas', '  RUSH  ').map((s) => s.id)).toEqual(['r1'])
    expect(filtrarSalas(salas, 'todas', 'rafa').map((s) => s.id)).toEqual(['r2'])
  })

  it('busca sem resultado devolve lista vazia', () => {
    expect(filtrarSalas(salas, 'todas', 'xyz')).toEqual([])
  })
})
