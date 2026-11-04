// Tarefa ui-24
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { ConfigConexao } from './ConfigConexao'
import type { NetworkStatus, Settings } from '../../bridge/types'

afterEach(() => cleanup())

function ajustes(over: Partial<Settings> = {}): Settings {
  return {
    name: 'marcao',
    color: '#3BF08A',
    install_id: 'x',
    open_in: 'local' as Settings['open_in'],
    local: { interface: null, port: 47800, announce: false },
    video: { camera_id: null, mirror_preview: false, default_preset: 'medio' as Settings['video']['default_preset'], smoothing_auto: true, smoothing_ms: 0 },
    sound: { initial_volume: 50, message_sound: true, notify_room_open: true, mute_when_minimized: false },
    update: { auto: true, channel: 'estavel' },
    errors: [],
    ...over,
  } as Settings
}

const rede: NetworkStatus = { radmin_ip: null, interfaces: ['Radmin VPN', 'Ethernet'] }

describe('ConfigConexao', () => {
  it('online_desativado: o texto em breve aparece e clicar na opção Online não chama onChange', () => {
    const onChange = vi.fn()
    render(<ConfigConexao settings={ajustes()} onChange={onChange} network={rede} />)
    expect(screen.getAllByText(/em breve/).length).toBeGreaterThan(0)
    fireEvent.click(screen.getByRole('button', { name: 'Online · em breve' }))
    expect(onChange).not.toHaveBeenCalled()
  })

  it('interface: trocar o Select para uma interface chama onChange com local.interface preenchido', () => {
    const onChange = vi.fn()
    const { container } = render(<ConfigConexao settings={ajustes()} onChange={onChange} network={rede} />)
    const select = container.querySelector('select') as HTMLSelectElement
    fireEvent.change(select, { target: { value: 'Ethernet' } })
    expect(onChange).toHaveBeenCalledWith({ local: { interface: 'Ethernet', port: 47800, announce: false } })
  })

  it('interface: escolher Todas as redes manda null', () => {
    const onChange = vi.fn()
    const { container } = render(
      <ConfigConexao settings={ajustes({ local: { interface: 'Ethernet', port: 47800, announce: false } })} onChange={onChange} network={rede} />,
    )
    const select = container.querySelector('select') as HTMLSelectElement
    fireEvent.change(select, { target: { value: '' } })
    expect(onChange).toHaveBeenCalledWith({ local: { interface: null, port: 47800, announce: false } })
  })

  it('porta_valida: digitar 48000 chama onChange com port 48000', () => {
    const onChange = vi.fn()
    const { container } = render(<ConfigConexao settings={ajustes()} onChange={onChange} network={rede} />)
    const input = container.querySelector('input') as HTMLInputElement
    fireEvent.change(input, { target: { value: '48000' } })
    expect(onChange).toHaveBeenCalledWith({ local: { interface: null, port: 48000, announce: false } })
  })

  it('porta_valida: digitar abc não chama onChange', () => {
    const onChange = vi.fn()
    const { container } = render(<ConfigConexao settings={ajustes()} onChange={onChange} network={rede} />)
    const input = container.querySelector('input') as HTMLInputElement
    fireEvent.change(input, { target: { value: 'abc' } })
    expect(onChange).not.toHaveBeenCalled()
  })

  it('porta_fora: 70000 é ignorada', () => {
    const onChange = vi.fn()
    const { container } = render(<ConfigConexao settings={ajustes()} onChange={onChange} network={rede} />)
    const input = container.querySelector('input') as HTMLInputElement
    fireEvent.change(input, { target: { value: '70000' } })
    expect(onChange).not.toHaveBeenCalled()
  })

  it('anunciar: alternar chama onChange com announce invertido', () => {
    const onChange = vi.fn()
    render(<ConfigConexao settings={ajustes({ local: { interface: null, port: 47800, announce: false } })} onChange={onChange} network={rede} />)
    fireEvent.click(screen.getByRole('switch'))
    expect(onChange).toHaveBeenCalledWith({ local: { interface: null, port: 47800, announce: true } })
  })

  it('radmin: network.radmin_ip mostra Radmin com o IP', () => {
    render(<ConfigConexao settings={ajustes()} onChange={vi.fn()} network={{ ...rede, radmin_ip: '26.1.2.3' }} />)
    expect(screen.getByText('Radmin 26.1.2.3')).toBeTruthy()
  })

  it('não muta settings', () => {
    const s = ajustes()
    const copia = JSON.stringify(s)
    render(<ConfigConexao settings={s} onChange={vi.fn()} network={rede} />)
    fireEvent.click(screen.getByRole('switch'))
    expect(JSON.stringify(s)).toBe(copia)
  })
})
