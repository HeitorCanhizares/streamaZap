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

  it('cada sala tem endereço e qualidade', () => {
    for (const r of mockSnapshot().rooms) {
      expect(r.addr).toBeTruthy()
      expect(r.quality).toBeTruthy()
    }
  })

  it('Sexta do Rush tem 3 quem compartilha e 4 pessoas', () => {
    const sala = mockSnapshot().rooms.find((r) => r.name === 'Sexta do Rush')!
    expect(sala.sharers).toHaveLength(3)
    expect(sala.people).toHaveLength(4)
  })

  it('Live de Python está em modo local', () => {
    const sala = mockSnapshot().rooms.find((r) => r.name === 'Live de Python')!
    expect(sala.mode).toBe('local')
  })

  it('sala mock tem code ZAP-7K2Q', () => {
    expect(mockSnapshot().room!.code).toBe('ZAP-7K2Q')
  })

  it('participantes mock trazem volume entre 0 e 1 e muted booleano', () => {
    for (const p of mockSnapshot().room!.participants) {
      expect(p.volume).toBeGreaterThanOrEqual(0)
      expect(p.volume).toBeLessThanOrEqual(1)
      expect(typeof p.muted).toBe('boolean')
    }
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

  it('mockSettings tem server.use_relay true e video.encoder Automatico', () => {
    const s = mockSettings()
    expect(s.server!.use_relay).toBe(true)
    expect(s.video.encoder).toBe('Automatico')
  })

  it('default_preset do mock e um dos 4 presets validos (720p30,720p60,1080p30,1080p60)', () => {
    expect(['720p30', '720p60', '1080p30', '1080p60']).toContain(mockSettings().video.default_preset)
  })
})

describe('mockSources', () => {
  it('tem 2 telas, 3 janelas, 1 câmera, 5 apps (com o estado do som) e os encoders', () => {
    const s = mockSources()
    expect(s.screens).toHaveLength(2)
    expect(s.windows).toHaveLength(3)
    expect(s.cameras).toHaveLength(1)
    expect(s.apps.map((a) => a.name)).toEqual(['Spotify', 'Discord', 'Google Chrome', 'Jogo', 'Visual Studio Code'])
    expect(s.apps.map((a) => a.sound)).toEqual(['playing', 'ready', 'ready', 'silent', 'silent'])
    expect(s.encoders).toEqual(['Automatico', 'NVENC', 'x264'])
  })
})
