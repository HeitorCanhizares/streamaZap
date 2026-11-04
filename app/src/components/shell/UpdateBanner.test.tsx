// Tarefa ui5-06
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { UpdateBanner } from './UpdateBanner'
import { mockSnapshot } from '../../bridge/mock'
import { api } from '../../bridge/api'
import { setSnapshot } from '../../bridge/store'
import { resetUpdateState, setUpdateState } from '../../bridge/updateStore'

vi.mock('../../bridge/api', () => ({
  api: {
    updateInstallNow: vi.fn().mockResolvedValue(undefined),
  },
  isTauri: () => false,
}))

// O store só aceita versões crescentes: cada snapshot de teste usa uma versão nova.
let versao = 5000
function snapshot(extra: { room?: null; newer_protocol?: number } = {}): void {
  const s = mockSnapshot()
  setSnapshot({ ...s, room: extra.room === null ? null : s.room, version: ++versao, ...(extra.newer_protocol !== undefined ? { newer_protocol: extra.newer_protocol } : {}) })
}

beforeEach(() => {
  vi.clearAllMocks()
  resetUpdateState()
  snapshot({ room: null })
})

afterEach(() => {
  cleanup()
  resetUpdateState()
})

describe('UpdateBanner', () => {
  it('sem atualização e sem newer_protocol não renderiza nada', () => {
    const { container } = render(<UpdateBanner onOpenConfig={() => undefined} />)
    expect(container.innerHTML).toBe('')
  })

  it('fase pronta mostra o aviso e o botão Reiniciar agora habilitado', () => {
    setUpdateState({ fase: 'pronta', progresso: 100, versao: '3.0.1', erro: null })
    render(<UpdateBanner onOpenConfig={() => undefined} />)
    expect(screen.getByText('Atualização 3.0.1 pronta — instala ao fechar')).toBeTruthy()
    const botao = screen.getByRole('button', { name: 'Reiniciar agora' }) as HTMLButtonElement
    expect(botao.disabled).toBe(false)
  })

  it('clicar em Reiniciar agora chama api.updateInstallNow', () => {
    setUpdateState({ fase: 'pronta', progresso: 100, versao: '3.0.1', erro: null })
    render(<UpdateBanner onOpenConfig={() => undefined} />)
    fireEvent.click(screen.getByRole('button', { name: 'Reiniciar agora' }))
    expect(api.updateInstallNow).toHaveBeenCalledTimes(1)
  })

  it('com sala aberta o botão Reiniciar agora fica desabilitado', () => {
    snapshot({})
    setUpdateState({ fase: 'pronta', progresso: 100, versao: '3.0.1', erro: null })
    render(<UpdateBanner onOpenConfig={() => undefined} />)
    const botao = screen.getByRole('button', { name: 'Reiniciar agora' }) as HTMLButtonElement
    expect(botao.disabled).toBe(true)
    expect(botao.title).toBe('Saia da sala para reiniciar')
  })

  it('falha ao reiniciar mostra o erro em role=alert', async () => {
    vi.mocked(api.updateInstallNow).mockRejectedValueOnce(new Error('sem permissão'))
    setUpdateState({ fase: 'pronta', progresso: 100, versao: '3.0.1', erro: null })
    render(<UpdateBanner onOpenConfig={() => undefined} />)
    fireEvent.click(screen.getByRole('button', { name: 'Reiniciar agora' }))
    expect(await screen.findByRole('alert')).toBeTruthy()
    await waitFor(() => expect(screen.getByText('sem permissão')).toBeTruthy())
  })

  it('fase baixando mostra o progresso e nenhum botão', () => {
    setUpdateState({ fase: 'baixando', progresso: 55, versao: '3.0.1', erro: null })
    render(<UpdateBanner onOpenConfig={() => undefined} />)
    expect(screen.getByText(/55%/)).toBeTruthy()
    expect(screen.queryAllByRole('button').length).toBe(0)
  })

  it('newer_protocol com fase nada mostra o convite e Atualizar chama onOpenConfig', () => {
    snapshot({ newer_protocol: 4 })
    const onOpenConfig = vi.fn()
    render(<UpdateBanner onOpenConfig={onOpenConfig} />)
    expect(screen.getByText(/versão mais nova/)).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Atualizar' }))
    expect(onOpenConfig).toHaveBeenCalledTimes(1)
  })
})
