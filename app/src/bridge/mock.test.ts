import { afterEach, describe, expect, it, vi } from 'vitest'
import { api } from './api'
import { mockSettings, mockSnapshot, mockSnapshotComPlano, mockSources } from './mock'

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

  it('account do mock: provider supabase, exige senha e não exige conta', () => {
    const acc = mockSnapshot().account!
    expect(acc.account_provider).toEqual({ provider: 'supabase', url: 'https://mock.supabase.co', key: 'mock-anon-key' })
    expect(acc.requires_password).toBe(true)
    expect(acc.requires_account).toBe(false)
    expect(acc).not.toHaveProperty('plan')
    expect(acc).not.toHaveProperty('login_error')
  })

  it('duas chamadas devolvem objetos account diferentes', () => {
    const a = mockSnapshot().account!
    const b = mockSnapshot().account!
    expect(a).not.toBe(b)
    expect(a.account_provider).not.toBe(b.account_provider)
    a.account_provider!.provider = 'mutado'
    expect(mockSnapshot().account!.account_provider!.provider).toBe('supabase')
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

  it('câmera tem o melhor modo nativo: max_height 1080 e max_fps 60', () => {
    const [camera] = mockSources().cameras
    expect(camera.max_height).toBe(1080)
    expect(camera.max_fps).toBe(60)
  })
})

describe('mockSettings (v3.3.0: servidor, câmera e atualização)', () => {
  it('server.enabled é true no mock, para o design mostrar o Online ligado', () => {
    expect(mockSettings().server!.enabled).toBe(true)
  })

  it('câmera do mock começa em 720p30 com o canto inferior direito', () => {
    const v = mockSettings().video
    expect(v.camera_preset).toBe('720p30')
    expect(v.camera_layout).toEqual({ modo: 'canto', canto: 'inf_dir' })
  })

  it('atualização do mock: check, download, install auto e canal estável', () => {
    const u = mockSettings().update
    expect(u).toEqual({ check: true, download: true, install: 'auto', channel: 'estavel' })
  })

  it('a seção update não tem a chave antiga "auto"', () => {
    expect(mockSettings().update).not.toHaveProperty('auto')
  })
})

describe('api.settingsSet no mock (merge por seção)', () => {
  it('trocar só install mantém check, download e channel da seção update', async () => {
    const s = await api.settingsSet({ update: { install: 'ask' } as never })
    expect(s.update).toEqual({ check: true, download: true, install: 'ask', channel: 'estavel' })
  })
})

describe('mockSnapshot com plano (?plano=)', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('sem parâmetro não tem quality_locks nem account.plan', () => {
    const snap = mockSnapshot()
    expect('quality_locks' in snap).toBe(false)
    expect(Object.keys(snap)).not.toContain('quality_locks')
    expect('plan' in snap.account!).toBe(false)
  })

  it('mockSnapshotComPlano("hd") trava só 1080p', () => {
    const snap = mockSnapshotComPlano('hd')
    expect(snap.quality_locks!.can_publish).toBe(true)
    expect(snap.quality_locks!.screen).toEqual([
      { preset: '1080p30', needs: 'fullhd' },
      { preset: '1080p60', needs: 'fullhd' },
    ])
    expect(snap.account!.plan).toEqual({ id: 'hd', pode_entrar: true })
  })

  it('espectador não publica e tem 4 travados', () => {
    const snap = mockSnapshotComPlano('espectador')
    expect(snap.quality_locks!.can_publish).toBe(false)
    expect(snap.quality_locks!.screen).toHaveLength(4)
    expect(snap.account!.plan!.pode_entrar).toBe(true)
  })

  it('nenhum não pode entrar', () => {
    expect(mockSnapshotComPlano('nenhum').account!.plan).toEqual({ id: 'nenhum', pode_entrar: false })
  })

  it('fullhd tem listas vazias', () => {
    const q = mockSnapshotComPlano('fullhd').quality_locks!
    expect(q.can_publish).toBe(true)
    expect(q.screen).toEqual([])
    expect(q.camera).toEqual([])
  })

  it('?plano=hd na URL traz as travas do hd', () => {
    vi.stubGlobal('location', { search: '?plano=hd' })
    const snap = mockSnapshot()
    expect(snap.quality_locks!.screen).toHaveLength(2)
    expect(snap.account!.plan!.id).toBe('hd')
  })

  it('plano inválido ou outro parâmetro sozinho não adiciona chaves', () => {
    for (const search of ['?plano=ouro', '?cobranca=1']) {
      vi.stubGlobal('location', { search })
      const snap = mockSnapshot()
      expect('quality_locks' in snap).toBe(false)
      expect('plan' in snap.account!).toBe(false)
    }
  })

  it('?plano=hd&musica traz travas e música juntas', () => {
    vi.stubGlobal('location', { search: '?plano=hd&musica' })
    const snap = mockSnapshot()
    expect(snap.quality_locks).toBeDefined()
    expect(snap.room!.music).toBeDefined()
  })
})
