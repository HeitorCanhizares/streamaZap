// Painel "Transmissão" com travas de plano (Online) e "Vire Transmissor".
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '../../bridge/api'
import { travasDoPlano } from '../../bridge/fixtures/travas'
import { mockSettings, mockSnapshot, mockSources } from '../../bridge/mock'
import { PRECOS_MOCK } from '../../bridge/mockConta'
import { resetOnlineEnabled, setOnlineEnabled } from '../../bridge/onlineStore'
import { resetPlano, setPlanoEstado } from '../../bridge/planoStore'
import { resetSnapshotForTests, setSnapshot } from '../../bridge/store'
import type { PlanId, RoomMode, Snapshot } from '../../bridge/types'
import { SalaTransmissao } from './SalaTransmissao'

function snapDoPlano(id: PlanId, modo: RoomMode = 'online'): Snapshot {
  const base = mockSnapshot()
  return {
    ...base,
    room: base.room ? { ...base.room, mode: modo } : base.room,
    account: { requires_account: false, requires_password: false, plan: { id, pode_entrar: id !== 'nenhum' } },
    quality_locks: travasDoPlano(id),
  }
}

function espiar(video: Record<string, unknown> = {}) {
  vi.spyOn(api, 'sourcesList').mockResolvedValue(mockSources())
  const base = mockSettings()
  vi.spyOn(api, 'settingsGet').mockResolvedValue({ ...base, video: { ...base.video, ...video } })
  return {
    start: vi.spyOn(api, 'shareStart').mockResolvedValue(undefined),
    update: vi.spyOn(api, 'shareUpdate').mockResolvedValue(undefined),
  }
}

function abrir(plano: PlanId, modo: RoomMode = 'online'): void {
  setOnlineEnabled(true)
  act(() => setSnapshot(snapDoPlano(plano, modo)))
  render(<SalaTransmissao share={null} camOn={false} onToggleCamera={vi.fn()} onCameraTrocada={vi.fn()} onClose={vi.fn()} />)
}

beforeEach(() => { resetSnapshotForTests(); resetOnlineEnabled(); resetPlano() })
afterEach(() => { cleanup(); vi.restoreAllMocks(); resetSnapshotForTests(); resetOnlineEnabled(); resetPlano() })

describe('SalaTransmissao com planos', () => {
  it('plano hd: 1080p60 da tela fica travado com selo Full HD', async () => {
    espiar()
    abrir('hd')
    const grupo = await screen.findByRole('radiogroup', { name: 'Qualidade' })
    const travado = Array.from(grupo.querySelectorAll('[role="radio"]')).find((r) => /1080/.test(r.textContent ?? '') && /60/.test(r.textContent ?? ''))
    expect(travado?.getAttribute('data-travado')).toBe('true')
    expect(travado?.textContent).toMatch(/Full HD/)
  })

  it('default_preset 1080p60 cai para 720p60 e o pedido usa 720p60', async () => {
    const { start } = espiar({ default_preset: '1080p60' })
    abrir('hd')
    const grupo = await screen.findByRole('radiogroup', { name: 'Qualidade' })
    await waitFor(() => {
      const r = Array.from(grupo.querySelectorAll('[role="radio"]')).find((x) => /720/.test(x.textContent ?? '') && /60/.test(x.textContent ?? ''))
      expect(r?.getAttribute('aria-checked')).toBe('true')
    })
    fireEvent.click(screen.getByRole('button', { name: 'Começar a transmitir' }))
    await waitFor(() => expect(start).toHaveBeenCalled())
    expect(start.mock.calls[0][0]).toMatchObject({ preset: '720p60' })
  })

  it('espectador: Vire Transmissor, sem seletor de qualidade; Ver planos só com cobrança', async () => {
    espiar()
    abrir('espectador')
    expect(screen.getByText('Vire Transmissor')).toBeTruthy()
    expect(screen.queryByRole('radiogroup', { name: 'Qualidade' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Ver planos' })).toBeNull()
    cleanup()
    act(() => setPlanoEstado({ config: { cobrancaAtiva: true, planoPadrao: null, precos: PRECOS_MOCK } }))
    abrir('espectador')
    expect(screen.getByRole('button', { name: 'Ver planos' })).toBeTruthy()
  })

  it('sala Local: sem travas', async () => {
    espiar()
    abrir('hd', 'local')
    await screen.findByRole('radiogroup', { name: 'Qualidade' })
    expect(document.querySelector('[data-travado]')).toBeNull()
  })
})
