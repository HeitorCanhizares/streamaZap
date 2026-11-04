// Tarefa config-05
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { ConfigSom } from './ConfigSom'
import type { Settings } from '../../bridge/types'

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

describe('ConfigSom', () => {
  it('renderiza título, slider e a legenda dos três avisos', () => {
    render(<ConfigSom settings={ajustes()} onChange={vi.fn()} />)
    expect(screen.getByRole('heading', { name: 'Som e avisos' })).toBeTruthy()
    expect(screen.getByLabelText('Volume inicial de cada transmissão')).toBeTruthy()
    expect(screen.getByText('Aquele plim quando alguém manda algo no chat.')).toBeTruthy()
    expect(screen.getByText('Notificação do Windows com o nome da sala.')).toBeTruthy()
    expect(screen.getByText('O som volta quando você abrir a janela de novo.')).toBeTruthy()
  })

  it('mostra o volume em % e grava o slider como fração', () => {
    const onChange = vi.fn()
    const ajuste = ajustes()
    render(<ConfigSom settings={ajuste} onChange={onChange} />)
    expect(screen.getByText('80%')).toBeTruthy()
    fireEvent.change(screen.getByLabelText('Volume inicial de cada transmissão'), { target: { value: '55' } })
    expect(onChange).toHaveBeenCalledTimes(1)
    const patch = onChange.mock.calls[0][0]
    expect(patch.sound.initial_volume).toBeCloseTo(0.55)
    expect(patch.sound.message_sound).toBe(ajuste.sound.message_sound)
  })

  it('mostra 55% quando settings tem volume 0.55', () => {
    render(<ConfigSom settings={ajustes({ sound: { initial_volume: 0.55, message_sound: true, notify_room_open: true, mute_when_minimized: false } })} onChange={vi.fn()} />)
    expect(screen.getByText('55%')).toBeTruthy()
  })

  it('alterna Som de nova mensagem e reflete aria-checked', () => {
    const onChange = vi.fn()
    const ajuste = ajustes()
    render(<ConfigSom settings={ajuste} onChange={onChange} />)
    const sw = screen.getByRole('switch', { name: /Som de nova mensagem/ })
    expect(sw.getAttribute('aria-checked')).toBe('true')
    fireEvent.click(sw)
    expect(onChange).toHaveBeenCalledWith({ sound: { ...ajuste.sound, message_sound: false } })
  })

  it('alterna Silenciar quando minimizar (desligado vira ligado)', () => {
    const onChange = vi.fn()
    const ajuste = ajustes()
    render(<ConfigSom settings={ajuste} onChange={onChange} />)
    const sw = screen.getByRole('switch', { name: /Silenciar quando minimizar/ })
    expect(sw.getAttribute('aria-checked')).toBe('false')
    fireEvent.click(sw)
    expect(onChange).toHaveBeenCalledWith({ sound: { ...ajuste.sound, mute_when_minimized: true } })
  })

  it('renderiza Atalhos com as quatro teclas e descrições', () => {
    render(<ConfigSom settings={ajustes()} onChange={vi.fn()} />)
    expect(screen.getByRole('heading', { name: 'Atalhos' })).toBeTruthy()
    for (const tecla of ['F11', 'Esc', 'M', 'C']) {
      expect(screen.getByText(tecla, { selector: 'kbd' })).toBeTruthy()
    }
    expect(screen.getByText('Tela cheia (ou duplo clique no vídeo)')).toBeTruthy()
    expect(screen.getByText('Sair da tela cheia')).toBeTruthy()
    expect(screen.getByText('Silenciar a sala toda')).toBeTruthy()
    expect(screen.getByText('Ligar ou desligar sua câmera')).toBeTruthy()
  })
})
