// Tarefa sala-05
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { PainelChat } from './PainelChat'
import type { ChatLine } from '../../bridge/types'

afterEach(() => cleanup())

const chat: ChatLine[] = [
  { id: '1', author_id: 'system', author: '', color: '#fff', text: 'Ju entrou', ts: 1767312000000 },
  { id: '2', author_id: 'p-ju', author: 'Ju', color: '#7CF2C0', text: 'cheguei', ts: 1767312600000 },
  { id: '3', author_id: 'eu', author: 'Eu', color: '#fff', text: 'opa', ts: Number.NaN },
]

describe('PainelChat', () => {
  it('renderiza mensagens, HOJE e ✓✓ só na própria', () => {
    render(<PainelChat chat={chat} meId="eu" onSend={vi.fn()} />)
    expect(screen.getByRole('log', { name: 'Mensagens' })).toBeTruthy()
    expect(screen.getByText('HOJE')).toBeTruthy()
    expect(screen.getByText('Ju entrou')).toBeTruthy()
    expect(screen.getByText('cheguei')).toBeTruthy()
    expect(screen.getByText('Ju')).toBeTruthy()
    expect(screen.getByText('opa')).toBeTruthy()
    expect(screen.getAllByText('✓✓')).toHaveLength(1)
    expect(screen.getByText(/--:--/)).toBeTruthy()
  })

  it('Enter envia texto aparado e limpa; vazio não envia', () => {
    const onSend = vi.fn()
    render(<PainelChat chat={[]} meId="eu" onSend={onSend} />)
    const input = screen.getByPlaceholderText('Mensagem') as HTMLInputElement
    fireEvent.change(input, { target: { value: '   ' } })
    fireEvent.keyDown(input, { key: 'Enter' })
    expect(onSend).not.toHaveBeenCalled()
    fireEvent.change(input, { target: { value: '  oi  ' } })
    fireEvent.keyDown(input, { key: 'Enter' })
    expect(onSend).toHaveBeenCalledWith('oi')
    expect(input.value).toBe('')
  })

  it('botão envia', () => {
    const onSend = vi.fn()
    render(<PainelChat chat={[]} meId="eu" onSend={onSend} />)
    fireEvent.change(screen.getByPlaceholderText('Mensagem'), { target: { value: 'x' } })
    fireEvent.click(screen.getByRole('button', { name: 'Enviar mensagem' }))
    expect(onSend).toHaveBeenCalledWith('x')
  })

  it('typing mostra o indicador', () => {
    const { rerender } = render(<PainelChat chat={[]} meId="eu" onSend={vi.fn()} />)
    expect(screen.queryByTestId('digitando')).toBeNull()
    rerender(<PainelChat chat={[]} meId="eu" onSend={vi.fn()} typing="Ju" />)
    expect(screen.getByTestId('digitando')).toBeTruthy()
    expect(screen.getByText('Ju está digitando…')).toBeTruthy()
  })
})
