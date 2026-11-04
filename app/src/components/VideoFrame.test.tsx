// Tarefa previa-07
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { VideoFrame } from './VideoFrame'

afterEach(() => cleanup())

describe('VideoFrame', () => {
  it('renderiza children', () => {
    render(<VideoFrame><span>conteudo</span></VideoFrame>)
    expect(screen.getByText('conteudo')).toBeTruthy()
  })

  it('por padrão há 4 cantos', () => {
    const { container } = render(<VideoFrame />)
    expect(container.querySelectorAll('[data-corner]').length).toBe(4)
  })

  it('com corners={false} não há cantos', () => {
    const { container } = render(<VideoFrame corners={false} />)
    expect(container.querySelectorAll('[data-corner]').length).toBe(0)
  })

  it('radius=12 gera borderRadius 12px', () => {
    render(<VideoFrame radius={12} />)
    const frame = screen.getByTestId('video-frame') as HTMLElement
    expect(frame.style.borderRadius).toBe('12px')
  })

  it('style do chamador sobrescreve o padrão', () => {
    render(<VideoFrame style={{ maxWidth: 480 }} />)
    const frame = screen.getByTestId('video-frame') as HTMLElement
    expect(frame.style.maxWidth).toBe('480px')
  })
})
