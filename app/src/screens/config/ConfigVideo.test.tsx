// Tarefa config-04
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { ConfigVideo } from './ConfigVideo'
import { mockSettings, mockSources } from '../../bridge/mock'
import type { Settings } from '../../bridge/types'

afterEach(() => cleanup())

function com(video: Partial<Settings['video']>): Settings {
  const s = mockSettings()
  return { ...s, video: { ...s.video, ...video } }
}

describe('ConfigVideo', () => {
  it('renderiza título e os 4 presets', () => {
    render(<ConfigVideo settings={mockSettings()} onChange={vi.fn()} sources={mockSources()} />)
    expect(screen.getByText('Vídeo e câmera')).toBeTruthy()
    expect(screen.getAllByRole('radio')).toHaveLength(4)
    expect(screen.getByText('30 fps · Radmin')).toBeTruthy()
    expect(screen.getAllByText('1080p')).toHaveLength(2)
  })

  it('clicar no 2º preset manda 720p60', () => {
    const onChange = vi.fn()
    const s = mockSettings()
    render(<ConfigVideo settings={s} onChange={onChange} sources={mockSources()} />)
    fireEvent.click(screen.getAllByRole('radio')[1])
    expect(onChange).toHaveBeenCalledWith({ video: { ...s.video, default_preset: '720p60' } })
  })

  it('marca o preset atual', () => {
    render(<ConfigVideo settings={com({ default_preset: '720p30' })} onChange={vi.fn()} sources={mockSources()} />)
    const r = screen.getAllByRole('radio')
    expect(r[0].getAttribute('aria-checked')).toBe('true')
    expect(r[1].getAttribute('aria-checked')).toBe('false')
  })

  it('mudar câmera manda camera_id', () => {
    const onChange = vi.fn()
    const src = mockSources()
    render(<ConfigVideo settings={mockSettings()} onChange={onChange} sources={src} />)
    fireEvent.change(screen.getByLabelText('Câmera'), { target: { value: src.cameras[0].id } })
    expect(onChange.mock.calls[0][0].video.camera_id).toBe(src.cameras[0].id)
  })

  it('alternar espelho inverte mirror_preview', () => {
    const onChange = vi.fn()
    render(<ConfigVideo settings={com({ mirror_preview: true })} onChange={onChange} sources={mockSources()} />)
    expect((screen.getByTestId('camera-preview') as HTMLElement).style.transform).toBe('scaleX(-1)')
    fireEvent.click(screen.getByRole('switch', { name: /Espelhar/ }))
    expect(onChange.mock.calls[0][0].video.mirror_preview).toBe(false)
  })

  it('suavização: auto desabilita o slider e mostra "auto"', () => {
    render(<ConfigVideo settings={com({ smoothing_auto: true })} onChange={vi.fn()} sources={mockSources()} />)
    expect((screen.getByLabelText('Suavização fixa em milissegundos') as HTMLInputElement).disabled).toBe(true)
    expect(screen.getByText('auto')).toBeTruthy()
  })

  it('suavização manual mostra ms e permite mudar', () => {
    const onChange = vi.fn()
    render(<ConfigVideo settings={com({ smoothing_auto: false, smoothing_ms: 120 })} onChange={onChange} sources={mockSources()} />)
    expect(screen.getByText('120 ms')).toBeTruthy()
    fireEvent.change(screen.getByLabelText('Suavização fixa em milissegundos'), { target: { value: '300' } })
    expect(onChange.mock.calls[0][0].video.smoothing_ms).toBe(300)
  })
})
