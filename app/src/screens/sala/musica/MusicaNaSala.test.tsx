// A música como transmissão da sala: onde ela aparece, o player e o que ele conta ao núcleo.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import type { RoomView, Snapshot } from '../../../bridge/types'

const estado = vi.hoisted(() => ({ snap: null as unknown as Snapshot }))
vi.mock('../../../bridge/store', () => ({ useSnapshot: () => estado.snap }))
vi.mock('../../../video/player', () => ({ VideoPlayer: class { push() {} close() {} } }))

import { api } from '../../../bridge/api'
import { mockMusic, mockSnapshot } from '../../../bridge/mock'
import { SalaScreen } from '../SalaScreen'
import { lugaresDasStreams, streamsDaSala, MUSICA } from '../salaModel'
import { PlayerYoutube } from './PlayerYoutube'

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
  vi.useRealTimers()
  localStorage.clear()
})

function snapCom(room: Partial<RoomView>): Snapshot {
  const s = mockSnapshot()
  if (!s.room) throw new Error('mock sem sala')
  return { ...s, room: { ...s.room, mode: 'local', music: mockMusic(), ...room } }
}

const iframe = (): HTMLIFrameElement | null => document.querySelector('iframe')

function manda(alvo: HTMLIFrameElement, data: unknown): void {
  act(() => {
    window.dispatchEvent(new MessageEvent('message', { data: JSON.stringify(data), source: alvo.contentWindow }))
  })
}

describe('música no layout', () => {
  it('sem vídeo dos outros, a música vai para a principal; com vídeo, para a faixa', () => {
    const soMusica = snapCom({ tracks: [] }).room as RoomView
    const s1 = streamsDaSala(soMusica, 'eu')
    expect(s1.map((s) => s.kind)).toEqual(['musica'])
    expect(lugaresDasStreams(s1, {})[MUSICA]).toBe('principal')

    const comVideo = snapCom({}).room as RoomView
    const s2 = streamsDaSala(comVideo, 'eu')
    expect(s2.map((s) => s.kind)).toContain('musica')
    expect(lugaresDasStreams(s2, {})[MUSICA]).toBe('faixa')

    const semMusica = snapCom({ music: { queue: [], current: null } }).room as RoomView
    expect(streamsDaSala(semMusica, 'eu').some((s) => s.kind === 'musica')).toBe(false)
  })
})

describe('música na SalaScreen', () => {
  it('na principal: a célula deixa a vaga; o player do YouTube já começa na posição da sala', () => {
    estado.snap = snapCom({ tracks: [] })
    render(<SalaScreen onLeft={() => {}} />)
    expect(screen.getByTestId('celula-musica').querySelector('[data-testid="vaga-musica"]')).toBeTruthy()
    const f = iframe()
    expect(f).toBeTruthy()
    const src = new URL(f?.getAttribute('src') ?? '')
    expect(src.pathname).toBe('/embed/jNQXAC9IVRw')
    expect(src.searchParams.get('start')).toBe('5')
    expect(f?.getAttribute('title')).toBe('Player do YouTube: Me at the zoo')
    // Nada do app por cima do player: a camada só tem o iframe, e ela fica na coluna da sala.
    const camada = screen.getByTestId('camada-musica')
    expect(camada.children).toHaveLength(1)
    expect(camada.parentElement).toBe(screen.getByTestId('sala-coluna'))
    expect(screen.getByRole('button', { name: 'Mandar a música para baixo' })).toBeTruthy()
  })

  it('o mesmo player segue a música entre a principal e a faixa (sem recarregar); fechar o tira', () => {
    estado.snap = snapCom({ tracks: [] })
    render(<SalaScreen onLeft={() => {}} />)
    const original = iframe()
    fireEvent.click(screen.getByRole('button', { name: 'Mandar a música para baixo' }))
    expect(screen.queryByTestId('celula-musica')).toBeNull()
    expect(screen.getByTestId('cartao-musica').querySelector('[data-testid="vaga-musica"]')).toBeTruthy()
    expect(iframe()).toBe(original)
    fireEvent.click(screen.getByRole('button', { name: 'Pôr a música na tela principal' }))
    expect(screen.getByTestId('celula-musica')).toBeTruthy()
    expect(iframe()).toBe(original)

    fireEvent.click(screen.getByRole('button', { name: 'Fechar a música (parar de tocar para mim)' }))
    expect(iframe()).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Ouvir a música da sala de novo' }))
    expect(screen.getByTestId('cartao-musica')).toBeTruthy()
    expect(iframe()).toBeTruthy()
  })

  it('a música desce sozinha quando alguém começa a transmitir, com o mesmo player', () => {
    estado.snap = snapCom({ tracks: [] })
    const { rerender } = render(<SalaScreen onLeft={() => {}} />)
    const original = iframe()
    estado.snap = snapCom({})
    rerender(<SalaScreen onLeft={() => {}} />)
    expect(screen.getByTestId('cartao-musica')).toBeTruthy()
    expect(iframe()).toBe(original)
  })

  it('o player conta duração, fim e erro ao núcleo (uma vez cada)', () => {
    estado.snap = snapCom({ tracks: [] })
    const report = vi.spyOn(api, 'musicReport').mockResolvedValue(undefined)
    render(<SalaScreen onLeft={() => {}} />)
    const f = iframe() as HTMLIFrameElement
    manda(f, { event: 'infoDelivery', info: { playerState: 1, currentTime: 6, duration: 19.02, videoData: { video_id: 'jNQXAC9IVRw', title: 'Me at the zoo', isLive: false } } })
    manda(f, { event: 'infoDelivery', info: { currentTime: 7, duration: 19.02 } })
    expect(report).toHaveBeenCalledTimes(1)
    expect(report).toHaveBeenLastCalledWith({ item_id: 'mu_1', duration_ms: 19_020 })
    manda(f, { event: 'onStateChange', info: 0 })
    manda(f, { event: 'onStateChange', info: 0 })
    expect(report).toHaveBeenLastCalledWith({ item_id: 'mu_1', ended: true })
    manda(f, { event: 'onError', info: 150 })
    expect(report).toHaveBeenLastCalledWith({ item_id: 'mu_1', failed: true })
    expect(report).toHaveBeenCalledTimes(3)
    // Mensagem de outra janela é ignorada.
    act(() => {
      window.dispatchEvent(new MessageEvent('message', { data: JSON.stringify({ event: 'onError', info: 100 }), source: window }))
    })
    expect(report).toHaveBeenCalledTimes(3)
  })

  it('aba Música no painel', () => {
    estado.snap = snapCom({})
    render(<SalaScreen onLeft={() => {}} />)
    fireEvent.click(screen.getByRole('tab', { name: /Música/ }))
    expect(screen.getByLabelText('Link do YouTube')).toBeTruthy()
  })
})

describe('pausar e mudar o tempo (para todos)', () => {
  /** Relógio parado em `T0`: a música do mock está em 5 s, com 19 s de duração. */
  const T0 = 1_700_000_000_000

  function comMusica(atual: Partial<NonNullable<ReturnType<typeof mockMusic>['current']>> = {}): Snapshot {
    const m = mockMusic(T0)
    if (!m.current) throw new Error('mock sem música')
    return snapCom({ tracks: [], music: { ...m, current: { ...m.current, ...atual } } })
  }

  it('pausar e os botões de 10 s mandam para a sala o item atual', () => {
    vi.useFakeTimers({ now: T0 })
    estado.snap = comMusica()
    const pausar = vi.spyOn(api, 'musicPause').mockResolvedValue(undefined)
    const buscar = vi.spyOn(api, 'musicSeek').mockResolvedValue(undefined)
    render(<SalaScreen onLeft={() => {}} />)
    expect(screen.getByTestId('tempo-musica').textContent).toBe('0:05 / 0:19')
    fireEvent.click(screen.getByRole('button', { name: 'Pausar a música para todos' }))
    expect(pausar).toHaveBeenLastCalledWith('mu_1', true)
    fireEvent.click(screen.getByRole('button', { name: 'Avançar 10 segundos para todos' }))
    expect(buscar).toHaveBeenLastCalledWith('mu_1', 15_000)
    // Voltar não passa do começo; avançar não passa do fim.
    fireEvent.click(screen.getByRole('button', { name: 'Voltar 10 segundos para todos' }))
    expect(buscar).toHaveBeenLastCalledWith('mu_1', 0)
    act(() => vi.advanceTimersByTime(10_000))
    expect(screen.getByTestId('tempo-musica').textContent).toBe('0:15 / 0:19')
    fireEvent.click(screen.getByRole('button', { name: 'Avançar 10 segundos para todos' }))
    expect(buscar).toHaveBeenLastCalledWith('mu_1', 19_000)
  })

  it('a barra de tempo busca quando a pessoa solta', () => {
    vi.useFakeTimers({ now: T0 })
    estado.snap = comMusica()
    const buscar = vi.spyOn(api, 'musicSeek').mockResolvedValue(undefined)
    render(<SalaScreen onLeft={() => {}} />)
    const barra = screen.getByRole('slider', { name: 'Tempo da música' })
    fireEvent.change(barra, { target: { value: '12000' } })
    expect(buscar).not.toHaveBeenCalled()
    expect(screen.getByTestId('tempo-musica').textContent).toBe('0:12 / 0:19')
    fireEvent.pointerUp(barra)
    expect(buscar).toHaveBeenCalledTimes(1)
    expect(buscar).toHaveBeenLastCalledWith('mu_1', 12_000)
    // Teclado: cada tecla solta busca.
    fireEvent.change(barra, { target: { value: '8000' } })
    fireEvent.keyUp(barra, { key: 'ArrowLeft' })
    expect(buscar).toHaveBeenLastCalledWith('mu_1', 8_000)
  })

  it('pausada: o tempo para, o botão continua e o player para na hora', () => {
    vi.useFakeTimers({ now: T0 })
    estado.snap = comMusica()
    const { rerender } = render(<SalaScreen onLeft={() => {}} />)
    const f = iframe() as HTMLIFrameElement
    manda(f, { event: 'infoDelivery', info: { playerState: 1, currentTime: 5 } })
    const enviar = vi.spyOn(f.contentWindow as Window, 'postMessage')
    estado.snap = comMusica({ paused: true })
    rerender(<SalaScreen onLeft={() => {}} />)
    const comandos = enviar.mock.calls.map((c) => JSON.parse(String(c[0])) as { func?: string }).map((c) => c.func)
    expect(comandos).toContain('pauseVideo')
    act(() => vi.advanceTimersByTime(10_000))
    expect(screen.getByTestId('tempo-musica').textContent).toBe('0:05 / 0:19')
    expect(screen.getByRole('button', { name: 'Continuar a música para todos' })).toBeTruthy()
    expect(screen.getByText(/pausada · pôs: Ju/)).toBeTruthy()
  })

  it('quem entra numa sala pausada carrega o player sem tocar', () => {
    estado.snap = comMusica({ paused: true })
    render(<SalaScreen onLeft={() => {}} />)
    expect(new URL(iframe()?.getAttribute('src') ?? '').searchParams.get('autoplay')).toBe('0')
  })

  it('live: só pausar; fonte antiga: sem controles de tempo', () => {
    const live = mockMusic(T0).queue[0]
    estado.snap = comMusica({ item: live })
    const { rerender } = render(<SalaScreen onLeft={() => {}} />)
    expect(screen.getByRole('button', { name: 'Pausar a música para todos' })).toBeTruthy()
    expect(screen.queryByRole('button', { name: 'Avançar 10 segundos para todos' })).toBeNull()
    expect(screen.queryByRole('slider', { name: 'Tempo da música' })).toBeNull()
    estado.snap = comMusica({ controls: false })
    rerender(<SalaScreen onLeft={() => {}} />)
    expect(screen.queryByRole('group', { name: 'Tempo da música' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Pausar a música para todos' })).toBeNull()
  })
})

describe('PlayerYoutube: aviso de anúncio', () => {
  it('liga com o tempo parado, desliga quando volta a andar e ao sair do ar', () => {
    vi.useFakeTimers()
    const onTravado = vi.fn()
    const atual = mockMusic(Date.now()).current
    if (!atual) throw new Error('sem música')
    const { unmount } = render(<PlayerYoutube atual={atual} volume={80} mudo={false} onReport={() => {}} onTravado={onTravado} />)
    const f = iframe() as HTMLIFrameElement
    manda(f, { event: 'infoDelivery', info: { playerState: 1, currentTime: 0 } })
    // Primeira amostra: ainda não dá para dizer que travou.
    act(() => vi.advanceTimersByTime(1000))
    expect(onTravado).not.toHaveBeenCalled()
    // O tempo não andou com o player "tocando": anúncio ou carregando.
    act(() => vi.advanceTimersByTime(1000))
    expect(onTravado).toHaveBeenLastCalledWith(true)
    // Voltou a andar.
    manda(f, { event: 'infoDelivery', info: { currentTime: 3 } })
    act(() => vi.advanceTimersByTime(1000))
    expect(onTravado).toHaveBeenLastCalledWith(false)
    // Travou de novo e o player saiu (fechado): o aviso desliga.
    act(() => vi.advanceTimersByTime(1000))
    expect(onTravado).toHaveBeenLastCalledWith(true)
    unmount()
    expect(onTravado).toHaveBeenLastCalledWith(false)
  })
})
