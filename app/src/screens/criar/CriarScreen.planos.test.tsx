import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '../../bridge/api'
import { travasDoPlano } from '../../bridge/fixtures/travas'
import { mockSettings, mockSnapshot, mockSources } from '../../bridge/mock'
import { PRECOS_MOCK } from '../../bridge/mockConta'
import { resetOnlineEnabled, setOnlineEnabled } from '../../bridge/onlineStore'
import { resetPlano, setPlanoEstado } from '../../bridge/planoStore'
import { resetSnapshotForTests, setSnapshot } from '../../bridge/store'
import type { PlanId, Snapshot } from '../../bridge/types'
import { CriarScreen } from './CriarScreen'

function snapDoPlano(id: PlanId): Snapshot {
  const base = mockSnapshot()
  return {
    ...base,
    account: { requires_account: false, requires_password: false, plan: { id, pode_entrar: id !== 'nenhum' } },
    quality_locks: travasDoPlano(id),
  }
}

function espiar(video: Record<string, unknown> = {}) {
  vi.spyOn(api, 'sourcesList').mockResolvedValue(mockSources())
  const base = mockSettings()
  vi.spyOn(api, 'settingsGet').mockResolvedValue({ ...base, video: { ...base.video, ...video } })
  return {
    roomCreate: vi.spyOn(api, 'roomCreate').mockResolvedValue(undefined),
    shareStart: vi.spyOn(api, 'shareStart').mockResolvedValue(undefined),
    cameraSet: vi.spyOn(api, 'cameraSet').mockResolvedValue(undefined),
    settingsSet: vi.spyOn(api, 'settingsSet').mockResolvedValue(mockSettings()),
  }
}

function abrir(plano: PlanId, online: boolean): void {
  setOnlineEnabled(true)
  act(() => setSnapshot(snapDoPlano(plano)))
  render(<CriarScreen onCreated={vi.fn()} onCancel={vi.fn()} />)
  if (online) fireEvent.click(screen.getByRole('radio', { name: /Online/ }))
}

beforeEach(() => {
  resetSnapshotForTests()
  resetOnlineEnabled()
  resetPlano()
})
afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
  resetSnapshotForTests()
  resetOnlineEnabled()
  resetPlano()
})

describe('CriarScreen com planos', () => {
  it('Local com plano hd não trava nada', async () => {
    espiar()
    abrir('hd', false)
    await waitFor(() => expect(screen.getAllByRole('radio').length).toBeGreaterThan(2))
    expect(document.querySelector('[data-travado]')).toBeNull()
  })

  it('Online hd com preset padrão 1080p60 usa 720p60 e envia o ajustado', async () => {
    const { shareStart } = espiar({ default_preset: '1080p60' })
    abrir('hd', true)
    const grupo = await screen.findByRole('radiogroup', { name: 'Qualidade da tela' })
    await waitFor(() => {
      const r = Array.from(grupo.querySelectorAll('[role="radio"]')).find((x) => /720p60|720ps*60/.test(x.textContent ?? ''))
      expect(r?.getAttribute('aria-checked')).toBe('true')
    })
    await waitFor(() => expect((screen.getByRole('button', { name: 'Iniciar transmissão' }) as HTMLButtonElement).disabled).toBe(false))
    fireEvent.click(screen.getByRole('button', { name: 'Iniciar transmissão' }))
    await waitFor(() => expect(shareStart).toHaveBeenCalled())
    expect(shareStart.mock.calls[0][0].preset).toBe('720p60')
  })

  it('Online hd mostra a câmera no melhor permitido sem gravar outro valor', async () => {
    const { settingsSet } = espiar({ camera_preset: '1080p60' })
    abrir('hd', true)
    await waitFor(() => expect((screen.getByRole('button', { name: 'Iniciar transmissão' }) as HTMLButtonElement).disabled).toBe(false))
    fireEvent.click(screen.getByRole('radio', { name: 'No canto' }))
    const grupo = await screen.findByRole('radiogroup', { name: 'Qualidade da câmera' })
    const marcado = Array.from(grupo.querySelectorAll('[role="radio"]')).find((x) => x.getAttribute('aria-checked') === 'true')
    expect(marcado?.textContent).toMatch(/720p60/)
    fireEvent.click(screen.getByRole('button', { name: 'Iniciar transmissão' }))
    await waitFor(() => expect(settingsSet).toHaveBeenCalled())
    expect(settingsSet.mock.calls[0][0].video?.camera_preset).toBe('1080p60')
  })

  it('espectador Online: Vire Transmissor e só Nada por enquanto; Ver planos só com cobrança', async () => {
    espiar()
    abrir('espectador', true)
    expect(await screen.findByText(/Vire Transmissor/)).toBeTruthy()
    const abas = screen.queryAllByRole('tab')
    expect(abas).toHaveLength(0)
    expect(screen.queryByText('Ver planos')).toBeNull()
    cleanup()
    act(() => setPlanoEstado({ config: { cobrancaAtiva: true, planoPadrao: null, precos: PRECOS_MOCK } }))
    render(<CriarScreen onCreated={vi.fn()} onCancel={vi.fn()} />)
    fireEvent.click(screen.getByRole('radio', { name: /Online/ }))
    expect(await screen.findByText('Ver planos')).toBeTruthy()
  })

  it('Online sem plano: convite no topo e botão de criar desligado', async () => {
    espiar()
    abrir('nenhum', true)
    expect(await screen.findByText('Sua conta não tem um plano para entrar em salas Online')).toBeTruthy()
    const botao = screen.getByRole('button', { name: 'Criar sala' }) as HTMLButtonElement
    expect(botao.disabled).toBe(true)
  })
})
