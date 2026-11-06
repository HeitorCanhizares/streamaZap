// Tarefa front-19: login por conta na interface inteira (<App />), com o cliente falso (mockConta).
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'
import { App } from './App'
import { api } from './bridge/api'
import { resetConta } from './bridge/contaStore'
import { resetOnlineEnabled, setOnlineEnabled } from './bridge/onlineStore'
import { getSnapshot, resetSnapshotForTests, setSnapshot } from './bridge/store'

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

function mockServerEnabled(enabled: boolean): void {
  const original = api.settingsGet.bind(api)
  vi.spyOn(api, 'settingsGet').mockImplementation(async () => {
    const s = await original()
    return { ...s, server: { address: '', password: '', use_relay: true, ...s.server, enabled } }
  })
}

beforeEach(() => {
  resetConta()
  resetSnapshotForTests()
  setOnlineEnabled(true)
})
afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
  resetConta()
  resetOnlineEnabled()
  resetSnapshotForTests()
})

async function abrirConfig(): Promise<void> {
  fireEvent.click(screen.getByRole('button', { name: 'Configurações' }))
  await screen.findByText('Tudo salva sozinho')
}

describe('App · login por conta no mock', () => {
  it('(a) Entrar com Google leva a "Conectado como" e Sair volta ao login', async () => {
    render(<App />)
    await abrirConfig()
    fireEvent.click(await screen.findByRole('button', { name: 'Entrar com Google' }))
    await waitFor(() => expect(document.body.textContent).toContain('Conectado como voce@exemplo.com'))
    fireEvent.click(await screen.findByRole('button', { name: 'Sair' }))
    expect(await screen.findByRole('button', { name: 'Entrar com Google' })).toBeTruthy()
  })

  it('(b) fluxo por e-mail com código certo e código errado', async () => {
    render(<App />)
    await abrirConfig()
    fireEvent.change(await screen.findByLabelText('E-mail'), { target: { value: 'voce@exemplo.com' } })
    fireEvent.click(screen.getByRole('button', { name: 'Enviar código' }))
    const campo = await screen.findByLabelText('Código de 6 dígitos')
    fireEvent.change(campo, { target: { value: '000000' } })
    fireEvent.click(screen.getByRole('button', { name: 'Confirmar' }))
    expect((await screen.findByRole('alert')).textContent).toContain('O código expirou ou está errado.')
    fireEvent.change(screen.getByLabelText('Código de 6 dígitos'), { target: { value: '123456' } })
    fireEvent.click(screen.getByRole('button', { name: 'Confirmar' }))
    await waitFor(() => expect(document.body.textContent).toContain('Conectado como voce@exemplo.com'))
  })

  it('(c) repassa o token ao núcleo ao entrar e null ao sair', async () => {
    const espiao = vi.spyOn(api, 'onlineSetAccount')
    render(<App />)
    await abrirConfig()
    fireEvent.click(await screen.findByRole('button', { name: 'Entrar com Google' }))
    await waitFor(() => expect(espiao).toHaveBeenCalledWith('mock-token'))
    espiao.mockClear()
    fireEvent.click(await screen.findByRole('button', { name: 'Sair' }))
    await waitFor(() => expect(espiao).toHaveBeenCalledWith(null))
  })

  it('(d) com o Online desligado não há seção Conta nem chamada ao núcleo', async () => {
    setOnlineEnabled(false)
    mockServerEnabled(false)
    const espiao = vi.spyOn(api, 'onlineSetAccount')
    render(<App />)
    await abrirConfig()
    expect(screen.queryByTestId('config-conta')).toBeNull()
    expect(screen.queryByRole('button', { name: 'Entrar com Google' })).toBeNull()
    expect(espiao).not.toHaveBeenCalled()
  })

  it('(e) snapshot sem account não mostra a seção Conta', async () => {
    const { account: _removido, ...resto } = getSnapshot()
    void _removido
    setSnapshot(resto as ReturnType<typeof getSnapshot>)
    render(<App />)
    await abrirConfig()
    expect(screen.queryByTestId('config-conta')).toBeNull()
    expect(screen.queryByRole('button', { name: 'Entrar com Google' })).toBeNull()
  })
})
