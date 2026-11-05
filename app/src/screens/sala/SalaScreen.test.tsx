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
    expect(screen.getByRole('tab', { name: 'Chat' })).toBeTruthy()
    expect(screen.getByRole('tab', { name: /Pessoas/ })).toBeTruthy()
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
    fireEvent.click(screen.getByRole('button', { name: 'Enviar mensagem' }))
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

  it('layout: mostra controles, grupo de transmissões e painel lateral', () => {
    estado.snap = mockSnapshot()
    render(<SalaScreen onLeft={() => {}} />)
    expect(screen.getByRole('toolbar', { name: 'Controles da sala' })).toBeTruthy()
    expect(screen.getByRole('group', { name: 'Transmissões na sala' })).toBeTruthy()
    expect(screen.getByRole('complementary', { name: 'Chat e pessoas' })).toBeTruthy()
  })

  it('chip ONLINE · ZAP-7K2Q aparece', () => {
    estado.snap = mockSnapshot()
    render(<SalaScreen onLeft={() => {}} />)
    expect(screen.getByText('ONLINE · ZAP-7K2Q')).toBeTruthy()
  })

  it('Copiar convite grava o texto do convite com o codigo ZAP-7K2Q no clipboard', () => {
    estado.snap = mockSnapshot()
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true })
    render(<SalaScreen onLeft={() => {}} />)
    fireEvent.click(screen.getByRole('button', { name: 'Copiar convite' }))
    expect(writeText).toHaveBeenCalledTimes(1)
    expect(writeText.mock.calls[0][0]).toContain('ZAP-7K2Q')
  })

  it('assina sozinho as transmissões dos outros: a primeira tela grande, o resto pequeno embaixo', () => {
    estado.snap = mockSnapshot()
    const watch = vi.spyOn(api, 'watch').mockResolvedValue(undefined)
    render(<SalaScreen onLeft={() => {}} />)
    // Marcão (tela) na principal; Dudu (janela) e Ju (câmera) na faixa, só a camada pequena.
    expect(watch).toHaveBeenCalledWith('trk-1', expect.any(Number), [1920, 1080], null, expect.any(Function))
    expect(watch).toHaveBeenCalledWith('trk-2', expect.any(Number), [320, 180], 2, expect.any(Function))
    expect(watch).toHaveBeenCalledWith('trk-3', expect.any(Number), [320, 180], 2, expect.any(Function))
    expect(screen.getAllByTestId('celula-principal')).toHaveLength(1)
    expect(screen.getAllByTestId('cartao-faixa')).toHaveLength(2)
  })

  it('pôr outra na principal divide a tela sem abrir outra assinatura', () => {
    estado.snap = mockSnapshot()
    const watch = vi.spyOn(api, 'watch').mockResolvedValue(undefined)
    const unwatch = vi.spyOn(api, 'unwatch').mockResolvedValue(undefined)
    render(<SalaScreen onLeft={() => {}} />)
    fireEvent.click(screen.getByRole('button', { name: 'Pôr janela de Dudu na tela principal' }))
    expect(screen.getAllByTestId('celula-principal')).toHaveLength(2)
    expect(watch).toHaveBeenLastCalledWith('trk-2', expect.any(Number), [960, 540], null, expect.any(Function))
    expect(unwatch).not.toHaveBeenCalled()
  })

  it('volume geral vai para o núcleo; sem transmitir não há cartão Você', () => {
    estado.snap = mockSnapshot()
    const master = vi.spyOn(api, 'setMasterVolume').mockResolvedValue(undefined)
    render(<SalaScreen onLeft={() => {}} />)
    expect(master).toHaveBeenLastCalledWith(1, false)
    fireEvent.click(screen.getByRole('button', { name: 'Silenciar a sala toda' }))
    expect(master).toHaveBeenLastCalledWith(1, true)
    expect(screen.queryByText('Você')).toBeNull()
  })

  it('painel: Fechar painel remove o aside e o botão Painel do cabeçalho o traz de volta', () => {
    estado.snap = mockSnapshot()
    render(<SalaScreen onLeft={() => {}} />)
    fireEvent.click(screen.getByRole('button', { name: 'Fechar painel' }))
    expect(screen.queryByRole('complementary', { name: 'Chat e pessoas' })).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: 'Painel' }))
    expect(screen.getByRole('complementary', { name: 'Chat e pessoas' })).toBeTruthy()
  })
})
