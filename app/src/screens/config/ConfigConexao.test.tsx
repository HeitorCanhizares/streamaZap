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
    open_in: 'online',
    local: { interface: null, port: 47800, announce: false },
    video: { camera_id: null, mirror_preview: false, default_preset: '720p30', smoothing_auto: true, smoothing_ms: 0 },
    sound: { initial_volume: 50, message_sound: true, notify_room_open: true, mute_when_minimized: false },
    update: { auto: true, channel: 'estavel' },
    ...over,
  } as Settings
}

const rede: NetworkStatus = { radmin_ip: null, interfaces: ['Radmin VPN', 'Ethernet'] }

function montar(onChange = vi.fn(), network = rede, s = ajustes()) {
  return render(<ConfigConexao settings={s} onChange={onChange} network={network} />)
}

describe('ConfigConexao', () => {
  it('renderiza título e textos fixos', () => {
    montar()
    expect(screen.getByText('Conexão')).toBeTruthy()
    expect(screen.getByText('Abrir o app em')).toBeTruthy()
    expect(screen.getByText('Online · servidor')).toBeTruthy()
    expect(screen.getByText('Endereço do servidor')).toBeTruthy()
    expect(screen.getByText('Senha do servidor')).toBeTruthy()
    expect(screen.getByText('Barra bots e quem não foi convidado.')).toBeTruthy()
    expect(screen.getByText('Testar conexão')).toBeTruthy()
    expect(screen.getByText('Ainda não testado')).toBeTruthy()
    expect(screen.getByText('Usar o servidor como ponte')).toBeTruthy()
    expect(screen.getByText('Local · Radmin e rede de casa')).toBeTruthy()
    expect(screen.getByText('Rede usada para as salas')).toBeTruthy()
    expect(screen.getByText('Porta da transmissão')).toBeTruthy()
    expect(screen.getByText('Anunciar minhas salas')).toBeTruthy()
  })

  it('clicar em Local chama onChange com open_in', () => {
    const onChange = vi.fn()
    montar(onChange)
    fireEvent.click(screen.getByRole('radio', { name: 'Local' }))
    expect(onChange).toHaveBeenCalledWith({ open_in: 'local' })
  })

  it('porta: 47801 chama onChange e letras são filtradas', () => {
    const onChange = vi.fn()
    montar(onChange)
    const input = screen.getByLabelText('Porta da transmissão') as HTMLInputElement
    fireEvent.change(input, { target: { value: '47801' } })
    expect(onChange).toHaveBeenCalledWith({ local: { interface: null, port: 47801, announce: false } })
    onChange.mockClear()
    fireEvent.change(input, { target: { value: 'abc' } })
    expect(onChange).not.toHaveBeenCalled()
    expect(input.value).toBe('')
  })

  it('anunciar inverte local.announce', () => {
    const onChange = vi.fn()
    montar(onChange)
    fireEvent.click(screen.getByRole('switch', { name: /Anunciar minhas salas/ }))
    expect(onChange).toHaveBeenCalledWith({ local: { interface: null, port: 47800, announce: true } })
  })

  it('renderiza sem settings.server (undefined) sem erro', () => {
    const s = ajustes()
    delete (s as Partial<Settings>).server
    montar(vi.fn(), rede, s)
    expect((screen.getByLabelText('Endereço do servidor') as HTMLInputElement).value).toBe('')
    expect((screen.getByLabelText('Senha do servidor') as HTMLInputElement).value).toBe('')
  })

  it('renderiza com server.address do mock sem erro', () => {
    montar(vi.fn(), rede, ajustes({ server: { address: 'zap.x.com:7443', password: 'segredo', use_relay: true } }))
    expect((screen.getByLabelText('Endereço do servidor') as HTMLInputElement).value).toBe('zap.x.com:7443')
  })

  it('editar endereço chama onChange com { server: { address } }', () => {
    const onChange = vi.fn()
    montar(onChange, rede, ajustes({ server: { address: 'antigo', password: 'p', use_relay: false } }))
    fireEvent.change(screen.getByLabelText('Endereço do servidor'), { target: { value: 'zap.x.com:7443' } })
    expect(onChange).toHaveBeenCalledWith({ server: { address: 'zap.x.com:7443', password: 'p', use_relay: false } })
  })

  it('teste de conexão: fail sem campos, ok com endereço e senha, volta a idle ao editar', () => {
    const onChange = vi.fn()
    const { unmount } = montar(onChange)
    fireEvent.click(screen.getByText('Testar conexão'))
    expect(screen.getByText('Preencha o endereço e a senha')).toBeTruthy()
    unmount()
    montar(onChange, rede, ajustes({ server: { address: 'zap.x.com:7443', password: 'segredo', use_relay: true } }))
    fireEvent.click(screen.getByText('Testar conexão'))
    expect(screen.getByText('Conectado · senha aceita · 42 ms')).toBeTruthy()
    fireEvent.change(screen.getByLabelText('Endereço do servidor'), { target: { value: 'outro.com' } })
    expect(screen.getByText('Ainda não testado')).toBeTruthy()
  })

  it('mostra network.radmin_ip', () => {
    montar(vi.fn(), { ...rede, radmin_ip: '26.1.2.3' })
    expect(screen.getByText('26.1.2.3')).toBeTruthy()
  })
})
