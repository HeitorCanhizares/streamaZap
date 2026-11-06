import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { ConfigConexao } from './ConfigConexao'
import { api } from '../../bridge/api'
import { mockSettings, mockSnapshot } from '../../bridge/mock'
import { resetSnapshotForTests, setSnapshot } from '../../bridge/store'
import type { NetworkStatus, Settings } from '../../bridge/types'

const rede: NetworkStatus = { radmin_ip: null, interfaces: ['Ethernet'] }

function ajustes(enabled: boolean): Settings {
  return { ...mockSettings(), server: { enabled, address: '', password: '', use_relay: true } }
}

function montar(enabled: boolean, onChange = vi.fn()) {
  render(<ConfigConexao settings={ajustes(enabled)} onChange={onChange} network={rede} />)
  return onChange
}

function entrarEmSalaOnline(): void {
  const snap = mockSnapshot()
  setSnapshot({ ...snap, room: { id: 'r', name: 'Sala', mode: 'online', participants: [], tracks: [], chat: [], unread: 0 } })
}

beforeEach(() => resetSnapshotForTests())
afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
  resetSnapshotForTests()
})

describe('ConfigConexao: modo Online', () => {
  it('desligado: sem campos do servidor e interruptor desmarcado', () => {
    montar(false)
    expect(screen.queryByLabelText('Endereço do servidor')).toBeNull()
    expect(screen.queryByLabelText('Senha do servidor')).toBeNull()
    expect(screen.getByRole('switch', { name: 'Usar o modo Online' }).getAttribute('aria-checked')).toBe('false')
  })

  it('ligar chama onChange com enabled true', () => {
    const onChange = montar(false)
    fireEvent.click(screen.getByRole('switch', { name: 'Usar o modo Online' }))
    expect(onChange).toHaveBeenCalledWith({ server: { enabled: true, address: '', password: '', use_relay: true } })
  })

  it('ligado: mostra os campos e o interruptor marcado', () => {
    montar(true)
    expect(screen.getByLabelText('Endereço do servidor')).toBeTruthy()
    expect(screen.getByLabelText('Senha do servidor')).toBeTruthy()
    expect(screen.getByRole('switch', { name: 'Usar o modo Online' }).getAttribute('aria-checked')).toBe('true')
  })

  it('desligar sem sala Online desliga direto', () => {
    setSnapshot({ ...mockSnapshot(), room: null })
    const onChange = montar(true)
    fireEvent.click(screen.getByRole('switch', { name: 'Usar o modo Online' }))
    expect(onChange).toHaveBeenCalledWith({ server: { enabled: false, address: '', password: '', use_relay: true } })
  })

  it('desligar em sala Online pede confirmação, cancela e confirma', async () => {
    entrarEmSalaOnline()
    const leave = vi.spyOn(api, 'roomLeave').mockResolvedValue()
    const onChange = montar(true)
    fireEvent.click(screen.getByRole('switch', { name: 'Usar o modo Online' }))
    expect(screen.getByRole('alertdialog', { name: 'Desligar o modo Online' })).toBeTruthy()
    expect(onChange).not.toHaveBeenCalled()
    fireEvent.click(screen.getByText('Cancelar'))
    expect(screen.queryByRole('alertdialog')).toBeNull()
    expect(onChange).not.toHaveBeenCalled()

    fireEvent.click(screen.getByRole('switch', { name: 'Usar o modo Online' }))
    fireEvent.click(screen.getByText('Desligar e sair'))
    await waitFor(() => expect(onChange).toHaveBeenCalledWith({ server: { enabled: false, address: '', password: '', use_relay: true } }))
    expect(leave).toHaveBeenCalled()
  })

  it('roomLeave falhando mostra o erro e não desliga', async () => {
    entrarEmSalaOnline()
    vi.spyOn(api, 'roomLeave').mockRejectedValue(new Error('sem rede'))
    const onChange = montar(true)
    fireEvent.click(screen.getByRole('switch', { name: 'Usar o modo Online' }))
    fireEvent.click(screen.getByText('Desligar e sair'))
    expect((await screen.findByRole('alert')).textContent).toContain('sem rede')
    expect(onChange).not.toHaveBeenCalled()
  })
})
