// Barra da sala (dock no pé da coluna): só o que é da sala.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { SalaControles, type SalaControlesProps } from './SalaControles'

afterEach(() => cleanup())

function props(over: Partial<SalaControlesProps> = {}): SalaControlesProps {
  return {
    camOn: true,
    sharing: false,
    onFullscreen: vi.fn(),
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

  it('não tem volume geral nem suavização: o som é de cada transmissão', () => {
    render(<SalaControles {...props()} />)
    expect(screen.queryByRole('slider')).toBeNull()
    expect(screen.queryByLabelText('Silenciar a sala toda')).toBeNull()
    expect(screen.queryByText('auto')).toBeNull()
  })

  it('rótulos da câmera alternam e o clique chama onToggleCam', () => {
    const p = props({ camOn: true })
    const { unmount } = render(<SalaControles {...p} />)
    fireEvent.click(screen.getByLabelText('Desligar minha câmera'))
    expect(p.onToggleCam).toHaveBeenCalledTimes(1)
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

  it('tela cheia do app e sair chamam callbacks', () => {
    const p = props()
    render(<SalaControles {...p} />)
    fireEvent.click(screen.getByLabelText('Tela cheia do app (F11)'))
    fireEvent.click(screen.getByRole('button', { name: 'Sair' }))
    expect(p.onFullscreen).toHaveBeenCalledTimes(1)
    expect(p.onLeave).toHaveBeenCalledTimes(1)
  })

  it('fica no fluxo da página, não por cima do vídeo', () => {
    render(<SalaControles {...props()} />)
    expect(screen.getByRole('toolbar', { name: 'Controles da sala' }).style.position).toBe('')
  })
})
