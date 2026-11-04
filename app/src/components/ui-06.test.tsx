// Tarefa ui-06
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { VideoTile } from './VideoTile'

afterEach(() => cleanup())

describe('VideoTile', () => {
  it('renderiza o rótulo e um canvas', () => {
    const { container } = render(<VideoTile label="marcao" color="#3bf08a" />)
    expect(screen.getByText('marcao')).toBeTruthy()
    expect(container.querySelector('canvas')).not.toBeNull()
  })

  it('entrega o elemento canvas ao canvasRef', () => {
    const canvasRef = vi.fn()
    const { container } = render(<VideoTile label="ana" canvasRef={canvasRef} />)
    const canvas = container.querySelector('canvas')
    expect(canvasRef).toHaveBeenCalledWith(canvas)
  })

  it('clicar em Fechar chama onClose', () => {
    const onClose = vi.fn()
    render(<VideoTile label="ana" onClose={onClose} />)
    fireEvent.click(screen.getByLabelText('Fechar'))
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('clicar em Mutar chama onMute', () => {
    const onMute = vi.fn()
    render(<VideoTile label="ana" onMute={onMute} />)
    fireEvent.click(screen.getByLabelText('Mutar'))
    expect(onMute).toHaveBeenCalledTimes(1)
  })

  it('sem onClose não existe botão Fechar', () => {
    render(<VideoTile label="ana" />)
    expect(screen.queryByLabelText('Fechar')).toBeNull()
    expect(screen.queryByText('Fechar')).toBeNull()
  })
})
