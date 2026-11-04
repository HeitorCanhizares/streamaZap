// Tarefa shell-05
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { TitleBar } from '../TitleBar'

afterEach(() => cleanup())

describe('TitleBar', () => {
  it('marca: mostra streamaZap e o ícone', () => {
    const { container } = render(<TitleBar />)
    const header = container.querySelector('header')!
    expect(header.textContent).toContain('streamaZap')
    expect(container.querySelector('img[src="/icone.png"]')).not.toBeNull()
  })

  it('drag_region: o header tem data-tauri-drag-region e o botão Fechar não', () => {
    const { container } = render(<TitleBar />)
    expect(container.querySelector('header')!.hasAttribute('data-tauri-drag-region')).toBe(true)
    expect(screen.getByRole('button', { name: 'Fechar' }).hasAttribute('data-tauri-drag-region')).toBe(false)
  })

  it('versao_sem_crumb: mostra a versão; com crumb a versão some e aparece o crumb', () => {
    const { unmount } = render(<TitleBar version="v3.0.0-alpha.0" />)
    expect(screen.getByText('v3.0.0-alpha.0')).toBeTruthy()
    unmount()

    render(<TitleBar version="v3.0.0-alpha.0" crumb={{ atual: 'Configurações' }} />)
    expect(screen.queryByText('v3.0.0-alpha.0')).toBeNull()
    expect(screen.getByText('Configurações')).toBeTruthy()
  })

  it('crumb_com_pai: mostra "Salas / Nova sala"', () => {
    const { container } = render(<TitleBar crumb={{ pai: 'Salas', atual: 'Nova sala' }} />)
    expect(container.querySelector('header')!.textContent).toContain('Salas / Nova sala')
  })

  it('status_padrao: mostra o status do servidor e do Radmin', () => {
    render(<TitleBar serverMs={42} radminIp="26.12.34.56" />)
    expect(screen.getByText('Servidor online · 42 ms')).toBeTruthy()
    expect(screen.getByText('Radmin · 26.12.34.56')).toBeTruthy()
  })

  it('server_online_com_ping: server online mostra o ping vindo do snapshot', () => {
    render(<TitleBar server={{ state: 'online', ping_ms: 30 }} radminIp="26.12.34.56" />)
    expect(screen.getByText('Servidor online · 30 ms')).toBeTruthy()
  })

  it('server_erro: server em erro mostra desconectado', () => {
    render(<TitleBar server={{ state: 'error' }} radminIp="26.12.34.56" />)
    expect(screen.getByText('Servidor · desconectado')).toBeTruthy()
  })

  it('right_substitui: right substitui o bloco de status', () => {
    render(<TitleBar right={<span>XYZ</span>} />)
    expect(screen.getByText('XYZ')).toBeTruthy()
    expect(screen.queryByText(/Servidor online/)).toBeNull()
  })

  it('fechar_chama_onClose', () => {
    const onClose = vi.fn()
    render(<TitleBar onClose={onClose} />)
    fireEvent.click(screen.getByRole('button', { name: 'Fechar' }))
    expect(onClose).toHaveBeenCalledTimes(1)
  })
})
