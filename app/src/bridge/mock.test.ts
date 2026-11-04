import { describe, expect, it } from 'vitest'
import { mockSettings, mockSnapshot, mockSources } from './mock'

describe('mockSnapshot', () => {
  it('traz as três salas do design com os nomes exatos', () => {
    const names = mockSnapshot().rooms.map((r) => r.name)
    expect(names).toEqual(['Sexta do Rush', 'Filminho de domingo', 'Live de Python'])
  })

  it('a sala aberta tem Marcão, Ju, Rafa e Dudu', () => {
    const room = mockSnapshot().room
    expect(room).not.toBeNull()
    const names = room!.participants.map((p) => p.name)
    expect(names).toEqual(['Marcão', 'Ju', 'Rafa', 'Dudu'])
    expect(room!.participants).toHaveLength(4)
  })

  it('ids de sala são únicos', () => {
    const ids = mockSnapshot().rooms.map((r) => r.room_id)
    expect(new Set(ids).size).toBe(ids.length)
  })

  it('duas chamadas não compartilham referência', () => {
    const a = mockSnapshot()
    const b = mockSnapshot()
    expect(a).not.toBe(b)
    expect(a.rooms).not.toBe(b.rooms)
    expect(a.room!.participants).not.toBe(b.room!.participants)
    a.rooms[0].name = 'mutado'
    expect(mockSnapshot().rooms[0].name).toBe('Sexta do Rush')
  })

  it('tem um tile com a qualidade do design', () => {
    const tiles = mockSnapshot().tiles
    expect(tiles).toHaveLength(1)
    expect(tiles[0]).toMatchObject({ layer: 2, width: 1280, height: 720, fps: 60, ping_ms: 24 })
  })
})

describe('mockSettings', () => {
  it('usa a porta padrão do plano', () => {
    expect(mockSettings().local.port).toBe(47800)
  })

  it('não compartilha referência entre chamadas', () => {
    const a = mockSettings()
    const b = mockSettings()
    expect(a).not.toBe(b)
    expect(a.local).not.toBe(b.local)
  })
})

describe('mockSources', () => {
  it('tem 2 telas, 3 janelas, 1 câmera, 3 apps e os encoders', () => {
    const s = mockSources()
    expect(s.screens).toHaveLength(2)
    expect(s.windows).toHaveLength(3)
    expect(s.cameras).toHaveLength(1)
    expect(s.apps.map((a) => a.name)).toEqual(['Discord', 'Chrome', 'Jogo'])
    expect(s.encoders).toEqual(['Automatico', 'NVENC', 'x264'])
  })
})
