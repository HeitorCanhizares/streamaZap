// Tarefa ui-21
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { SalaPainel } from './SalaPainel'
import { mockSnapshot } from '../../bridge/mock'
import type { RoomView } from '../../bridge/types'

afterEach(() => cleanup())

const mockRoom = (): RoomView => mockSnapshot().room as RoomView

describe('SalaPainel', () => {
  it('chat_lista: mostra autores e textos das mensagens do mock', () => {
    render(<SalaPainel room={mockRoom()} onSend={() => {}} />)
    expect(screen.getByText('Ju')).toBeTruthy()
    expect(screen.getByText('Rafa')).toBeTruthy()
    expect(screen.getByText('cheguei! tá liso aqui, 38 ms')).toBeTruthy()
    expect(screen.getByText('abri o mapa, clica no meu quad')).toBeTruthy()
  })

  it('hora: ts em segundos epoch vira HH:MM', () => {
    const room = mockRoom()
    room.chat = [{ id: 'x', author_id: 'p-ju', author: 'Ju', color: '#7CF2C0', text: 'oi', ts: 1767312600 }]
    render(<SalaPainel room={room} onSend={() => {}} />)
    const d = new Date(1767312600 * 1000)
    const esperado = `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
    expect(screen.getByText(esperado)).toBeTruthy()
  })

  it('envio: digitar " oi " e clicar Enviar chama onSend("oi") e limpa o campo', () => {
    const onSend = vi.fn()
    render(<SalaPainel room={mockRoom()} onSend={onSend} />)
    const campo = screen.getByPlaceholderText('Mensagem') as HTMLInputElement
    fireEvent.change(campo, { target: { value: ' oi ' } })
    fireEvent.click(screen.getByRole('button', { name: 'Enviar' }))
    expect(onSend).toHaveBeenCalledWith('oi')
    expect(onSend).toHaveBeenCalledTimes(1)
    expect(campo.value).toBe('')
  })

  it('enter: Enter também envia', () => {
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
    fireEvent.click(screen.getByRole('button', { name: 'Enviar' }))
    fireEvent.keyDown(campo, { key: 'Enter' })
    expect(onSend).not.toHaveBeenCalled()
  })

  it('pessoas: clicar na aba Pessoas mostra os participantes e marca transmitindo', () => {
    render(<SalaPainel room={mockRoom()} onSend={() => {}} />)
    fireEvent.click(screen.getByRole('tab', { name: 'Pessoas' }))
    expect(screen.getByText('Marcão')).toBeTruthy()
    expect(screen.getByText('Ju')).toBeTruthy()
    expect(screen.getByText('Rafa')).toBeTruthy()
    expect(screen.getByText('Dudu')).toBeTruthy()
    // Marcão e Dudu têm sharing=true no mock.
    expect(screen.getAllByText('transmitindo')).toHaveLength(2)
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
})
