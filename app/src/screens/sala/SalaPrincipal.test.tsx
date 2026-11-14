import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react'

vi.mock('../../video/player', () => ({
  VideoPlayer: class { push() {} close() {} },
}))

import { SalaPrincipal, type SalaPrincipalProps } from './SalaPrincipal'
import type { Celula, Lugar, Stream, VolumePessoa } from './salaModel'
import type { RoomView, TileView } from '../../bridge/types'

afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.useRealTimers() })

const room: RoomView = {
  id: 'r', name: 'Sexta', mode: 'online', chat: [], unread: 0,
  participants: [
    { id: 'p1', name: 'Ana', color: '#fff', ping_ms: 10, sharing: true, camera: true },
    { id: 'p2', name: 'Bia', color: '#fff', ping_ms: 10, sharing: true, camera: false },
  ],
  tracks: [
    { id: 't1', owner_id: 'p1', kind: 'screen', title: 'Tela', number: 1 },
    { id: 't2', owner_id: 'p1', kind: 'camera', title: 'Câmera', number: 2 },
    { id: 't3', owner_id: 'p2', kind: 'window', title: 'Jogo', number: 3 },
  ],
}
const s1: Stream = { trackId: 't1', ownerId: 'p1', kind: 'screen', tile: 7 }
const s2: Stream = { trackId: 't2', ownerId: 'p1', kind: 'camera', tile: 8 }
const s3: Stream = { trackId: 't3', ownerId: 'p2', kind: 'window', tile: 9 }
const tile7: TileView = { tile: 7, track_id: 't1', layer: 1, width: 1920, height: 1080, fps: 60, kbps: 8200, ping_ms: 42, smoothing_ms: 200, reason: '' }

function montar(over: Partial<SalaPrincipalProps> = {}) {
  const fns = {
    onToggleCamera: vi.fn(), onToggleShare: vi.fn(), onFullscreen: vi.fn(), onSairTelaCheia: vi.fn(),
    onTelaCheia: vi.fn<(trackId: string | null) => void>(),
    onMover: vi.fn<(trackId: string, lugar: Lugar) => void>(),
    onVolumePessoa: vi.fn<(ownerId: string, v: VolumePessoa) => void>(),
  }
  const celulas: Celula[] = [{ stream: s1, pip: null }]
  const r = render(
    <SalaPrincipal
      room={room}
      celulas={celulas}
      tiles={[tile7]}
      canvasDe={() => document.createElement('canvas')}
      temOutras={false}
      share={null}
      volumeDe={() => ({ vol: 80, muted: false })}
      {...fns}
      {...over}
    />,
  )
  return { ...fns, container: r.container }
}

describe('SalaPrincipal', () => {
  it('titulo_e_metricas', () => {
    montar()
    const t = document.body.textContent ?? ''
    expect(t).toContain('42 ms')
    expect(screen.getByTitle('Suavização automática, 200 ms')).toBeTruthy()
    expect(t).toContain('camada 2')
  })

  it('sem transmissões na principal: texto conforme haja outras embaixo', () => {
    montar({ celulas: [] })
    expect(screen.getByText('Nenhuma transmissão na tela principal')).toBeTruthy()
    expect(screen.getByText('Quando alguém compartilhar a tela, ela aparece aqui.')).toBeTruthy()
    cleanup()
    montar({ celulas: [], temOutras: true })
    expect(screen.getByText('Escolha uma transmissão aqui embaixo.')).toBeTruthy()
  })

  it('a barra da sala não fica aqui: nada de Sair, Compartilhar ou volume geral por cima do vídeo', () => {
    montar()
    expect(screen.queryByRole('toolbar', { name: 'Controles da sala' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Sair' })).toBeNull()
    expect(screen.queryByLabelText('Silenciar a sala toda')).toBeNull()
  })

  it('a altura que fica fora do vídeo soma a da faixa de baixo', () => {
    const reserva = () => /100vh - (\d+)px/.exec(screen.getByTestId('palco').getAttribute('style') ?? '')?.[1]
    montar()
    expect(reserva()).toBe('236')
    cleanup()
    montar({ alturaFaixa: 180 })
    expect(reserva()).toBe('416')
    cleanup()
    montar({ alturaFaixa: 216.4 })
    expect(reserva()).toBe('452')
  })

  it('sem os cantos verdes por cima da imagem', () => {
    const { container } = montar()
    expect(container.querySelectorAll('[data-corner]')).toHaveLength(0)
  })

  it('botao_parar', () => {
    montar({ share: { source: 's', audio: 'none', preset: '720p30', encoder: 'x', layers: [], viewers_per_layer: [], encode_ms: 1, conversion: [] } })
    expect(screen.getAllByRole('button', { name: 'Parar' }).length).toBeGreaterThan(0)
  })

  it('atalhos: C, F11, F (tela cheia da transmissão) e M (som da transmissão)', () => {
    const f = montar()
    fireEvent.keyDown(window, { key: 'c' })
    expect(f.onToggleCamera).toHaveBeenCalled()
    fireEvent.keyDown(window, { key: 'F11' })
    expect(f.onFullscreen).toHaveBeenCalled()
    fireEvent.keyDown(window, { key: 'f' })
    expect(f.onTelaCheia).toHaveBeenCalledWith('t1')
    fireEvent.keyDown(window, { key: 'M' })
    expect(f.onVolumePessoa).toHaveBeenCalledWith('p1', { vol: 80, muted: true })
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(f.onSairTelaCheia).toHaveBeenCalled()
  })

  it('atalhos com Ctrl são do sistema (Ctrl+C não liga a câmera)', () => {
    const f = montar()
    fireEvent.keyDown(window, { key: 'c', ctrlKey: true })
    fireEvent.keyDown(window, { key: 'f', ctrlKey: true })
    expect(f.onToggleCamera).not.toHaveBeenCalled()
    expect(f.onTelaCheia).not.toHaveBeenCalled()
  })

  it('M não mexe no som quando só a minha transmissão está na tela', () => {
    const f = montar({ celulas: [{ stream: { ...s1, mine: true }, pip: null }] })
    fireEvent.keyDown(window, { key: 'm' })
    expect(f.onVolumePessoa).not.toHaveBeenCalled()
  })

  it('estrutura', () => {
    montar()
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('Sexta')
    expect(screen.getByText(/AO VIVO/)).toBeTruthy()
  })

  it('o canvas da transmissão fica na célula', () => {
    const canvas = document.createElement('canvas')
    montar({ canvasDe: () => canvas })
    expect(screen.getByRole('img', { name: 'Tela de Ana' }).contains(canvas)).toBe(true)
  })

  it('fechar, mandar para baixo e silenciar a pessoa', () => {
    const f = montar()
    fireEvent.click(screen.getByRole('button', { name: 'Fechar tela de Ana' }))
    expect(f.onMover).toHaveBeenCalledWith('t1', 'fechada')
    fireEvent.click(screen.getByRole('button', { name: 'Mandar tela de Ana para baixo' }))
    expect(f.onMover).toHaveBeenCalledWith('t1', 'faixa')
    fireEvent.click(screen.getByRole('button', { name: 'Silenciar Ana' }))
    expect(f.onVolumePessoa).toHaveBeenCalledWith('p1', { vol: 80, muted: true })
  })

  it('volume da pessoa na própria transmissão', () => {
    const f = montar()
    const linha = screen.getByTestId('controles-transmissao')
    fireEvent.change(within(linha).getByLabelText('Volume de Ana'), { target: { value: '35' } })
    expect(f.onVolumePessoa).toHaveBeenCalledWith('p1', { vol: 35, muted: false })
    expect(linha.textContent).toContain('80%')
  })

  it('a minha transmissão não tem som para ajustar, só a tela cheia', () => {
    montar({ celulas: [{ stream: { ...s1, mine: true }, pip: null }] })
    expect(screen.queryByLabelText(/^Volume de/)).toBeNull()
    expect(screen.getByRole('button', { name: 'Pôr minha tela em tela cheia' })).toBeTruthy()
  })

  it('várias transmissões: uma célula para cada, lado a lado', () => {
    montar({ celulas: [{ stream: s1, pip: null }, { stream: s3, pip: null }] })
    expect(screen.getAllByTestId('celula-principal')).toHaveLength(2)
    expect(screen.getByRole('img', { name: 'Janela de Bia' })).toBeTruthy()
    // Cada uma com o próprio som e a própria tela cheia.
    expect(screen.getByLabelText('Volume de Ana')).toBeTruthy()
    expect(screen.getByLabelText('Volume de Bia')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Pôr janela de Bia em tela cheia' })).toBeTruthy()
  })

  it('câmera do dono no canto da tela; o X manda a câmera para baixo', () => {
    const f = montar({ celulas: [{ stream: s1, pip: s2 }] })
    expect(screen.getByRole('img', { name: 'Câmera de Ana' })).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Tirar a câmera de Ana do canto' }))
    expect(f.onMover).toHaveBeenCalledWith('t2', 'faixa')
  })

  it('repassa invite ao cabecalho', () => {
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true })
    montar({ invite: 'Entra na sala "Sexta" no streamaZap: código ZAP-7K2Q' })
    fireEvent.click(screen.getByRole('button', { name: 'Copiar convite' }))
    expect(writeText).toHaveBeenCalledWith('Entra na sala "Sexta" no streamaZap: código ZAP-7K2Q')
  })

  it('controles da transmissão somem com o mouse parado e voltam ao mexer', () => {
    vi.useFakeTimers()
    montar()
    const linha = screen.getByTestId('controles-transmissao')
    expect(linha.style.opacity).toBe('1')
    act(() => { vi.advanceTimersByTime(3000) })
    expect(linha.style.opacity).toBe('0')
    fireEvent.pointerMove(screen.getByTestId('palco'))
    expect(linha.style.opacity).toBe('1')
  })

  it('mouse em cima dos controles da transmissão: não somem', () => {
    vi.useFakeTimers()
    montar()
    const linha = screen.getByTestId('controles-transmissao')
    fireEvent.pointerEnter(linha)
    act(() => { vi.advanceTimersByTime(5000) })
    expect(linha.style.opacity).toBe('1')
    fireEvent.pointerLeave(linha)
    act(() => { vi.advanceTimersByTime(3000) })
    expect(linha.style.opacity).toBe('0')
  })

  describe('tela cheia de uma transmissão', () => {
    it('o botão da transmissão pede a tela cheia dela', () => {
      const f = montar()
      fireEvent.click(screen.getByRole('button', { name: 'Pôr tela de Ana em tela cheia' }))
      expect(f.onTelaCheia).toHaveBeenCalledWith('t1')
    })

    it('em tela cheia: ela sozinha por cima de tudo, com o canvas, o som e o botão de sair', () => {
      const canvas = document.createElement('canvas')
      const f = montar({ celulas: [{ stream: s1, pip: s2 }, { stream: s3, pip: null }], telaCheia: 't1', canvasDe: (id) => (id === 't1' ? canvas : document.createElement('canvas')) })
      const cheia = screen.getByRole('dialog', { name: 'Transmissão em tela cheia' })
      expect(cheia.parentElement).toBe(document.body)
      expect(cheia.style.position).toBe('fixed')
      expect(within(cheia).getByRole('img', { name: 'Tela de Ana' }).contains(canvas)).toBe(true)
      // A câmera do canto vai junto; mandar para baixo e fechar não aparecem na tela cheia.
      expect(within(cheia).getByRole('img', { name: 'Câmera de Ana' })).toBeTruthy()
      expect(within(cheia).queryByRole('button', { name: 'Fechar tela de Ana' })).toBeNull()
      expect(within(cheia).queryByRole('button', { name: 'Tirar a câmera de Ana do canto' })).toBeNull()
      expect(within(cheia).getByLabelText('Volume de Ana')).toBeTruthy()
      // Na grade fica só o lugar dela; a outra continua.
      expect(screen.getByTestId('celula-em-tela-cheia').textContent).toContain('Em tela cheia')
      expect(screen.getByRole('img', { name: 'Janela de Bia' })).toBeTruthy()
      fireEvent.click(within(cheia).getByRole('button', { name: 'Sair da tela cheia' }))
      expect(f.onTelaCheia).toHaveBeenCalledWith(null)
    })

    it('Esc e F saem da tela cheia; M silencia quem está nela', () => {
      const f = montar({ celulas: [{ stream: s1, pip: null }, { stream: s3, pip: null }], telaCheia: 't3' })
      fireEvent.keyDown(window, { key: 'm' })
      expect(f.onVolumePessoa).toHaveBeenCalledWith('p2', { vol: 80, muted: true })
      fireEvent.keyDown(window, { key: 'Escape' })
      expect(f.onTelaCheia).toHaveBeenLastCalledWith(null)
      expect(f.onSairTelaCheia).not.toHaveBeenCalled()
      f.onTelaCheia.mockClear()
      fireEvent.keyDown(window, { key: 'F' })
      expect(f.onTelaCheia).toHaveBeenCalledWith(null)
    })

    it('trilha em tela cheia que não está na principal: nada por cima', () => {
      montar({ telaCheia: 'outra' })
      expect(screen.queryByRole('dialog')).toBeNull()
      expect(screen.queryByTestId('celula-em-tela-cheia')).toBeNull()
    })
  })

  describe('câmera: canto e jeito', () => {
    const pos = (celula: Celula, nomeCam = 'Câmera de Ana') => {
      montar({ celulas: [celula] })
      return (screen.getByRole('img', { name: nomeCam }).parentElement as HTMLElement).style
    }
    const vazio = (v: string) => v === '' || v === 'auto'

    it('posiciona o pip pelo canto (embaixo, acima da linha de controles)', () => {
      let st = pos({ stream: s1, pip: s2, corner: 'sup_esq' })
      expect(st.left).toBe('26px'); expect(st.top).not.toBe(''); expect(vazio(st.right)).toBe(true); expect(vazio(st.bottom)).toBe(true)
      cleanup()
      st = pos({ stream: s1, pip: s2, corner: 'sup_dir' })
      expect(st.right).toBe('26px'); expect(st.top).not.toBe(''); expect(vazio(st.left)).toBe(true); expect(vazio(st.bottom)).toBe(true)
      cleanup()
      st = pos({ stream: s1, pip: s2, corner: 'inf_esq' })
      expect(st.left).toBe('26px'); expect(st.bottom).not.toBe(''); expect(vazio(st.right)).toBe(true); expect(vazio(st.top)).toBe(true)
      cleanup()
      st = pos({ stream: s1, pip: s2, corner: 'inf_dir' })
      expect(st.right).toBe('26px'); expect(st.bottom).toBe('74px'); expect(vazio(st.left)).toBe(true); expect(vazio(st.top)).toBe(true)
      cleanup()
      st = pos({ stream: s1, pip: s2 })
      expect(st.right).toBe('26px'); expect(st.bottom).toBe('74px'); expect(vazio(st.left)).toBe(true); expect(vazio(st.top)).toBe(true)
    })

    it('Soltar a câmera só existe com onJeito', () => {
      const onJeito = vi.fn<(trackId: string, jeito: 'canto' | 'solta') => void>()
      montar({ celulas: [{ stream: s1, pip: s2 }], onJeito })
      fireEvent.click(screen.getByRole('button', { name: 'Soltar a câmera de Ana' }))
      expect(onJeito).toHaveBeenCalledWith('t2', 'solta')
      cleanup()
      montar({ celulas: [{ stream: s1, pip: s2 }] })
      expect(screen.queryByRole('button', { name: 'Soltar a câmera de Ana' })).toBeNull()
    })

    it('Pôr a câmera no canto aparece só com podeCanto e onJeito', () => {
      const onJeito = vi.fn<(trackId: string, jeito: 'canto' | 'solta') => void>()
      montar({ celulas: [{ stream: s2, pip: null, podeCanto: true }], onJeito })
      fireEvent.click(screen.getByRole('button', { name: 'Pôr a câmera de Ana no canto' }))
      expect(onJeito).toHaveBeenCalledWith('t2', 'canto')
      cleanup()
      montar({ celulas: [{ stream: s2, pip: null }], onJeito })
      expect(screen.queryByRole('button', { name: /no canto/ })).toBeNull()
      cleanup()
      montar({ celulas: [{ stream: s2, pip: null, podeCanto: true }] })
      expect(screen.queryByRole('button', { name: /no canto/ })).toBeNull()
    })

    it('minha câmera usa os rótulos em primeira pessoa', () => {
      const onJeito = vi.fn<(trackId: string, jeito: 'canto' | 'solta') => void>()
      const minha: Stream = { ...s2, ownerId: 'p1', mine: true }
      const tela: Stream = { ...s1, mine: true }
      montar({ celulas: [{ stream: tela, pip: minha }], onJeito })
      expect(screen.getByRole('button', { name: 'Soltar minha câmera' })).toBeTruthy()
      cleanup()
      montar({ celulas: [{ stream: minha, pip: null, podeCanto: true }], onJeito })
      expect(screen.getByRole('button', { name: 'Pôr minha câmera no canto' })).toBeTruthy()
    })
  })
})
