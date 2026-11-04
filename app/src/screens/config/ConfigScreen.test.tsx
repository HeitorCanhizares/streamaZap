// Tarefa ui-26
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { api } from '../../bridge/api'
import { mockSettings, mockSnapshot } from '../../bridge/mock'
import { useSnapshot } from '../../bridge/store'
import { ConfigScreen } from './ConfigScreen'

vi.mock('../../bridge/store', () => ({ useSnapshot: vi.fn() }))

beforeEach(async () => {
  vi.mocked(useSnapshot).mockReturnValue(mockSnapshot())
  // O mock do api guarda estado entre testes: volta às configurações padrão.
  await api.settingsSet(mockSettings())
})

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
  vi.useRealTimers()
})

describe('ConfigScreen', () => {
  it('carrega: com o mock renderiza Configurações, Perfil, Conexão e o nome do mockSettings', async () => {
    render(<ConfigScreen />)
    expect(await screen.findByText('Configurações')).toBeTruthy()
    expect(screen.getByText('Perfil')).toBeTruthy()
    expect(screen.getByText('Conexão')).toBeTruthy()
    expect(screen.getByDisplayValue(mockSettings().name)).toBeTruthy()
  })

  it('autosave: mudar o nome chama settingsSet com {name} e mostra Salvo', async () => {
    const espiada = vi.spyOn(api, 'settingsSet')
    render(<ConfigScreen />)
    const campo = (await screen.findByDisplayValue(mockSettings().name)) as HTMLInputElement
    fireEvent.change(campo, { target: { value: 'novo nome' } })
    expect(espiada).toHaveBeenCalledWith({ name: 'novo nome' })
    expect(await screen.findByText('Salvo')).toBeTruthy()
  })

  it('erro: settingsSet rejeitando mostra a mensagem de erro', async () => {
    vi.spyOn(api, 'settingsSet').mockRejectedValue(new Error('disco cheio'))
    render(<ConfigScreen />)
    const campo = (await screen.findByDisplayValue(mockSettings().name)) as HTMLInputElement
    fireEvent.change(campo, { target: { value: 'x' } })
    expect(await screen.findByText(/disco cheio/)).toBeTruthy()
  })

  it('Salvo some depois de 1,5 s', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    vi.spyOn(api, 'settingsSet')
    render(<ConfigScreen />)
    const campo = (await screen.findByDisplayValue(mockSettings().name)) as HTMLInputElement
    fireEvent.change(campo, { target: { value: 'outro' } })
    await waitFor(() => expect(screen.getByText('Salvo')).toBeTruthy())
    await act(async () => {
      vi.advanceTimersByTime(1500)
    })
    expect(screen.queryByText('Salvo')).toBeNull()
    expect(screen.getByText('Tudo salva sozinho')).toBeTruthy()
  })
})
