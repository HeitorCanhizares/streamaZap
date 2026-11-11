// Tarefa telas-15: com a cobrança desligada nenhuma tela mostra preço, "Assinar" nem "Ver planos".
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import type { ReactElement } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('../../video/player', () => ({ VideoPlayer: class { push() {} close() {} } }))
vi.mock('../salas/AtualizadoCard', () => ({ AtualizadoCard: () => <div /> }))

import { api } from '../../bridge/api'
import { setContaEstado, resetConta } from '../../bridge/contaStore'
import { travasDoPlano } from '../../bridge/fixtures/travas'
import { mockSettings, mockSnapshot, mockSources } from '../../bridge/mock'
import { resetOnlineEnabled, setOnlineEnabled } from '../../bridge/onlineStore'
import { abrirPlanos } from '../../bridge/planoAcoes'
import { resetPlano } from '../../bridge/planoStore'
import { resetSnapshotForTests, setSnapshot } from '../../bridge/store'
import type { PlanId, PlanNotice, Snapshot } from '../../bridge/types'
import { ConfigConexao } from '../config/ConfigConexao'
import { CriarScreen } from '../criar/CriarScreen'
import { SalaScreen } from '../sala/SalaScreen'
import { SalaTransmissao } from '../sala/SalaTransmissao'
import { SalasScreen } from '../salas/SalasScreen'
import { AvisoPlano } from './AvisoPlano'
import { PlanosModal } from './PlanosModal'

const PLANOS: PlanId[] = ['nenhum', 'espectador', 'hd', 'fullhd']

const aviso: PlanNotice = { seq: 1, plan: 'espectador', action: 'transmitir', message: 'Seu plano não permite isso.' }

function snapDoPlano(id: PlanId, extra: Partial<Snapshot> = {}): Snapshot {
  const base = mockSnapshot()
  return {
    ...base,
    room: base.room ? { ...base.room, mode: 'online' } : base.room,
    account: { ...base.account!, plan: { id, pode_entrar: id !== 'nenhum' } },
    quality_locks: travasDoPlano(id),
    ...extra,
  }
}

function montar(plano: PlanId, ui: ReactElement, extra: Partial<Snapshot> = {}): HTMLElement {
  setOnlineEnabled(true)
  act(() => setSnapshot(snapDoPlano(plano, extra)))
  return render(ui).container
}

function semPreco(container: HTMLElement): void {
  expect(container.textContent).not.toMatch(/R\$/)
  expect(screen.queryByText('Assinar')).toBeNull()
  expect(screen.queryByRole('button', { name: 'Ver planos' })).toBeNull()
  expect(screen.queryByText('Assinar / trocar plano')).toBeNull()
}

beforeEach(() => {
  vi.spyOn(api, 'sourcesList').mockResolvedValue(mockSources())
  vi.spyOn(api, 'settingsGet').mockResolvedValue(mockSettings())
})
afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
  resetSnapshotForTests()
  resetOnlineEnabled()
  resetPlano()
  resetConta()
})

describe.each(PLANOS)('sem cobrança, plano %s', (plano) => {
  it('SalasScreen (Online)', () => {
    semPreco(montar(plano, <SalasScreen onCreate={() => {}} onEntered={() => {}} />))
  })

  it('CriarScreen (Online)', async () => {
    const c = montar(plano, <CriarScreen onCreated={() => {}} onCancel={() => {}} />)
    fireEvent.click(screen.getByRole('radio', { name: /Online/ }))
    await act(async () => {})
    semPreco(c)
  })

  it('SalaScreen (sala Online)', async () => {
    const c = montar(plano, <SalaScreen onLeft={() => {}} />, { plan_notice: aviso })
    await act(async () => {})
    semPreco(c)
  })

  it('SalaTransmissao (sala Online)', async () => {
    const c = montar(
      plano,
      <SalaTransmissao share={null} camOn={false} onToggleCamera={vi.fn()} onCameraTrocada={vi.fn()} onClose={vi.fn()} />,
    )
    await act(async () => {})
    semPreco(c)
  })

  it('Configurações: Conexão e Conta', () => {
    act(() => setContaEstado({ sessao: 'dentro', email: 'voce@exemplo.com', identidades: [] }))
    const settings = { ...mockSettings(), server: { enabled: true, address: '', password: '', use_relay: true } }
    const c = montar(plano, <ConfigConexao settings={settings} onChange={vi.fn()} network={{ radmin_ip: null, interfaces: [] }} />)
    expect(screen.getByTestId('config-conta')).toBeTruthy()
    semPreco(c)
  })

  it('AvisoPlano com plan_notice', () => {
    const c = montar(plano, <AvisoPlano />, { plan_notice: aviso })
    expect(screen.getByRole('alert').textContent).toContain('Seu plano não permite isso.')
    semPreco(c)
  })

  it('abrirPlanos não abre o modal', () => {
    montar(plano, <PlanosModal />)
    act(() => abrirPlanos())
    act(() => abrirPlanos('hd'))
    expect(screen.queryByRole('dialog', { name: 'Planos do Online' })).toBeNull()
  })
})
