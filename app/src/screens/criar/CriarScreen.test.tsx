import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { api } from '../../bridge/api'
import { mockSnapshot, mockSources } from '../../bridge/mock'
import type { Snapshot } from '../../bridge/types'
import { CriarScreen } from './CriarScreen'
import { modoHint } from './criarModel'

// Controla o "ambiente" (app ou navegador) e um snapshot de teste sem mudar o store real.
const estado = vi.hoisted(() => ({ tauri: false, snap: null as Snapshot | null }))

vi.mock('../../bridge/api', async (importOriginal) => {
  const real = await importOriginal<typeof import('../../bridge/api')>()
  return { ...real, isTauri: () => estado.tauri }
})

vi.mock('../../bridge/store', async (importOriginal) => {
  const real = await importOriginal<typeof import('../../bridge/store')>()
  return {
    ...real,
    useSnapshot: () => {
      const atual = real.useSnapshot()
      return estado.snap ?? atual
    },
  }
})

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
  estado.tauri = false
  estado.snap = null
})

// Espera as fontes carregarem: sem fonte escolhida o botão de iniciar fica desabilitado.
// (O nome da fonte aparece no cartão e no resumo, por isso não se usa getByText aqui.)
async function esperarFonte(): Promise<void> {
  await waitFor(() => {
    const botao = screen.getByRole('button', { name: 'Iniciar transmissão' }) as HTMLButtonElement
    expect(botao.disabled).toBe(false)
  })
}

describe('CriarScreen', () => {
  it('renderiza_com_mock: mostra "Bora transmitir?" e "Iniciar transmissão"', () => {
    render(<CriarScreen onCreated={vi.fn()} onCancel={vi.fn()} />)
    expect(screen.getByText('Bora transmitir?')).toBeTruthy()
    expect(screen.getByText('Iniciar transmissão')).toBeTruthy()
  })

  it('iniciar: cria a sala local, começa a transmissão e chama onCreated', async () => {
    const roomCreate = vi.spyOn(api, 'roomCreate').mockResolvedValue(undefined)
    const shareStart = vi.spyOn(api, 'shareStart').mockResolvedValue(undefined)
    const onCreated = vi.fn()
    render(<CriarScreen onCreated={onCreated} onCancel={vi.fn()} />)
    await esperarFonte()

    fireEvent.click(screen.getByRole('button', { name: 'Iniciar transmissão' }))

    await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1))
    expect(roomCreate).toHaveBeenCalledWith(expect.objectContaining({ mode: 'local', password: null }))
    expect(shareStart).toHaveBeenCalledWith(expect.objectContaining({ source: 'screen:0', audio: 'none' }))
    // Ordem: primeiro a sala, depois a transmissão.
    expect(roomCreate.mock.invocationCallOrder[0]).toBeLessThan(shareStart.mock.invocationCallOrder[0])
  })

  it('senha_vazia: com "Com senha" e senha vazia mostra erro e não chama a api', async () => {
    const roomCreate = vi.spyOn(api, 'roomCreate').mockResolvedValue(undefined)
    const shareStart = vi.spyOn(api, 'shareStart').mockResolvedValue(undefined)
    render(<CriarScreen onCreated={vi.fn()} onCancel={vi.fn()} />)
    await esperarFonte()

    fireEvent.click(screen.getByRole('button', { name: /Com senha/ }))
    fireEvent.click(screen.getByRole('button', { name: 'Iniciar transmissão' }))

    expect(screen.getByText('Informe a senha')).toBeTruthy()
    expect(roomCreate).not.toHaveBeenCalled()
    expect(shareStart).not.toHaveBeenCalled()
  })

  it('cancelar: clicar em "Cancelar" chama onCancel', () => {
    const onCancel = vi.fn()
    render(<CriarScreen onCreated={vi.fn()} onCancel={onCancel} />)
    fireEvent.click(screen.getByRole('button', { name: 'Cancelar' }))
    expect(onCancel).toHaveBeenCalledTimes(1)
  })

  it('erro: roomCreate rejeitando mostra a mensagem e libera o botão', async () => {
    vi.spyOn(api, 'roomCreate').mockRejectedValue(new Error('falhou ao criar'))
    const shareStart = vi.spyOn(api, 'shareStart').mockResolvedValue(undefined)
    const onCreated = vi.fn()
    render(<CriarScreen onCreated={onCreated} onCancel={vi.fn()} />)
    await esperarFonte()

    fireEvent.click(screen.getByRole('button', { name: 'Iniciar transmissão' }))

    await waitFor(() => expect(screen.getByText(/falhou ao criar/)).toBeTruthy())
    expect(shareStart).not.toHaveBeenCalled()
    expect(onCreated).not.toHaveBeenCalled()
    const botao = screen.getByRole('button', { name: 'Iniciar transmissão' }) as HTMLButtonElement
    expect(botao.disabled).toBe(false)
  })

  it('modo_online_envia_online: selecionar Online envia mode online', async () => {
    const roomCreate = vi.spyOn(api, 'roomCreate').mockResolvedValue(undefined)
    vi.spyOn(api, 'shareStart').mockResolvedValue(undefined)
    render(<CriarScreen onCreated={vi.fn()} onCancel={vi.fn()} />)
    await esperarFonte()
    fireEvent.click(screen.getByText('Online'))
    fireEvent.click(screen.getByRole('button', { name: 'Iniciar transmissão' }))
    await waitFor(() => expect(roomCreate).toHaveBeenCalledWith(expect.objectContaining({ mode: 'online' })))
  })

  it('secoes_presentes: mostra os títulos das seções', () => {
    render(<CriarScreen onCreated={vi.fn()} onCancel={vi.fn()} />)
    for (const t of ['Bora transmitir?', 'Onde a sala vai rodar', 'O que compartilhar', 'Som', 'Qualidade', 'É isso que a galera vai ver']) {
      expect(screen.getAllByText(t).length).toBeGreaterThan(0)
    }
  })

  it('hint_muda_com_modo: a legenda muda ao trocar o modo', () => {
    render(<CriarScreen onCreated={vi.fn()} onCancel={vi.fn()} />)
    expect(screen.getByText(modoHint('local'))).toBeTruthy()
    fireEvent.click(screen.getByText('Online'))
    expect(screen.getByText(modoHint('online'))).toBeTruthy()
  })

  it('online_no_navegador: mostra o código do design e o status do servidor no resumo', () => {
    estado.snap = { ...mockSnapshot(), server: { state: 'online', ping_ms: 42 } }
    render(<CriarScreen onCreated={vi.fn()} onCancel={vi.fn()} />)
    fireEvent.click(screen.getByText('Online'))
    expect(screen.getByText('ZAP-7K2Q')).toBeTruthy()
    expect(screen.getByText(/Servidor online · 42 ms\./)).toBeTruthy()
  })

  it('online_sem_servidor_no_app: bloqueia iniciar e não chama roomCreate', async () => {
    estado.tauri = true
    estado.snap = { ...mockSnapshot(), server: { state: 'error' } }
    vi.spyOn(api, 'sourcesList').mockResolvedValue(mockSources())
    const roomCreate = vi.spyOn(api, 'roomCreate').mockResolvedValue(undefined)
    const shareStart = vi.spyOn(api, 'shareStart').mockResolvedValue(undefined)
    render(<CriarScreen onCreated={vi.fn()} onCancel={vi.fn()} />)
    await esperarFonte()

    fireEvent.click(screen.getByText('Online'))
    fireEvent.click(screen.getByRole('button', { name: 'Iniciar transmissão' }))

    expect(screen.getByText(/Sem conexão com o servidor/)).toBeTruthy()
    expect(roomCreate).not.toHaveBeenCalled()
    expect(shareStart).not.toHaveBeenCalled()
  })

  it('online_com_servidor_no_app: com servidor online chama roomCreate com mode online', async () => {
    estado.tauri = true
    estado.snap = { ...mockSnapshot(), server: { state: 'online', ping_ms: 42 } }
    vi.spyOn(api, 'sourcesList').mockResolvedValue(mockSources())
    const roomCreate = vi.spyOn(api, 'roomCreate').mockResolvedValue(undefined)
    vi.spyOn(api, 'shareStart').mockResolvedValue(undefined)
    render(<CriarScreen onCreated={vi.fn()} onCancel={vi.fn()} />)
    await esperarFonte()

    fireEvent.click(screen.getByText('Online'))
    fireEvent.click(screen.getByRole('button', { name: 'Iniciar transmissão' }))

    await waitFor(() => expect(roomCreate).toHaveBeenCalledWith(expect.objectContaining({ mode: 'online' })))
  })
})
