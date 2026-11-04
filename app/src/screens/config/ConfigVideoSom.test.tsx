// Tarefa ui-25
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { ConfigVideoSom } from './ConfigVideoSom'
import type { Settings, Sources } from '../../bridge/types'

afterEach(() => cleanup())

function ajustes(over: Partial<Settings> = {}): Settings {
  return {
    name: 'marcao',
    color: '#3BF08A',
    install_id: 'x',
    open_in: 'local' as Settings['open_in'],
    local: { interface: null, port: 47800, announce: false },
    video: { camera_id: null, mirror_preview: false, default_preset: '720p30', smoothing_auto: false, smoothing_ms: 200 },
    sound: { initial_volume: 0.8, message_sound: true, notify_room_open: true, mute_when_minimized: false },
    update: { auto: true, channel: 'estavel' },
    errors: [],
    ...over,
  } as Settings
}

const fontes: Sources = {
  screens: [],
  windows: [],
  cameras: [
    { id: 'cam-1', kind: 'camera', name: 'Câmera integrada' },
    { id: 'cam-2', kind: 'camera', name: 'Webcam USB' },
  ],
  apps: [],
  encoders: [],
}

describe('ConfigVideoSom', () => {
  it('camera: escolher Nenhuma manda camera_id null', () => {
    const onChange = vi.fn()
    const ajuste = ajustes({ video: { ...ajustes().video, camera_id: 'cam-1' } })
    render(<ConfigVideoSom settings={ajuste} onChange={onChange} sources={fontes} />)
    const select = within(screen.getByRole('group', { name: 'Câmera' })).getByRole('combobox')
    fireEvent.change(select, { target: { value: '' } })
    expect(onChange).toHaveBeenCalledWith({ video: { ...ajuste.video, camera_id: null } })
  })

  it('camera: escolher uma câmera do mock manda o id', () => {
    const onChange = vi.fn()
    const ajuste = ajustes()
    render(<ConfigVideoSom settings={ajuste} onChange={onChange} sources={fontes} />)
    const select = within(screen.getByRole('group', { name: 'Câmera' })).getByRole('combobox')
    fireEvent.change(select, { target: { value: 'cam-2' } })
    expect(onChange).toHaveBeenCalledWith({ video: { ...ajuste.video, camera_id: 'cam-2' } })
  })

  it('preset: clicar 1080p · 60 chama onChange com 1080p60 preservando o resto de video', () => {
    const onChange = vi.fn()
    const ajuste = ajustes()
    render(<ConfigVideoSom settings={ajuste} onChange={onChange} sources={fontes} />)
    fireEvent.click(screen.getByRole('button', { name: '1080p · 60' }))
    expect(onChange).toHaveBeenCalledWith({ video: { ...ajuste.video, default_preset: '1080p60' } })
  })

  it('suavizacao_slider: com smoothing_auto=false mudar o slider chama onChange com smoothing_ms', () => {
    const onChange = vi.fn()
    const ajuste = ajustes()
    render(<ConfigVideoSom settings={ajuste} onChange={onChange} sources={fontes} />)
    const slider = within(screen.getByRole('group', { name: 'Suavização' })).getByRole('slider')
    fireEvent.change(slider, { target: { value: '400' } })
    expect(onChange).toHaveBeenCalledWith({ video: { ...ajuste.video, smoothing_ms: 400 } })
  })

  it('suavizacao_slider: com smoothing_auto=true mudar o slider não chama onChange', () => {
    const onChange = vi.fn()
    render(<ConfigVideoSom settings={ajustes({ video: { ...ajustes().video, smoothing_auto: true } })} onChange={onChange} sources={fontes} />)
    const slider = within(screen.getByRole('group', { name: 'Suavização' })).getByRole('slider')
    fireEvent.change(slider, { target: { value: '400' } })
    expect(onChange).not.toHaveBeenCalled()
  })

  it('suavizacao: mostra o valor em ms', () => {
    render(<ConfigVideoSom settings={ajustes()} onChange={vi.fn()} sources={fontes} />)
    expect(screen.getByText('200 ms')).toBeTruthy()
  })

  it('volume: slider 50 grava initial_volume 0.5', () => {
    const onChange = vi.fn()
    const ajuste = ajustes()
    render(<ConfigVideoSom settings={ajuste} onChange={onChange} sources={fontes} />)
    const slider = within(screen.getByRole('group', { name: 'Volume inicial de cada transmissão' })).getByRole('slider')
    fireEvent.change(slider, { target: { value: '50' } })
    expect(onChange).toHaveBeenCalledWith({ sound: { ...ajuste.sound, initial_volume: 0.5 } })
  })

  it('toggles: alternar Som de mensagem inverte message_sound', () => {
    const onChange = vi.fn()
    const ajuste = ajustes()
    render(<ConfigVideoSom settings={ajuste} onChange={onChange} sources={fontes} />)
    fireEvent.click(screen.getByRole('switch', { name: 'Som de mensagem' }))
    expect(onChange).toHaveBeenCalledWith({ sound: { ...ajuste.sound, message_sound: false } })
  })

  it('atalhos: texto F11 aparece', () => {
    render(<ConfigVideoSom settings={ajustes()} onChange={vi.fn()} sources={fontes} />)
    expect(screen.getByText('F11')).toBeTruthy()
  })
})
