// Tarefa config-04
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'

vi.mock('../../video/player', () => ({
  VideoPlayer: class { push() {} close() {} },
}))

import { api } from '../../bridge/api'
import { ConfigVideo } from './ConfigVideo'
import { mockSettings, mockSources } from '../../bridge/mock'
import type { Settings, SourceItem } from '../../bridge/types'

afterEach(() => { cleanup(); vi.restoreAllMocks() })

const tela = () => within(screen.getByRole('radiogroup', { name: 'Qualidade padrão das suas salas' }))
const cam = () => within(screen.getByRole('radiogroup', { name: 'Qualidade padrão da câmera' }))

function com(video: Partial<Settings['video']>): Settings {
  const s = mockSettings()
  return { ...s, video: { ...s.video, ...video } }
}

describe('ConfigVideo', () => {
  it('renderiza título e os 4 presets', () => {
    render(<ConfigVideo settings={mockSettings()} onChange={vi.fn()} sources={mockSources()} />)
    expect(screen.getByText('Vídeo e câmera')).toBeTruthy()
    expect(tela().getAllByRole('radio')).toHaveLength(4)
    expect(screen.getAllByText('30 fps · Radmin').length).toBeGreaterThan(0)
  })

  it('clicar no 2º preset manda 720p60', () => {
    const onChange = vi.fn()
    const s = mockSettings()
    render(<ConfigVideo settings={s} onChange={onChange} sources={mockSources()} />)
    fireEvent.click(tela().getAllByRole('radio')[1])
    expect(onChange).toHaveBeenCalledWith({ video: { ...s.video, default_preset: '720p60' } })
  })

  it('marca o preset atual', () => {
    render(<ConfigVideo settings={com({ default_preset: '720p30' })} onChange={vi.fn()} sources={mockSources()} />)
    const r = tela().getAllByRole('radio')
    expect(r[0].getAttribute('aria-checked')).toBe('true')
    expect(r[1].getAttribute('aria-checked')).toBe('false')
  })

  it('card 1080p 30 fps seleciona 1080p30 ao clicar', () => {
    const onChange = vi.fn()
    const s = mockSettings()
    render(<ConfigVideo settings={s} onChange={onChange} sources={mockSources()} />)
    fireEvent.click(tela().getAllByRole('radio')[2])
    expect(onChange).toHaveBeenCalledWith({ video: { ...s.video, default_preset: '1080p30' } })
  })

  it('default_preset 1080p30 marca o 3o card e nao o 4o', () => {
    render(<ConfigVideo settings={com({ default_preset: '1080p30' })} onChange={vi.fn()} sources={mockSources()} />)
    const r = tela().getAllByRole('radio')
    expect(r[2].getAttribute('aria-checked')).toBe('true')
    expect(r[3].getAttribute('aria-checked')).toBe('false')
  })

  it('default_preset 1080p60 marca o 4o card', () => {
    render(<ConfigVideo settings={com({ default_preset: '1080p60' })} onChange={vi.fn()} sources={mockSources()} />)
    const r = tela().getAllByRole('radio')
    expect(r[3].getAttribute('aria-checked')).toBe('true')
    expect(r[2].getAttribute('aria-checked')).toBe('false')
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
    vi.spyOn(api, 'cameraPreview').mockResolvedValue(undefined)
    render(<ConfigVideo settings={com({ mirror_preview: true })} onChange={onChange} sources={mockSources()} />)
    fireEvent.click(screen.getByRole('button', { name: 'Testar câmera' }))
    expect((screen.getByTestId('camera-preview') as HTMLElement).style.transform).toBe('scaleX(-1)')
    fireEvent.click(screen.getByRole('switch', { name: /Espelhar/ }))
    expect(onChange.mock.calls[0][0].video.mirror_preview).toBe(false)
  })

  it('prévia da câmera: só liga no botão, abre a câmera escolhida e fecha no Parar', () => {
    const abrir = vi.spyOn(api, 'cameraPreview').mockResolvedValue(undefined)
    const fechar = vi.spyOn(api, 'cameraPreviewStop').mockResolvedValue(undefined)
    render(<ConfigVideo settings={com({ camera_id: 'camera:0' })} onChange={vi.fn()} sources={mockSources()} />)
    expect(abrir).not.toHaveBeenCalled()
    expect(screen.queryByTestId('camera-preview')).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Testar câmera' }))
    expect(screen.getByLabelText('Prévia ao vivo da sua câmera')).toBeTruthy()
    expect(abrir).toHaveBeenCalledWith('camera:0', expect.any(Number), expect.any(Function))
    const tile = abrir.mock.calls[0][1]
    fireEvent.click(screen.getByRole('button', { name: 'Parar' }))
    expect(fechar).toHaveBeenCalledWith(tile)
    expect(screen.getByRole('button', { name: 'Testar câmera' })).toBeTruthy()
  })

  it('prévia da câmera: erro do núcleo aparece e volta ao botão', async () => {
    vi.spyOn(api, 'cameraPreview').mockRejectedValue(new Error('nenhuma câmera encontrada'))
    vi.spyOn(api, 'cameraPreviewStop').mockResolvedValue(undefined)
    render(<ConfigVideo settings={mockSettings()} onChange={vi.fn()} sources={mockSources()} />)
    fireEvent.click(screen.getByRole('button', { name: 'Testar câmera' }))
    await waitFor(() => expect(screen.getByRole('status').textContent).toContain('nenhuma câmera encontrada'))
    expect(screen.getByRole('button', { name: 'Testar câmera' })).toBeTruthy()
  })

  it('sair da tela fecha a prévia', () => {
    vi.spyOn(api, 'cameraPreview').mockResolvedValue(undefined)
    const fechar = vi.spyOn(api, 'cameraPreviewStop').mockResolvedValue(undefined)
    const { unmount } = render(<ConfigVideo settings={mockSettings()} onChange={vi.fn()} sources={mockSources()} />)
    fireEvent.click(screen.getByRole('button', { name: 'Testar câmera' }))
    unmount()
    expect(fechar).toHaveBeenCalledTimes(1)
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

  it('qualidade da câmera: mostra o rótulo e o grupo com 4 radios', () => {
    render(<ConfigVideo settings={mockSettings()} onChange={vi.fn()} sources={mockSources()} />)
    expect(screen.getByText('Qualidade padrão da câmera')).toBeTruthy()
    expect(cam().getAllByRole('radio')).toHaveLength(4)
    expect(tela().getAllByRole('radio')).toHaveLength(4)
  })

  it('qualidade da câmera: clicar em 1080p30 manda camera_preset', () => {
    const onChange = vi.fn()
    const s = mockSettings()
    render(<ConfigVideo settings={s} onChange={onChange} sources={mockSources()} />)
    fireEvent.click(cam().getAllByRole('radio')[2])
    expect(onChange).toHaveBeenCalledWith({ video: { ...s.video, camera_preset: '1080p30' } })
  })

  it('qualidade da câmera: câmera 720p30 desliga os outros três', () => {
    const src = mockSources()
    const c720: SourceItem = { ...src.cameras[0], max_height: 720, max_fps: 30 }
    render(<ConfigVideo settings={mockSettings()} onChange={vi.fn()} sources={{ ...src, cameras: [c720] }} />)
    const r = cam().getAllByRole('radio') as HTMLButtonElement[]
    expect(r[0].disabled).toBe(false)
    expect(r[1].disabled).toBe(true)
    expect(r[2].disabled).toBe(true)
    expect(r[3].disabled).toBe(true)
  })

  it('preset guardado que a câmera não faz é ajustado uma vez', () => {
    const onChange = vi.fn()
    const src = mockSources()
    const c720: SourceItem = { ...src.cameras[0], max_height: 720, max_fps: 30 }
    const s = com({ camera_preset: '1080p60' })
    render(<ConfigVideo settings={s} onChange={onChange} sources={{ ...src, cameras: [c720] }} />)
    expect(onChange).toHaveBeenCalledTimes(1)
    expect(onChange).toHaveBeenCalledWith({ video: { ...s.video, camera_preset: '720p30' } })
  })

  it('câmera que faz 1080p60 não dispara ajuste sozinho', () => {
    const onChange = vi.fn()
    const src = mockSources()
    const c: SourceItem = { ...src.cameras[0], max_height: 1080, max_fps: 60 }
    render(<ConfigVideo settings={com({ camera_preset: '1080p60' })} onChange={onChange} sources={{ ...src, cameras: [c] }} />)
    expect(onChange).not.toHaveBeenCalled()
  })
})
