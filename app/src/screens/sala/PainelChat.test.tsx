// Tarefa sala-05
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
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

describe('PainelChat digitação (chat_typing)', () => {
  beforeEach(() => vi.useFakeTimers())
  afterEach(() => vi.useRealTimers())

  function digitar(input: HTMLInputElement, valor: string): void {
    fireEvent.change(input, { target: { value: valor } })
  }

  it('digitar a primeira letra chama onTyping(true) uma vez', () => {
    const onTyping = vi.fn()
    render(<PainelChat chat={[]} meId="eu" onSend={vi.fn()} onTyping={onTyping} />)
    const input = screen.getByPlaceholderText('Mensagem') as HTMLInputElement
    digitar(input, 'a')
    expect(onTyping).toHaveBeenCalledTimes(1)
    expect(onTyping).toHaveBeenCalledWith(true)
  })

  it('digitar mais letras não repete onTyping(true)', () => {
    const onTyping = vi.fn()
    render(<PainelChat chat={[]} meId="eu" onSend={vi.fn()} onTyping={onTyping} />)
    const input = screen.getByPlaceholderText('Mensagem') as HTMLInputElement
    digitar(input, 'a')
    digitar(input, 'ab')
    digitar(input, 'abc')
    expect(onTyping).toHaveBeenCalledTimes(1)
    expect(onTyping).toHaveBeenCalledWith(true)
  })

  it('após 3 s parado chama onTyping(false)', () => {
    const onTyping = vi.fn()
    render(<PainelChat chat={[]} meId="eu" onSend={vi.fn()} onTyping={onTyping} />)
    const input = screen.getByPlaceholderText('Mensagem') as HTMLInputElement
    digitar(input, 'oi')
    vi.advanceTimersByTime(2999)
    expect(onTyping).toHaveBeenCalledTimes(1)
    vi.advanceTimersByTime(1)
    expect(onTyping).toHaveBeenCalledTimes(2)
    expect(onTyping).toHaveBeenLastCalledWith(false)
  })

  it('digitar de novo reinicia o tempo de 3 s e depois avisa true de novo', () => {
    const onTyping = vi.fn()
    render(<PainelChat chat={[]} meId="eu" onSend={vi.fn()} onTyping={onTyping} />)
    const input = screen.getByPlaceholderText('Mensagem') as HTMLInputElement
    digitar(input, 'o')
    vi.advanceTimersByTime(2000)
    digitar(input, 'oi')
    vi.advanceTimersByTime(2000)
    expect(onTyping).toHaveBeenCalledTimes(1)
    vi.advanceTimersByTime(1000)
    expect(onTyping.mock.calls).toEqual([[true], [false]])
  })

  it('enviar chama onTyping(false) e onSend', () => {
    const onTyping = vi.fn()
    const onSend = vi.fn()
    render(<PainelChat chat={[]} meId="eu" onSend={onSend} onTyping={onTyping} />)
    const input = screen.getByPlaceholderText('Mensagem') as HTMLInputElement
    digitar(input, 'oi')
    fireEvent.click(screen.getByRole('button', { name: 'Enviar mensagem' }))
    expect(onTyping).toHaveBeenLastCalledWith(false)
    expect(onTyping.mock.calls).toEqual([[true], [false]])
    expect(onSend).toHaveBeenCalledWith('oi')
    vi.advanceTimersByTime(5000)
    expect(onTyping).toHaveBeenCalledTimes(2)
  })

  it('esvaziar o campo chama onTyping(false) na hora', () => {
    const onTyping = vi.fn()
    render(<PainelChat chat={[]} meId="eu" onSend={vi.fn()} onTyping={onTyping} />)
    const input = screen.getByPlaceholderText('Mensagem') as HTMLInputElement
    digitar(input, 'oi')
    digitar(input, '')
    expect(onTyping.mock.calls).toEqual([[true], [false]])
  })

  it('unmount limpa o timer sem chamar onTyping', () => {
    const onTyping = vi.fn()
    const { unmount } = render(<PainelChat chat={[]} meId="eu" onSend={vi.fn()} onTyping={onTyping} />)
    digitar(screen.getByPlaceholderText('Mensagem') as HTMLInputElement, 'oi')
    unmount()
    vi.advanceTimersByTime(5000)
    expect(onTyping.mock.calls).toEqual([[true]])
  })

  it('sem onTyping nada quebra', () => {
    const onSend = vi.fn()
    render(<PainelChat chat={[]} meId="eu" onSend={onSend} />)
    const input = screen.getByPlaceholderText('Mensagem') as HTMLInputElement
    digitar(input, 'oi')
    fireEvent.click(screen.getByRole('button', { name: 'Enviar mensagem' }))
    vi.advanceTimersByTime(5000)
    expect(onSend).toHaveBeenCalledWith('oi')
  })
})
