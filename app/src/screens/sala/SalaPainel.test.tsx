// Tarefa sala-07
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { SalaPainel } from './SalaPainel'
import { mockSnapshot } from '../../bridge/mock'
import type { RoomView } from '../../bridge/types'

afterEach(() => cleanup())

const mockRoom = (): RoomView => mockSnapshot().room as RoomView

describe('SalaPainel', () => {
  it('cabeçalho: abas Chat e Pessoas · N (N = participants.length do mock)', () => {
    render(<SalaPainel room={mockRoom()} onSend={() => {}} />)
    expect(screen.getByRole('tab', { name: 'Chat' })).toBeTruthy()
    expect(screen.getByRole('tab', { name: 'Pessoas · 4' })).toBeTruthy()
    expect(screen.getByRole('tab', { name: 'Chat' }).getAttribute('aria-selected')).toBe('true')
  })

  it('chat_lista: mostra autores e textos das mensagens do mock', () => {
    render(<SalaPainel room={mockRoom()} onSend={() => {}} />)
    expect(screen.getByText('Ju')).toBeTruthy()
    expect(screen.getByText('Rafa')).toBeTruthy()
    expect(screen.getByText('cheguei! tá liso aqui, 38 ms')).toBeTruthy()
    expect(screen.getByText('abri o mapa, clica no meu quad')).toBeTruthy()
  })

  it('hora: ts em milissegundos vira HH:MM', () => {
    const room = mockRoom()
    room.chat = [{ id: 'x', author_id: 'p-ju', author: 'Ju', color: '#7CF2C0', text: 'oi', ts: 1767312600000 }]
    render(<SalaPainel room={room} onSend={() => {}} />)
    const d = new Date(1767312600000)
    const esperado = `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
    expect(screen.getByText(esperado)).toBeTruthy()
  })

  it('meId: mensagem do próprio usuário ganha ✓✓; sem meId nenhuma ganha', () => {
    const room = mockRoom()
    const { unmount } = render(<SalaPainel room={room} onSend={() => {}} meId="p-ju" />)
    expect(screen.getAllByText('✓✓')).toHaveLength(1)
    unmount()
    render(<SalaPainel room={room} onSend={() => {}} />)
    expect(screen.queryAllByText('✓✓')).toHaveLength(0)
  })

  it('envio: digitar " oi " e clicar Enviar mensagem chama onSend("oi") e limpa o campo', () => {
    const onSend = vi.fn()
    render(<SalaPainel room={mockRoom()} onSend={onSend} />)
    const campo = screen.getByPlaceholderText('Mensagem') as HTMLInputElement
    fireEvent.change(campo, { target: { value: ' oi ' } })
    fireEvent.click(screen.getByRole('button', { name: 'Enviar mensagem' }))
    expect(onSend).toHaveBeenCalledWith('oi')
    expect(onSend).toHaveBeenCalledTimes(1)
    expect(campo.value).toBe('')
  })

  it('enter: Enter no campo chama onSend e limpa', () => {
    const onSend = vi.fn()
    render(<SalaPainel room={mockRoom()} onSend={onSend} />)
    const campo = screen.getByPlaceholderText('Mensagem') as HTMLInputElement
    fireEvent.change(campo, { target: { value: 'bora' } })
    fireEvent.keyDown(campo, { key: 'Enter' })
    expect(onSend).toHaveBeenCalledWith('bora')
    expect(campo.value).toBe('')
  })

  it('vazio: só espaços não chama onSend', () => {
    const onSend = vi.fn()
    render(<SalaPainel room={mockRoom()} onSend={onSend} />)
    const campo = screen.getByPlaceholderText('Mensagem')
    fireEvent.change(campo, { target: { value: '   ' } })
    fireEvent.click(screen.getByRole('button', { name: 'Enviar mensagem' }))
    fireEvent.keyDown(campo, { key: 'Enter' })
    expect(onSend).not.toHaveBeenCalled()
  })

  it('trocar aba: clicar em Pessoas mostra os participantes e some o chat', () => {
    render(<SalaPainel room={mockRoom()} onSend={() => {}} />)
    fireEvent.click(screen.getByRole('tab', { name: 'Pessoas · 4' }))
    expect(screen.getByRole('tab', { name: 'Pessoas · 4' }).getAttribute('aria-selected')).toBe('true')
    expect(screen.queryByRole('log', { name: 'Mensagens' })).toBeNull()
    expect(screen.getByText('Marcão')).toBeTruthy()
    expect(screen.getByText('Ju')).toBeTruthy()
    expect(screen.getByText('Rafa')).toBeTruthy()
    expect(screen.getByText('Dudu')).toBeTruthy()
    // Marcão e Dudu têm sharing=true no mock: status de tela compartilhada.
    expect(screen.getAllByText('compartilhando a tela')).toHaveLength(2)
  })

  it('unread: room.unread=3 mostra 3 na aba Chat', () => {
    const room = mockRoom()
    room.unread = 3
    render(<SalaPainel room={room} onSend={() => {}} />)
    expect(screen.getByRole('tab', { name: /Chat/ }).textContent).toContain('3')
  })

  it('unread: sem não lidas não mostra selo', () => {
    render(<SalaPainel room={mockRoom()} onSend={() => {}} />)
    expect(screen.getByRole('tab', { name: 'Chat' }).querySelector('.zp-badge')).toBeNull()
  })

  it('fechar: botão Fechar painel chama onClose', () => {
    const onClose = vi.fn()
    render(<SalaPainel room={mockRoom()} onSend={() => {}} onClose={onClose} />)
    fireEvent.click(screen.getByRole('button', { name: 'Fechar painel' }))
    expect(onClose).toHaveBeenCalledTimes(1)
  })
})
