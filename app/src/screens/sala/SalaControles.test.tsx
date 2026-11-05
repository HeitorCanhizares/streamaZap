// Tarefa sala-02
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { SalaControles, type SalaControlesProps } from './SalaControles'

afterEach(() => cleanup())

function props(over: Partial<SalaControlesProps> = {}): SalaControlesProps {
  return {
    muted: false,
    volume: 50,
    camOn: true,
    sharing: false,
    smoothingMs: 120,
    onFullscreen: vi.fn(),
    onToggleMute: vi.fn(),
    onVolume: vi.fn(),
    onToggleCam: vi.fn(),
    onToggleShare: vi.fn(),
    onLeave: vi.fn(),
    ...over,
  }
}

describe('SalaControles', () => {
  it('renderiza a toolbar', () => {
    render(<SalaControles {...props()} />)
    expect(screen.getByRole('toolbar', { name: 'Controles da sala' })).toBeTruthy()
  })

  it('mudo chama onToggleMute e rótulo muda', () => {
    const p = props()
    const { unmount } = render(<SalaControles {...p} />)
    fireEvent.click(screen.getByLabelText('Silenciar a sala toda'))
    expect(p.onToggleMute).toHaveBeenCalledTimes(1)
    unmount()
    render(<SalaControles {...props({ muted: true })} />)
    expect(screen.getByLabelText('Ativar som da sala')).toBeTruthy()
  })

  it('range chama onVolume com número', () => {
    const p = props()
    render(<SalaControles {...p} />)
    fireEvent.change(screen.getByRole('slider'), { target: { value: '70' } })
    expect(p.onVolume).toHaveBeenCalledWith(70)
  })

  it('rótulos da câmera alternam', () => {
    const { unmount } = render(<SalaControles {...props({ camOn: true })} />)
    expect(screen.getByLabelText('Desligar minha câmera')).toBeTruthy()
    unmount()
    render(<SalaControles {...props({ camOn: false })} />)
    expect(screen.getByLabelText('Ligar minha câmera')).toBeTruthy()
  })

  it('compartilhar reflete sharing', () => {
    const { unmount } = render(<SalaControles {...props({ sharing: true })} />)
    expect(screen.getByRole('button', { name: 'Transmissão' }).getAttribute('aria-pressed')).toBe('true')
    unmount()
    render(<SalaControles {...props({ sharing: false })} />)
    expect(screen.getByRole('button', { name: 'Compartilhar' }).getAttribute('aria-pressed')).toBe('false')
  })

  it('suavização contém o número', () => {
    render(<SalaControles {...props({ smoothingMs: 240 })} />)
    expect(screen.getByTitle('Suavização').getAttribute('aria-label')).toContain('240')
  })

  it('tela cheia e sair chamam callbacks', () => {
    const p = props()
    render(<SalaControles {...p} />)
    fireEvent.click(screen.getByLabelText('Tela cheia (F11)'))
    fireEvent.click(screen.getByRole('button', { name: 'Sair' }))
    expect(p.onFullscreen).toHaveBeenCalledTimes(1)
    expect(p.onLeave).toHaveBeenCalledTimes(1)
  })
})
