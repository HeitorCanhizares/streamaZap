// Tarefa front-14
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { api } from '../../bridge/api'
import { mockSettings } from '../../bridge/mock'
import type { Settings } from '../../bridge/types'
import { CartaoSenhaServidor } from './CartaoSenhaServidor'

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

function comServer(): Settings {
  return {
    ...mockSettings(),
    server: { enabled: true, address: 'x.com', password: 'velha', use_relay: false },
  }
}

function preparar(atual: Settings = comServer()) {
  vi.spyOn(api, 'settingsGet').mockResolvedValue(atual)
  return vi.spyOn(api, 'settingsSet').mockResolvedValue(atual)
}

describe('CartaoSenhaServidor', () => {
  it('mostra o texto, o campo de senha e o botão desabilitado', () => {
    preparar()
    render(<CartaoSenhaServidor />)
    expect(screen.getByText('O Online está em teste fechado')).toBeTruthy()
    expect(screen.getByText('Digite a senha do servidor (pede pro dono do servidor).')).toBeTruthy()
    const campo = screen.getByLabelText('Senha do servidor') as HTMLInputElement
    expect(campo.type).toBe('password')
    expect((screen.getByRole('button', { name: 'Entrar' }) as HTMLButtonElement).disabled).toBe(true)
  })

  it('clicar em Entrar grava a senha mantendo os outros campos', async () => {
    const set = preparar()
    render(<CartaoSenhaServidor />)
    fireEvent.change(screen.getByLabelText('Senha do servidor'), { target: { value: 'nova' } })
    fireEvent.click(screen.getByRole('button', { name: 'Entrar' }))
    await waitFor(() => expect(set).toHaveBeenCalledTimes(1))
    expect(set).toHaveBeenCalledWith({
      server: { enabled: true, address: 'x.com', password: 'nova', use_relay: false },
    })
    await waitFor(() =>
      expect((screen.getByLabelText('Senha do servidor') as HTMLInputElement).value).toBe(''),
    )
  })

  it('Enter no campo também envia', async () => {
    const set = preparar()
    render(<CartaoSenhaServidor />)
    const campo = screen.getByLabelText('Senha do servidor')
    fireEvent.change(campo, { target: { value: 'nova' } })
    fireEvent.keyDown(campo, { key: 'Enter' })
    await waitFor(() => expect(set).toHaveBeenCalledTimes(1))
  })

  it('sem server nas settings usa os padrões', async () => {
    const base = mockSettings()
    delete base.server
    const set = preparar(base)
    render(<CartaoSenhaServidor />)
    fireEvent.change(screen.getByLabelText('Senha do servidor'), { target: { value: 'nova' } })
    fireEvent.click(screen.getByRole('button', { name: 'Entrar' }))
    await waitFor(() => expect(set).toHaveBeenCalledTimes(1))
    expect(set).toHaveBeenCalledWith({
      server: { enabled: true, address: '', password: 'nova', use_relay: true },
    })
  })

  it('erro ao salvar aparece em alert e reabilita o botão', async () => {
    preparar()
    vi.spyOn(api, 'settingsSet').mockRejectedValue(new Error('falhou'))
    render(<CartaoSenhaServidor />)
    fireEvent.change(screen.getByLabelText('Senha do servidor'), { target: { value: 'nova' } })
    fireEvent.click(screen.getByRole('button', { name: 'Entrar' }))
    const alerta = await screen.findByRole('alert')
    expect(alerta.textContent).toContain('falhou')
    await waitFor(() =>
      expect((screen.getByRole('button', { name: 'Entrar' }) as HTMLButtonElement).disabled).toBe(false),
    )
  })
})
