// Tarefa salas-03
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { ModoAbas } from './ModoAbas'

afterEach(() => cleanup())

describe('ModoAbas', () => {
  it('mostra os rótulos e as contagens de cada modo', () => {
    render(<ModoAbas modo="online" onModo={() => {}} contagemOnline={2} contagemLocal={1} />)
    expect(screen.getByText('Online')).toBeTruthy()
    expect(screen.getByText('via servidor · 2')).toBeTruthy()
    expect(screen.getByText('Local')).toBeTruthy()
    expect(screen.getByText('Radmin / rede local · 1')).toBeTruthy()
  })

  it('a aba do modo ativo tem aria-selected true e a outra false', () => {
    const { rerender } = render(<ModoAbas modo="online" onModo={() => {}} contagemOnline={2} contagemLocal={1} />)
    const online = screen.getByRole('tab', { name: /Online/ })
    const local = screen.getByRole('tab', { name: /Local/ })
    expect(online.getAttribute('aria-selected')).toBe('true')
    expect(local.getAttribute('aria-selected')).toBe('false')

    rerender(<ModoAbas modo="local" onModo={() => {}} contagemOnline={2} contagemLocal={1} />)
    expect(screen.getByRole('tab', { name: /Online/ }).getAttribute('aria-selected')).toBe('false')
    expect(screen.getByRole('tab', { name: /Local/ }).getAttribute('aria-selected')).toBe('true')
  })

  it('clicar em Local chama onModo com "local"', () => {
    const onModo = vi.fn()
    render(<ModoAbas modo="online" onModo={onModo} contagemOnline={2} contagemLocal={1} />)
    fireEvent.click(screen.getByRole('tab', { name: /Local/ }))
    expect(onModo).toHaveBeenCalledWith('local')
  })

  it('o container é uma tablist com aria-label "Onde procurar salas"', () => {
    render(<ModoAbas modo="online" onModo={() => {}} contagemOnline={0} contagemLocal={0} />)
    const lista = screen.getByRole('tablist')
    expect(lista.getAttribute('aria-label')).toBe('Onde procurar salas')
  })
})
