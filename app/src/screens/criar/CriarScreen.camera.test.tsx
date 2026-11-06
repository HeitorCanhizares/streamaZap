import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { api } from '../../bridge/api'
import { mockSettings, mockSources } from '../../bridge/mock'
import { resetOnlineEnabled, setOnlineEnabled } from '../../bridge/onlineStore'
import { CriarScreen } from './CriarScreen'

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
  resetOnlineEnabled()
})

function espiar() {
  vi.spyOn(api, 'sourcesList').mockResolvedValue(mockSources())
  return {
    roomCreate: vi.spyOn(api, 'roomCreate').mockResolvedValue(undefined),
    shareStart: vi.spyOn(api, 'shareStart').mockResolvedValue(undefined),
    cameraSet: vi.spyOn(api, 'cameraSet').mockResolvedValue(undefined),
    settingsSet: vi.spyOn(api, 'settingsSet').mockResolvedValue(mockSettings()),
  }
}

async function esperarFonte(): Promise<void> {
  await waitFor(() => {
    const botao = screen.getByRole('button', { name: 'Iniciar transmissão' }) as HTMLButtonElement
    expect(botao.disabled).toBe(false)
  })
}

async function iniciarComCamera(canto: string | null): Promise<ReturnType<typeof espiar>> {
  const spies = espiar()
  render(<CriarScreen onCreated={vi.fn()} onCancel={vi.fn()} />)
  await esperarFonte()
  fireEvent.click(screen.getByRole('radio', { name: 'No canto' }) )
  if (canto === null) {
    fireEvent.click(screen.getByRole('radio', { name: 'Solta' }))
  } else {
    fireEvent.click(screen.getByRole('radio', { name: canto }))
  }
  const grupo = screen.getByRole('radiogroup', { name: 'Qualidade da câmera' })
  fireEvent.click(within(grupo).getByRole('radio', { name: /1080ps*60 fps/ }))
  fireEvent.click(screen.getByRole('button', { name: 'Iniciar transmissão' }))
  await waitFor(() => expect(spies.cameraSet).toHaveBeenCalledWith(true))
  return spies
}

describe('CriarScreen (câmera, sem transmitir, Online desligado)', () => {
  it('nada_por_enquanto: só cria a sala, sem som, qualidade, transmissão nem câmera', async () => {
    const { roomCreate, shareStart, cameraSet } = espiar()
    const onCreated = vi.fn()
    render(<CriarScreen onCreated={onCreated} onCancel={vi.fn()} />)
    await esperarFonte()

    fireEvent.click(screen.getByRole('tab', { name: 'Nada por enquanto' }))

    expect(screen.queryByText(/^Som$/)).toBeNull()
    expect(screen.queryByText(/^Qualidade$/)).toBeNull()
    expect(screen.getAllByText('Você entra na sala sem transmitir').length).toBeGreaterThan(0)
    fireEvent.click(screen.getByRole('button', { name: 'Criar sala' }))

    await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1))
    expect(roomCreate).toHaveBeenCalledTimes(1)
    expect(shareStart).not.toHaveBeenCalled()
    expect(cameraSet).not.toHaveBeenCalled()
  })

  it('fluxo_normal: cria a sala e depois transmite', async () => {
    const { roomCreate, shareStart } = espiar()
    const onCreated = vi.fn()
    render(<CriarScreen onCreated={onCreated} onCancel={vi.fn()} />)
    await esperarFonte()
    fireEvent.click(screen.getByRole('button', { name: 'Iniciar transmissão' }))
    await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1))
    expect(roomCreate.mock.invocationCallOrder[0]).toBeLessThan(shareStart.mock.invocationCallOrder[0])
  })

  it('camera_no_canto: grava layout e qualidade antes de ligar a câmera', async () => {
    vi.spyOn(api, 'settingsGet').mockResolvedValue(mockSettings())
    const { settingsSet, cameraSet } = await iniciarComCamera('Superior esquerdo')
    const video = settingsSet.mock.calls[0][0].video
    expect(video?.camera_layout).toEqual({ modo: 'canto', canto: 'sup_esq' })
    expect(video?.camera_preset).toBe('1080p60')
    expect(settingsSet.mock.invocationCallOrder[0]).toBeLessThan(cameraSet.mock.invocationCallOrder[0])
  })

  it('camera_solta: o layout enviado é solta', async () => {
    vi.spyOn(api, 'settingsGet').mockResolvedValue(mockSettings())
    const { settingsSet } = await iniciarComCamera(null)
    expect(settingsSet.mock.calls[0][0].video?.camera_layout).toEqual({ modo: 'solta' })
  })

  it('online_desligado: sem texto online e a sala sai como local', async () => {
    setOnlineEnabled(false)
    const { roomCreate } = espiar()
    render(<CriarScreen onCreated={vi.fn()} onCancel={vi.fn()} />)
    await esperarFonte()
    expect(screen.queryAllByText(/online/i).length).toBe(0)
    expect(screen.queryByRole('radio', { name: /Online/ })).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Iniciar transmissão' }))
    await waitFor(() => expect(roomCreate).toHaveBeenCalledWith(expect.objectContaining({ mode: 'local' })))
  })
})
