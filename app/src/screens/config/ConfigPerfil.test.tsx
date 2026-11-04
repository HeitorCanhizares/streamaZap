// Tarefa ui-23
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { ConfigPerfil } from './ConfigPerfil'
import type { Settings } from '../../bridge/types'

afterEach(() => cleanup())

function ajustes(over: Partial<Settings> = {}): Settings {
  return {
    name: 'marcao',
    color: '#3BF08A',
    install_id: 'x',
    open_in: 'game' as Settings['open_in'],
    local: { interface: null, port: 0, announce: false },
    video: { camera_id: null, mirror_preview: false, default_preset: 'medio' as Settings['video']['default_preset'], smoothing_auto: true, smoothing_ms: 0 },
    sound: { initial_volume: 50, message_sound: true, notify_room_open: true, mute_when_minimized: false },
    update: { auto: true, channel: 'estavel' },
    errors: [],
    ...over,
  } as Settings
}

describe('ConfigPerfil', () => {
  it('nome: digitar no campo Nome chama onChange({name})', () => {
    const onChange = vi.fn()
    const { container } = render(<ConfigPerfil settings={ajustes()} onChange={onChange} />)
    const input = container.querySelector('input') as HTMLInputElement
    fireEvent.change(input, { target: { value: 'X' } })
    expect(onChange).toHaveBeenCalledWith({ name: 'X' })
  })

  it('cor: clicar numa cor chama onChange({color})', () => {
    const onChange = vi.fn()
    render(<ConfigPerfil settings={ajustes()} onChange={onChange} />)
    fireEvent.click(screen.getByRole('radio', { name: '#5CD6FF' }))
    expect(onChange).toHaveBeenCalledWith({ color: '#5CD6FF' })
  })

  it('cor: a cor atual fica marcada com aria-checked', () => {
    render(<ConfigPerfil settings={ajustes({ color: '#FFB547' })} onChange={vi.fn()} />)
    expect(screen.getByRole('radio', { name: '#FFB547' }).getAttribute('aria-checked')).toBe('true')
    expect(screen.getByRole('radio', { name: '#3BF08A' }).getAttribute('aria-checked')).toBe('false')
  })

  it('previa: o texto do nome atual aparece na prévia do chat', () => {
    render(<ConfigPerfil settings={ajustes({ name: 'ana' })} onChange={vi.fn()} />)
    expect(screen.getByText('ana')).toBeTruthy()
  })

  it('auto_update: alternar chama onChange com update.auto invertido e mantém channel', () => {
    const onChange = vi.fn()
    render(<ConfigPerfil settings={ajustes({ update: { auto: true, channel: 'beta' } })} onChange={onChange} />)
    fireEvent.click(screen.getByRole('switch'))
    expect(onChange).toHaveBeenCalledWith({ update: { auto: false, channel: 'beta' } })
  })

  it('canal: trocar para Beta chama onChange com update.channel beta', () => {
    const onChange = vi.fn()
    const { container } = render(<ConfigPerfil settings={ajustes()} onChange={onChange} />)
    const select = container.querySelector('select') as HTMLSelectElement
    fireEvent.change(select, { target: { value: 'beta' } })
    expect(onChange).toHaveBeenCalledWith({ update: { auto: true, channel: 'beta' } })
  })

  it('não muta settings', () => {
    const s = ajustes()
    const copia = JSON.stringify(s)
    render(<ConfigPerfil settings={s} onChange={vi.fn()} />)
    fireEvent.click(screen.getByRole('switch'))
    expect(JSON.stringify(s)).toBe(copia)
  })
})
