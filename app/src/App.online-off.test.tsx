// Tarefa front-27: aceite A do plano contas-planos-camera §4, na interface inteira (<App />).
// Com server.enabled = false não pode aparecer "Online", aba, código de sala nem status de servidor.
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'
import { App } from './App'
import { api } from './bridge/api'
import { resetOnlineEnabled, setOnlineEnabled } from './bridge/onlineStore'
import { resetSnapshotForTests } from './bridge/store'

// jsdom não tem WebCodecs: a tela Sala cria um VideoDecoder ao montar, então damos um falso.
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

// O App lê settingsGet ao montar; o mock devolve enabled = true, então forçamos o valor do teste.
function mockServerEnabled(enabled: boolean): void {
  const original = api.settingsGet.bind(api)
  vi.spyOn(api, 'settingsGet').mockImplementation(async () => {
    const s = await original()
    return { ...s, server: { address: '', password: '', use_relay: true, ...s.server, enabled } }
  })
}

beforeEach(() => {
  setOnlineEnabled(false)
})
afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
  resetOnlineEnabled()
  resetSnapshotForTests()
})

// Texto da tela sem o cartão discreto (única exceção permitida).
function textoSemCartao(): string {
  const clone = document.body.cloneNode(true) as HTMLElement
  clone.querySelectorAll('[data-testid="cartao-online"]').forEach((n) => n.remove())
  return clone.textContent ?? ''
}
const barra = (): string => document.querySelector('header.zp-titlebar')?.textContent ?? ''

function semOnline(): void {
  const t = textoSemCartao()
  expect(t).not.toMatch(/online/i)
  expect(t).not.toMatch(/ZAP-[A-Z0-9]{4}/)
}

describe('App com Online desligado (aceite A)', () => {
  beforeEach(() => mockServerEnabled(false))

  it('(1) tela Salas sem Online, só o cartão discreto', async () => {
    render(<App />)
    await screen.findByTestId('cartao-online')
    await waitFor(() => expect(api.settingsGet).toHaveBeenCalled())
    semOnline()
  })

  it('(2) tela Criar sala sem Online', async () => {
    render(<App />)
    await screen.findByTestId('cartao-online')
    fireEvent.click(screen.getByRole('button', { name: /^(Nova sala|Criar sala)$/ }))
    await waitFor(() => expect(document.querySelector('header')?.textContent).toContain('Nova sala'))
    expect(screen.queryByTestId('cartao-online')).toBeNull()
    semOnline()
  })

  it('(3) barra de título sem "servidor"', async () => {
    render(<App />)
    await screen.findByTestId('cartao-online')
    expect(barra()).not.toMatch(/servidor/i)
  })

  it('(4) tela Diagnóstico sem Online', async () => {
    render(<App />)
    await screen.findByTestId('cartao-online')
    fireEvent.click(screen.getByRole('button', { name: /diagn/i }))
    await waitFor(() => expect(barra()).toContain('Diagnóstico'))
    semOnline()
  })

  it('sem aba, sem Entrar com código', async () => {
    render(<App />)
    await screen.findByTestId('cartao-online')
    expect(screen.queryByRole('tab')).toBeNull()
    expect(screen.queryByText('Entrar com código')).toBeNull()
  })
})

describe('App com Online ligado', () => {
  beforeEach(() => mockServerEnabled(true))

  it('(5) Salas mostra as abas Online e Local e a barra mostra "Servidor online"', async () => {
    render(<App />)
    expect(await screen.findByRole('tab', { name: /Online/ })).toBeTruthy()
    expect(screen.getByRole('tab', { name: /Local/ })).toBeTruthy()
    await waitFor(() => expect(barra()).toContain('Servidor online'))
  })

  it('(6) o cartão discreto não aparece', async () => {
    render(<App />)
    await screen.findByRole('tab', { name: /Online/ })
    expect(screen.queryByTestId('cartao-online')).toBeNull()
  })
})
