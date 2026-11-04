// Tarefa ui-18
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { api } from '../../bridge/api'
import { CriarScreen } from './CriarScreen'

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
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

    fireEvent.click(screen.getByRole('button', { name: 'Com senha' }))
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
})
