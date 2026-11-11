import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'

const travasMock = vi.hoisted(() => ({ podeTransmitir: true }))
vi.mock('../../bridge/usePlano', () => ({
  useTravas: () => ({ ativo: true, tela: [], camera: [], podeTransmitir: travasMock.podeTransmitir, podeEntrar: true }),
}))
vi.mock('../../video/player', () => ({
  VideoPlayer: class { push() {} close() {} },
}))

import { SalaControles, type SalaControlesProps } from './SalaControles'
import { SalaPrincipal } from './SalaPrincipal'
import type { RoomView } from '../../bridge/types'

afterEach(() => { cleanup(); travasMock.podeTransmitir = true })

function montar(over: Partial<SalaControlesProps> = {}) {
  const fns = { onToggleCam: vi.fn(), onToggleShare: vi.fn() }
  render(
    <SalaControles
      muted={false} volume={80} camOn={false} sharing={false} smoothingMs={200}
      onFullscreen={vi.fn()} onToggleMute={vi.fn()} onVolume={vi.fn()} onLeave={vi.fn()}
      {...fns} {...over}
    />,
  )
  return fns
}

describe('SalaControles com plano sem transmissão', () => {
  it('mostra Vire Transmissor e abre o painel', () => {
    const f = montar({ podeTransmitir: false })
    fireEvent.click(screen.getByRole('button', { name: 'Vire Transmissor' }))
    expect(f.onToggleShare).toHaveBeenCalledTimes(1)
  })

  it('trava a câmera', () => {
    const f = montar({ podeTransmitir: false })
    const cam = screen.getByRole('button', { name: 'Ligar minha câmera' })
    expect(cam.getAttribute('aria-disabled')).toBe('true')
    expect(cam.getAttribute('title')).toBe('Vire Transmissor para ligar a câmera')
    fireEvent.click(cam)
    expect(f.onToggleCam).not.toHaveBeenCalled()
  })

  it('transmitindo, não mostra Vire Transmissor', () => {
    montar({ podeTransmitir: false, sharing: true })
    expect(screen.queryByText('Vire Transmissor')).toBeNull()
  })

  it('sem a prop, tudo como hoje', () => {
    const f = montar()
    fireEvent.click(screen.getByRole('button', { name: 'Ligar minha câmera' }))
    expect(f.onToggleCam).toHaveBeenCalledTimes(1)
    expect(screen.getByRole('button', { name: 'Compartilhar' })).toBeTruthy()
  })
})

describe('tecla C na SalaPrincipal', () => {
  const room: RoomView = { id: 'r', name: 'S', mode: 'online', chat: [], unread: 0, participants: [], tracks: [] }
  function montarSala() {
    const onToggleCamera = vi.fn()
    render(
      <SalaPrincipal
        room={room} celulas={[]} tiles={[]} canvasDe={() => document.createElement('canvas')} temOutras={false}
        share={null} smoothingAuto volumeDe={() => ({ vol: 80, muted: false })} onVolumePessoa={vi.fn()}
        onMover={vi.fn()} onSmoothingAuto={vi.fn()} onToggleCamera={onToggleCamera} onToggleShare={vi.fn()}
        onLeave={vi.fn()} onFullscreen={vi.fn()}
      />,
    )
    return onToggleCamera
  }

  it('não liga a câmera quando não pode transmitir', () => {
    travasMock.podeTransmitir = false
    const cam = montarSala()
    fireEvent.keyDown(window, { key: 'c' })
    expect(cam).not.toHaveBeenCalled()
  })

  it('liga a câmera quando pode transmitir', () => {
    const cam = montarSala()
    fireEvent.keyDown(window, { key: 'c' })
    expect(cam).toHaveBeenCalledTimes(1)
  })
})
