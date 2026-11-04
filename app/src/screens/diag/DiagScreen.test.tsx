// Tarefa diag-06
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { mockSnapshot } from '../../bridge/mock'
import { useSnapshot } from '../../bridge/store'
import { api } from '../../bridge/api'
import { mockDiagReport } from '../../bridge/mockDiag'
import { DiagScreen } from './DiagScreen'

vi.mock('../../bridge/store', () => ({ useSnapshot: vi.fn() }))
vi.mock('../../bridge/api', () => ({
  api: { diagnosticsRun: vi.fn(), diagnosticsText: vi.fn(), firewallFix: vi.fn() },
}))

const run = vi.mocked(api.diagnosticsRun)
const texto = vi.mocked(api.diagnosticsText)
const fix = vi.mocked(api.firewallFix)

function relFw(bloqueado: boolean) {
  const r = mockDiagReport()
  return { ...r, firewall: { ...r.firewall, bloqueado } }
}

const mocked = vi.mocked(useSnapshot)


beforeEach(() => {
  mocked.mockReturnValue(mockSnapshot())
  run.mockReset().mockResolvedValue(mockDiagReport())
  texto.mockReset().mockResolvedValue('RELATORIO X')
  fix.mockReset().mockResolvedValue({ ok: true, mensagem: 'Firewall corrigido.', removidas: 1, perfis_liberados: [] })
})

afterEach(() => {
  cleanup()
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

describe('DiagScreen', () => {
  it('renderiza_relatorio', async () => {
    render(<DiagScreen />)
    expect(await screen.findByText('Radmin VPN · 26.100.5.20')).toBeTruthy()
    for (const t of ['Tá tudo conectando?', 'Rede', 'Amigos com o app aberto', 'Salas encontradas', 'Servidor online', 'Firewall do Windows', 'Como ler isso', 'Copiar relatório']) {
      expect(screen.getAllByText(t).length).toBeGreaterThan(0)
    }
    expect(screen.getByText('Servidor não configurado')).toBeTruthy()
  })

  it('cinco_cards', async () => {
    const { container } = render(<DiagScreen />)
    await screen.findByText('Radmin VPN · 26.100.5.20')
    expect(container.querySelectorAll('section[data-estado]').length).toBe(5)
  })

  it('sem_rede', async () => {
    const r = mockDiagReport()
    run.mockResolvedValue({ ...r, network: { interfaces: [], radmin_ip: undefined } })
    render(<DiagScreen />)
    expect(await screen.findByText('Sem rede')).toBeTruthy()
  })

  it('verificando_dura_a_chamada', async () => {
    render(<DiagScreen />)
    await screen.findByText('Radmin VPN · 26.100.5.20')
    let resolve: (v: ReturnType<typeof mockDiagReport>) => void = () => {}
    run.mockReturnValueOnce(new Promise((r) => { resolve = r }))
    fireEvent.click(screen.getByText('Verificar de novo'))
    expect(await screen.findByText('Verificando sua conexão…')).toBeTruthy()
    resolve(mockDiagReport())
    await waitFor(() => expect(screen.queryByText('Verificando sua conexão…')).toBeNull())
  })

  it('copiar_usa_diagnosticsText', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined)
    vi.stubGlobal('navigator', { ...navigator, clipboard: { writeText } })
    render(<DiagScreen />)
    fireEvent.click(screen.getByText('Copiar relatório'))
    expect(await screen.findByText('Copiado!')).toBeTruthy()
    expect(writeText).toHaveBeenCalledWith('RELATORIO X')
  })

  it('copiar_fallback', async () => {
    texto.mockRejectedValue(new Error('falhou'))
    const writeText = vi.fn().mockResolvedValue(undefined)
    vi.stubGlobal('navigator', { ...navigator, clipboard: { writeText } })
    render(<DiagScreen />)
    fireEvent.click(screen.getByText('Copiar relatório'))
    expect(await screen.findByText('Copiado!')).toBeTruthy()
    expect(writeText).toHaveBeenCalledWith(expect.stringMatching(/^streamaZap · diagnóstico/))
  })

  it('corrigir_firewall', async () => {
    run.mockResolvedValue(relFw(true))
    render(<DiagScreen />)
    fireEvent.click(await screen.findByRole('button', { name: 'Corrigir firewall' }))
    expect(await screen.findByText('Firewall corrigido.')).toBeTruthy()
    expect(fix).toHaveBeenCalledTimes(1)
    expect(run.mock.calls.length).toBeGreaterThanOrEqual(2)
  })

  it('sem_bloqueio_sem_botao', async () => {
    render(<DiagScreen />)
    await screen.findByText('Radmin VPN · 26.100.5.20')
    expect(screen.queryByRole('button', { name: 'Corrigir firewall' })).toBeNull()
  })

  it('firewall_erro', async () => {
    run.mockResolvedValue(relFw(true))
    fix.mockRejectedValue(new Error('Permissão negada'))
    render(<DiagScreen />)
    fireEvent.click(await screen.findByRole('button', { name: 'Corrigir firewall' }))
    const al = await screen.findByText('Permissão negada')
    expect(al.getAttribute('role')).toBe('alert')
  })

  it('diagnostico_falha', async () => {
    run.mockRejectedValue(new Error('boom'))
    render(<DiagScreen />)
    const al = await screen.findByRole('alert')
    expect(al.textContent).toContain('boom')
    expect(screen.getByText('Como ler isso')).toBeTruthy()
  })
})
