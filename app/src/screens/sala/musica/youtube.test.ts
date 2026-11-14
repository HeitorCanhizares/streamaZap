import { describe, expect, it } from 'vitest'
import { ESTADO, LIMITE_DESVIO_MS, decidir, duracaoTexto, lerMensagem, miniatura, posicaoDaSala, relatorio, srcDoPlayer } from './youtube'

describe('srcDoPlayer', () => {
  it('liga a API do embed, começa tocando e já na posição da sala', () => {
    const u = new URL(srcDoPlayer('dQw4w9WgXcQ', 65_400, 'http://tauri.localhost'))
    expect(u.origin + u.pathname).toBe('https://www.youtube.com/embed/dQw4w9WgXcQ')
    expect(u.searchParams.get('enablejsapi')).toBe('1')
    expect(u.searchParams.get('autoplay')).toBe('1')
    expect(u.searchParams.get('origin')).toBe('http://tauri.localhost')
    expect(u.searchParams.get('start')).toBe('65')
  })

  it('com a sala pausada, carrega sem tocar', () => {
    expect(new URL(srcDoPlayer('dQw4w9WgXcQ', 65_400, 'x', false)).searchParams.get('autoplay')).toBe('0')
  })

  it('live e posição negativa não levam start', () => {
    expect(new URL(srcDoPlayer('dQw4w9WgXcQ', null, 'x')).searchParams.has('start')).toBe(false)
    expect(new URL(srcDoPlayer('dQw4w9WgXcQ', -1500, 'x')).searchParams.has('start')).toBe(false)
  })
})

describe('posicaoDaSala', () => {
  it('soma o tempo desde que o núcleo calculou', () => {
    expect(posicaoDaSala({ position_ms: 10_000, at_ms: 1_000_000 }, 1_002_500)).toBe(12_500)
    expect(posicaoDaSala({ position_ms: -2_000, at_ms: 1_000_000 }, 1_000_500)).toBe(-1_500)
    // Relógio "voltou" (snapshot do futuro): não anda para trás.
    expect(posicaoDaSala({ position_ms: 5_000, at_ms: 1_000_000 }, 999_000)).toBe(5_000)
  })

  it('pausada, fica parada', () => {
    expect(posicaoDaSala({ position_ms: 10_000, at_ms: 1_000_000, paused: true }, 1_060_000)).toBe(10_000)
  })
})

describe('lerMensagem', () => {
  it('lê infoDelivery com estado, tempo, duração e dados do vídeo', () => {
    const m = lerMensagem(JSON.stringify({
      event: 'infoDelivery',
      info: { playerState: 1, currentTime: 8.2, duration: 19.02, videoData: { video_id: 'jNQXAC9IVRw', title: ' Me at the zoo ', isLive: false } },
    }))
    expect(m).toEqual({ estado: 1, tempo: 8.2, duracao: 19.02, videoId: 'jNQXAC9IVRw', titulo: 'Me at the zoo', aoVivo: false })
  })

  it('lê pronto, mudança de estado e erro; ignora o resto', () => {
    expect(lerMensagem('{"event":"onReady"}')).toEqual({ pronto: true })
    expect(lerMensagem({ event: 'onStateChange', info: 0 })).toEqual({ estado: 0 })
    expect(lerMensagem('{"event":"onError","info":150}')).toEqual({ erro: 150 })
    expect(lerMensagem('não é json')).toBeNull()
    expect(lerMensagem({ event: 'outra' })).toBeNull()
    expect(lerMensagem(null)).toBeNull()
  })
})

describe('decidir', () => {
  const tocando = (tempoS: number) => ({ estado: ESTADO.tocando, tempo: tempoS })

  it('tocando perto da sala: nada; longe: busca a posição da sala', () => {
    expect(decidir({ live: false, posSalaMs: 30_000, info: tocando(29.5), tempoAndou: true })).toEqual({ comandos: [], travado: false })
    const longe = decidir({ live: false, posSalaMs: 30_000, info: tocando(30 - (LIMITE_DESVIO_MS + 500) / 1000), tempoAndou: true })
    expect(longe.comandos).toEqual([{ func: 'seekTo', args: [30, true] }])
  })

  it('tocando sem o tempo andar (anúncio): espera, sem buscar', () => {
    expect(decidir({ live: false, posSalaMs: 60_000, info: tocando(0), tempoAndou: false })).toEqual({ comandos: [], travado: true })
  })

  it('pausado: volta a tocar e alcança a sala', () => {
    expect(decidir({ live: false, posSalaMs: 40_000, info: { estado: ESTADO.pausado, tempo: 10 }, tempoAndou: false }).comandos).toEqual([
      { func: 'seekTo', args: [40, true] },
      { func: 'playVideo' },
    ])
    expect(decidir({ live: false, posSalaMs: 10_500, info: { estado: ESTADO.pausado, tempo: 10 }, tempoAndou: false }).comandos).toEqual([{ func: 'playVideo' }])
  })

  it('antes de começar: espera parado', () => {
    expect(decidir({ live: false, posSalaMs: -1_000, info: tocando(0), tempoAndou: true }).comandos).toEqual([{ func: 'pauseVideo' }])
    expect(decidir({ live: false, posSalaMs: -1_000, info: { estado: ESTADO.pronto }, tempoAndou: false }).comandos).toEqual([])
  })

  it('terminou e carregando: nada', () => {
    expect(decidir({ live: false, posSalaMs: 90_000, info: { estado: ESTADO.terminou, tempo: 80 }, tempoAndou: false }).comandos).toEqual([])
    expect(decidir({ live: false, posSalaMs: 90_000, info: { estado: ESTADO.carregando, tempo: 10 }, tempoAndou: false }).comandos).toEqual([])
  })

  it('sala pausada: para o player na posição da sala', () => {
    const pausada = (posSalaMs: number, info: { estado?: number; tempo?: number }, live = false) =>
      decidir({ live, pausada: true, posSalaMs, info, tempoAndou: true })
    // Tocando perto: só pausa. Longe: busca e pausa.
    expect(pausada(30_000, tocando(30))).toEqual({ comandos: [{ func: 'pauseVideo' }], travado: false })
    expect(pausada(30_000, tocando(10)).comandos).toEqual([{ func: 'seekTo', args: [30, true] }, { func: 'pauseVideo' }])
    // Já pausado: nada; longe, só busca (a busca não tira do pausado).
    expect(pausada(30_000, { estado: ESTADO.pausado, tempo: 30 }).comandos).toEqual([])
    expect(pausada(45_000, { estado: ESTADO.pausado, tempo: 30 }).comandos).toEqual([{ func: 'seekTo', args: [45, true] }])
    // Pronto (sem tocar ainda) e longe: a busca faz tocar, então pausa em seguida.
    expect(pausada(45_000, { estado: ESTADO.pronto }).comandos).toEqual([{ func: 'seekTo', args: [45, true] }, { func: 'pauseVideo' }])
    expect(pausada(500, { estado: ESTADO.pronto }).comandos).toEqual([])
    // Carregando: pausa. Terminou: nada.
    expect(pausada(30_000, { estado: ESTADO.carregando, tempo: 30 }).comandos).toEqual([{ func: 'pauseVideo' }])
    expect(pausada(30_000, { estado: ESTADO.terminou, tempo: 30 }).comandos).toEqual([])
    // Live: só para, sem buscar.
    expect(pausada(999_000, tocando(5), true).comandos).toEqual([{ func: 'pauseVideo' }])
    expect(pausada(999_000, { estado: ESTADO.pausado, tempo: 5 }, true).comandos).toEqual([])
    // Nunca é anúncio com a sala pausada.
    expect(decidir({ live: false, pausada: true, posSalaMs: 30_000, info: tocando(30), tempoAndou: false }).travado).toBe(false)
  })

  it('live nunca busca posição: só garante que está tocando', () => {
    expect(decidir({ live: true, posSalaMs: 999_000, info: tocando(5), tempoAndou: true }).comandos).toEqual([])
    expect(decidir({ live: true, posSalaMs: 999_000, info: { estado: ESTADO.pausado }, tempoAndou: false }).comandos).toEqual([{ func: 'playVideo' }])
    expect(decidir({ live: true, posSalaMs: 0, info: tocando(5), tempoAndou: false }).travado).toBe(true)
  })
})

describe('relatorio', () => {
  it('conta a duração, a live e o título só uma vez e só do vídeo da sala', () => {
    const info = { videoId: 'aaaaaaaaaaa', duracao: 212.4, aoVivo: true, titulo: 'Do player' }
    expect(relatorio('aaaaaaaaaaa', true, info, {})).toEqual({ duration_ms: 212_400, live: true, title: 'Do player' })
    expect(relatorio('aaaaaaaaaaa', true, info, { duracaoMs: 212_400, aoVivo: true, titulo: 'Do player' })).toBeNull()
    // Título real já conhecido: não manda o do player.
    expect(relatorio('aaaaaaaaaaa', false, { videoId: 'aaaaaaaaaaa', titulo: 'X' }, {})).toBeNull()
    // Outro vídeo (anúncio): nada.
    expect(relatorio('aaaaaaaaaaa', true, { ...info, videoId: 'anuncio1234' }, {})).toBeNull()
    // Duração zero (live ou ainda sem saber): não conta.
    expect(relatorio('aaaaaaaaaaa', false, { duracao: 0 }, {})).toBeNull()
  })
})

describe('textos', () => {
  it('duração e miniatura', () => {
    expect(duracaoTexto(187_000, false)).toBe('3:07')
    expect(duracaoTexto(3_765_000, false)).toBe('1:02:45')
    expect(duracaoTexto(undefined, false)).toBe('')
    expect(duracaoTexto(10, true)).toBe('AO VIVO')
    expect(miniatura('dQw4w9WgXcQ')).toBe('https://i.ytimg.com/vi/dQw4w9WgXcQ/mqdefault.jpg')
  })
})
