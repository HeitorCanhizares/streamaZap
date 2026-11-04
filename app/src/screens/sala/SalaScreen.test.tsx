// Tarefa ui-22
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import type { Snapshot } from '../../bridge/types'

const estado = vi.hoisted(() => ({ snap: null as unknown as Snapshot }))

vi.mock('../../bridge/store', () => ({
  useSnapshot: () => estado.snap,
}))

vi.mock('../../video/player', () => ({
  VideoPlayer: class { push() {} close() {} },
}))

import { api } from '../../bridge/api'
import { mockSnapshot } from '../../bridge/mock'
import { SalaScreen } from './SalaScreen'

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

describe('SalaScreen', () => {
  it('sem_sala: snapshot sem room mostra aviso e Voltar', () => {
    estado.snap = { ...mockSnapshot(), room: null }
    render(<SalaScreen onLeft={() => {}} />)
    expect(screen.getByText('Você não está em uma sala')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Voltar' })).toBeTruthy()
  })

  it('com_sala: mostra nome da sala, Chat e Pessoas', () => {
    estado.snap = mockSnapshot()
    render(<SalaScreen onLeft={() => {}} />)
    expect(screen.getByText('Sexta do Rush')).toBeTruthy()
    expect(screen.getByText('Chat')).toBeTruthy()
    expect(screen.getByText('Pessoas')).toBeTruthy()
  })

  it('sair: clicar Sair chama api.roomLeave e depois onLeft', async () => {
    estado.snap = mockSnapshot()
    const leave = vi.spyOn(api, 'roomLeave').mockResolvedValue(undefined)
    const onLeft = vi.fn()
    render(<SalaScreen onLeft={onLeft} />)
    fireEvent.click(screen.getByRole('button', { name: 'Sair' }))
    expect(leave).toHaveBeenCalledTimes(1)
    await waitFor(() => expect(onLeft).toHaveBeenCalledTimes(1))
  })

  it('chat: enviar mensagem chama api.chatSend', () => {
    estado.snap = mockSnapshot()
    const send = vi.spyOn(api, 'chatSend').mockResolvedValue(undefined)
    render(<SalaScreen onLeft={() => {}} />)
    const campo = screen.getByPlaceholderText('Mensagem') as HTMLInputElement
    fireEvent.change(campo, { target: { value: 'oi' } })
    fireEvent.click(screen.getByRole('button', { name: 'Enviar' }))
    expect(send).toHaveBeenCalledWith('oi')
  })

  it('sala_encerrada: re-render com room null chama onLeft', () => {
    estado.snap = mockSnapshot()
    const onLeft = vi.fn()
    const { rerender } = render(<SalaScreen onLeft={onLeft} />)
    expect(onLeft).not.toHaveBeenCalled()
    estado.snap = { ...mockSnapshot(), room: null }
    rerender(<SalaScreen onLeft={onLeft} />)
    expect(onLeft).toHaveBeenCalledTimes(1)
  })
})
