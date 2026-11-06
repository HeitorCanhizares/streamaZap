import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'

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
    onSmoothingAuto: vi.fn(), onToggleCamera: vi.fn(), onToggleShare: vi.fn(), onLeave: vi.fn(), onFullscreen: vi.fn(),
    onToggleMute: vi.fn(),
    onMover: vi.fn<(trackId: string, lugar: Lugar) => void>(),
    onVolumePessoa: vi.fn<(ownerId: string, v: VolumePessoa) => void>(),
  }
  const celulas: Celula[] = [{ stream: s1, pip: null }]
  render(
    <SalaPrincipal
      room={room}
      celulas={celulas}
      tiles={[tile7]}
      canvasDe={() => document.createElement('canvas')}
      temOutras={false}
      share={null}
      smoothingAuto
      volumeDe={() => ({ vol: 80, muted: false })}
      {...fns}
      {...over}
    />,
  )
  return fns
}

describe('SalaPrincipal', () => {
  it('titulo_e_metricas', () => {
    montar()
    const t = document.body.textContent ?? ''
    expect(t).toContain('42 ms')
    expect(screen.getByLabelText('Suavização automática, 200 milissegundos')).toBeTruthy()
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

  it('botoes', () => {
    const f = montar()
    fireEvent.click(screen.getByText('Sair'))
    expect(f.onLeave).toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: /^Compartilhar$/ }))
    expect(f.onToggleShare).toHaveBeenCalled()
  })

  it('botao_parar', () => {
    montar({ share: { source: 's', audio: 'none', preset: '720p30', encoder: 'x', layers: [], viewers_per_layer: [], encode_ms: 1, conversion: [] } })
    expect(screen.getAllByRole('button', { name: 'Parar' }).length).toBeGreaterThan(0)
  })

  it('atalhos: C, F11 e M (mudo geral)', () => {
    const f = montar()
    fireEvent.keyDown(window, { key: 'c' })
    expect(f.onToggleCamera).toHaveBeenCalled()
    fireEvent.keyDown(window, { key: 'F11' })
    expect(f.onFullscreen).toHaveBeenCalled()
    fireEvent.keyDown(window, { key: 'M' })
    expect(f.onToggleMute).toHaveBeenCalled()
  })

  it('estrutura', () => {
    montar()
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('Sexta')
    expect(screen.getByText(/AO VIVO/)).toBeTruthy()
    expect(screen.getByRole('toolbar', { name: 'Controles da sala' })).toBeTruthy()
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

  it('várias transmissões: uma célula para cada, lado a lado', () => {
    montar({ celulas: [{ stream: s1, pip: null }, { stream: s3, pip: null }] })
    expect(screen.getAllByTestId('celula-principal')).toHaveLength(2)
    expect(screen.getByRole('img', { name: 'Janela de Bia' })).toBeTruthy()
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

  it('controles somem com o mouse parado e voltam ao mexer', () => {
    vi.useFakeTimers()
    montar()
    const barra = screen.getByRole('toolbar', { name: 'Controles da sala' })
    expect(barra.getAttribute('data-visivel')).toBe('true')
    act(() => { vi.advanceTimersByTime(3000) })
    expect(barra.getAttribute('data-visivel')).toBe('false')
    expect(barra.style.opacity).toBe('0')
    fireEvent.pointerMove(screen.getByTestId('palco'))
    expect(barra.getAttribute('data-visivel')).toBe('true')
  })

  it('mouse em cima dos controles: não somem', () => {
    vi.useFakeTimers()
    montar()
    const barra = screen.getByRole('toolbar', { name: 'Controles da sala' })
    fireEvent.pointerEnter(barra)
    act(() => { vi.advanceTimersByTime(5000) })
    expect(barra.getAttribute('data-visivel')).toBe('true')
    fireEvent.pointerLeave(barra)
    expect(barra.getAttribute('data-visivel')).toBe('false')
  })

  describe('câmera: canto e jeito', () => {
    const pos = (celula: Celula, nomeCam = 'Câmera de Ana') => {
      montar({ celulas: [celula] })
      return (screen.getByRole('img', { name: nomeCam }).parentElement as HTMLElement).style
    }
    const vazio = (v: string) => v === '' || v === 'auto'

    it('posiciona o pip pelo canto', () => {
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
      expect(st.right).toBe('26px'); expect(st.bottom).toBe('96px'); expect(vazio(st.left)).toBe(true); expect(vazio(st.top)).toBe(true)
      cleanup()
      st = pos({ stream: s1, pip: s2 })
      expect(st.right).toBe('26px'); expect(st.bottom).toBe('96px'); expect(vazio(st.left)).toBe(true); expect(vazio(st.top)).toBe(true)
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
