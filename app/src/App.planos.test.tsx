// Tarefa telas-14: AvisoPlano e PlanosModal globais no <App />.
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('./bridge/contaClient', async (importOriginal) => {
  const orig = await importOriginal<typeof import('./bridge/contaClient')>()
  return { ...orig, obterCliente: vi.fn(orig.obterCliente) }
})

import { App } from './App'
import { api } from './bridge/api'
import { obterCliente } from './bridge/contaClient'
import { mockSnapshot, mockSnapshotComPlano } from './bridge/mock'
import { PRECOS_MOCK } from './bridge/mockConta'
import { resetOnlineEnabled, setOnlineEnabled } from './bridge/onlineStore'
import { abrirPlanos } from './bridge/planoAcoes'
import { resetPlano, setPlanoEstado } from './bridge/planoStore'
import { resetSnapshotForTests, setSnapshot } from './bridge/store'

// jsdom não tem WebCodecs: a tela Sala cria um VideoDecoder ao montar.
const semVideoDecoder = !('VideoDecoder' in globalThis)
beforeAll(() => {
  if (semVideoDecoder) {
    ;(globalThis as Record<string, unknown>).VideoDecoder = class {
      configure(): void {}
      decode(): void {}
      close(): void {}
    }
  }
})
afterAll(() => {
  if (semVideoDecoder) delete (globalThis as Record<string, unknown>).VideoDecoder
})

function mockServerEnabled(enabled: boolean): void {
  const original = api.settingsGet.bind(api)
  vi.spyOn(api, 'settingsGet').mockImplementation(async () => {
    const s = await original()
    return { ...s, server: { address: '', password: '', use_relay: true, ...s.server, enabled } }
  })
}

const COBRANCA = { cobrancaAtiva: true, planoPadrao: null, precos: PRECOS_MOCK }

beforeEach(() => {
  setOnlineEnabled(true)
  resetPlano()
  vi.mocked(obterCliente).mockClear()
})
afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
  resetOnlineEnabled()
  resetPlano()
  resetSnapshotForTests()
})

describe('App com avisos e planos globais', () => {
  it('mostra o plan_notice do snapshot; sem cobrança não há Ver planos', async () => {
    mockServerEnabled(true)
    act(() => {
      setSnapshot({
        ...mockSnapshot(),
        version: 100,
        plan_notice: { seq: 1, plan: 'hd', action: 'transmitir', field: 'camadas', message: 'Seu plano não permite essa qualidade' },
      })
    })
    render(<App />)
    expect(await screen.findByText('Seu plano não permite essa qualidade')).toBeTruthy()
    expect(screen.queryByText('Ver planos')).toBeNull()
  })

  it('abrirPlanos abre o diálogo Planos do Online', async () => {
    mockServerEnabled(true)
    render(<App />)
    act(() => {
      setPlanoEstado({ config: COBRANCA })
      abrirPlanos('hd')
    })
    expect(await screen.findByRole('dialog', { name: 'Planos do Online' })).toBeTruthy()
  })

  it('Online desligado: sem cadeado, sem aviso/convite e sem criar o mock da conta', async () => {
    setOnlineEnabled(false)
    mockServerEnabled(false)
    // Conta sem plano e com travas: com o Online ligado haveria convite e cadeado.
    act(() => {
      setSnapshot(mockSnapshotComPlano('nenhum'))
    })
    const semConvite = (): void => {
      expect(screen.queryByText(/Sua conta não tem um plano para entrar em salas Online/)).toBeNull()
      expect(screen.queryByText(/Assine o Espectador/)).toBeNull()
      expect(screen.queryByText('Ver planos')).toBeNull()
      expect(screen.queryByRole('alert')).toBeNull()
      expect(screen.queryByRole('dialog', { name: 'Planos do Online' })).toBeNull()
    }
    render(<App />)
    await screen.findByTestId('cartao-online')
    await waitFor(() => expect(api.settingsGet).toHaveBeenCalled())
    semConvite()
    fireEvent.click(screen.getByRole('button', { name: /^(Nova sala|Criar sala)$/ }))
    await waitFor(() => expect(document.querySelector('header')?.textContent).toContain('Nova sala'))
    semConvite()
    expect(document.querySelector('[data-travado]')).toBeNull()
    expect(screen.queryAllByTestId('selo-plano')).toHaveLength(0)
    expect(obterCliente).not.toHaveBeenCalled()
  })
})
