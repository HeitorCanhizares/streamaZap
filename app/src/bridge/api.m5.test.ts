import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

// Dublê do plugin Tauri: invoke falso e Channel minimo com onmessage.
vi.mock('@tauri-apps/api/core', () => {
  class FakeChannel {
    onmessage: ((m: unknown) => void) | null = null
  }
  return { invoke: vi.fn(), Channel: FakeChannel }
})

import { invoke } from '@tauri-apps/api/core'
import { api } from './api'

const invokeMock = vi.mocked(invoke)

// Ambiente de navegador: sem internals do Tauri.
const fora = () => {
  delete (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
}
const dentro = () => {
  ;(globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {}
}

beforeEach(() => {
  invokeMock.mockReset()
  invokeMock.mockResolvedValue(undefined as never)
})

afterEach(() => {
  fora()
})

describe('M5 fora do Tauri (mock)', () => {
  it('updateCheck devolve disponivel false sem chamar invoke', async () => {
    fora()
    const info = await api.updateCheck()
    expect(info).toEqual({ disponivel: false, versao: null, notas: null, tamanho: null })
    expect(invokeMock).not.toHaveBeenCalled()
  })

  it('updateDownload, updateInstallNow e updateInstallOnExit nao chamam invoke', async () => {
    fora()
    await api.updateDownload()
    await api.updateInstallNow()
    await api.updateInstallOnExit()
    expect(invokeMock).not.toHaveBeenCalled()
  })

  it('diagnosticsRun devolve o relatório de exemplo', async () => {
    fora()
    const r = await api.diagnosticsRun()
    expect(r.app_version).toBe('3.0.0')
    expect(r.friends.length).toBe(4)
    expect(invokeMock).not.toHaveBeenCalled()
  })

  it('diagnosticsText devolve texto com Radmin', async () => {
    fora()
    const texto = await api.diagnosticsText()
    expect(texto).toContain('Radmin')
    expect(texto.startsWith('streamaZap · relatório de diagnóstico')).toBe(true)
    expect(invokeMock).not.toHaveBeenCalled()
  })

  it('firewallFix devolve ok true sem chamar invoke', async () => {
    fora()
    const r = await api.firewallFix()
    expect(r.ok).toBe(true)
    expect(invokeMock).not.toHaveBeenCalled()
  })
})

describe('M5 dentro do Tauri', () => {
  it('updateCheck chama invoke("update_check")', async () => {
    dentro()
    await api.updateCheck()
    expect(invokeMock).toHaveBeenCalledWith('update_check')
  })

  it('updateDownload chama invoke("update_download")', async () => {
    dentro()
    await api.updateDownload()
    expect(invokeMock).toHaveBeenCalledWith('update_download')
  })

  it('updateInstallNow chama invoke("update_install_now")', async () => {
    dentro()
    await api.updateInstallNow()
    expect(invokeMock).toHaveBeenCalledWith('update_install_now')
  })

  it('updateInstallOnExit chama invoke("update_install_on_exit")', async () => {
    dentro()
    await api.updateInstallOnExit()
    expect(invokeMock).toHaveBeenCalledWith('update_install_on_exit')
  })

  it('diagnosticsRun chama invoke("diagnostics_run")', async () => {
    dentro()
    await api.diagnosticsRun()
    expect(invokeMock).toHaveBeenCalledWith('diagnostics_run')
  })

  it('diagnosticsText chama invoke("diagnostics_text")', async () => {
    dentro()
    await api.diagnosticsText()
    expect(invokeMock).toHaveBeenCalledWith('diagnostics_text')
  })

  it('firewallFix chama invoke("firewall_fix")', async () => {
    dentro()
    await api.firewallFix()
    expect(invokeMock).toHaveBeenCalledWith('firewall_fix')
  })

  it('erro string do invoke em um comando M5 vira Error com a mesma mensagem', async () => {
    dentro()
    invokeMock.mockRejectedValueOnce('sem permissão')
    const erro = await api.updateCheck().catch((e: unknown) => e)
    expect(erro).toBeInstanceOf(Error)
    expect((erro as Error).message).toBe('sem permissão')
  })
})
