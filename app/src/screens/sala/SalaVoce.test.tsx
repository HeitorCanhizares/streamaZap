import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'

import { mockSnapshot } from '../../bridge/mock'
import { SalaVoce, type SalaVoceProps } from './SalaVoce'
import type { Lugar, Stream } from './salaModel'

afterEach(() => { cleanup(); vi.restoreAllMocks() })

const room = mockSnapshot().room!
const tela: Stream = { trackId: 'minha-tela', ownerId: 'eu', kind: 'screen', tile: 40, mine: true }
const cam: Stream = { trackId: 'minha-cam', ownerId: 'eu', kind: 'camera', tile: 41, mine: true }

function montar(over: Partial<SalaVoceProps> = {}) {
  const onMover = vi.fn<(trackId: string, lugar: Lugar) => void>()
  const canvases = new Map<string, HTMLCanvasElement>()
  const canvasDe = (id: string) => {
    let c = canvases.get(id)
    if (!c) { c = document.createElement('canvas'); canvases.set(id, c) }
    return c
  }
  render(<SalaVoce room={room} meId="eu" camOn={false} sharing={false} canvasDe={canvasDe} onMover={onMover} {...over} />)
  return { onMover, canvasDe }
}

describe('SalaVoce', () => {
  it('renderiza titulo Você e os botoes Câmera e Tela', () => {
    const cam = vi.fn()
    const share = vi.fn()
    montar({ onToggleCam: cam, onToggleShare: share })
    expect(screen.getByText('Você')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Câmera' }))
    fireEvent.click(screen.getByRole('button', { name: 'Tela' }))
    expect(cam).toHaveBeenCalled()
    expect(share).toHaveBeenCalled()
  })

  it('sem transmitir e só o som: avisos', () => {
    montar()
    expect(screen.getByText('Você não está transmitindo')).toBeTruthy()
    cleanup()
    montar({ sharing: true })
    expect(screen.getByText('Transmitindo só o som')).toBeTruthy()
  })

  it('mostra a minha tela com a câmera no canto, nos canvases das transmissões', () => {
    const { canvasDe } = montar({ sharing: true, camOn: true, tela, cam })
    expect(screen.getByRole('img', { name: 'Prévia: sua tela' }).contains(canvasDe('minha-tela'))).toBe(true)
    expect(screen.getByRole('img', { name: 'Prévia: câmera' }).contains(canvasDe('minha-cam'))).toBe(true)
  })

  it('pôr a minha tela na tela principal', () => {
    const { onMover } = montar({ sharing: true, tela })
    fireEvent.click(screen.getByRole('button', { name: 'Pôr minha tela na tela principal' }))
    expect(onMover).toHaveBeenCalledWith('minha-tela', 'principal')
  })

  it('só a câmera: sobe a câmera', () => {
    const { onMover } = montar({ camOn: true, cam })
    expect(screen.getByRole('img', { name: 'Prévia: sua câmera' })).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Pôr minha câmera na tela principal' }))
    expect(onMover).toHaveBeenCalledWith('minha-cam', 'principal')
  })

  it('na tela principal: aviso no cartão e sem prévia aqui', () => {
    montar({ sharing: true, tela, lugares: { 'minha-tela': 'principal' } })
    expect(screen.getByText('Sua transmissão está na tela principal')).toBeTruthy()
    expect(screen.queryByRole('img', { name: 'Prévia: sua tela' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Pôr minha tela na tela principal' })).toBeNull()
  })

  it('esconder e mostrar a prévia', () => {
    const { onMover } = montar({ sharing: true, camOn: true, tela, cam })
    fireEvent.click(screen.getByRole('button', { name: 'Esconder minha prévia' }))
    expect(onMover).toHaveBeenCalledWith('minha-tela', 'fechada')
    expect(onMover).toHaveBeenCalledWith('minha-cam', 'fechada')
    cleanup()
    const r = montar({ sharing: true, tela, lugares: { 'minha-tela': 'fechada' } })
    expect(screen.getByText('Prévia escondida')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Mostrar minha prévia' }))
    expect(r.onMover).toHaveBeenCalledWith('minha-tela', 'faixa')
  })
})
