// Tarefa config-02
import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { ConfigSobre } from './ConfigSobre'
import { mockSettings } from '../../bridge/mock'

afterEach(() => {
  cleanup()
  vi.useRealTimers()
})

describe('ConfigSobre', () => {
  it('renderiza o título, a versão e a logo', () => {
    render(<ConfigSobre settings={mockSettings()} onChange={vi.fn()} />)
    expect(screen.getByRole('heading', { name: 'Sobre e atualizações' })).toBeTruthy()
    // O nome fica dividido em spans (Zap em verde), então a busca compara o texto completo do elemento.
    expect(screen.getByText((_, el) => el?.textContent === 'streamaZap')).toBeTruthy()
    expect(screen.getByText('versão 3.0.0-alpha.0 · protocolo 3 · licença MIT')).toBeTruthy()
    expect(screen.getByAltText('Logo do streamaZap')).toBeTruthy()
  })

  it('alternar o switch de atualização preserva o canal', () => {
    const onChange = vi.fn()
    const s = { ...mockSettings(), update: { auto: true, channel: 'beta' as const } }
    render(<ConfigSobre settings={s} onChange={onChange} />)
    const sw = screen.getByRole('switch')
    expect(sw.getAttribute('aria-checked')).toBe('true')
    fireEvent.click(sw)
    expect(onChange).toHaveBeenCalledWith({ update: { auto: false, channel: 'beta' } })
  })

  it('trocar o canal chama onChange com o update.channel novo', () => {
    const onChange = vi.fn()
    render(<ConfigSobre settings={{ ...mockSettings(), update: { auto: true, channel: 'estavel' } }} onChange={onChange} />)
    fireEvent.change(screen.getByRole('combobox'), { target: { value: 'beta' } })
    expect(onChange).toHaveBeenCalledWith({ update: { auto: true, channel: 'beta' } })
  })

  it('Procurar atualização mostra a verificação e volta ao estado normal após 1200 ms', () => {
    vi.useFakeTimers()
    render(<ConfigSobre settings={mockSettings()} onChange={vi.fn()} />)
    expect(screen.getByText('Você está na versão mais nova')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Procurar atualização' }))
    expect(screen.getByText('Procurando nas Releases…')).toBeTruthy()
    expect(screen.queryByText('Você está na versão mais nova')).toBeNull()
    act(() => {
      vi.advanceTimersByTime(1200)
    })
    expect(screen.queryByText('Procurando nas Releases…')).toBeNull()
    expect(screen.getByText('Você está na versão mais nova')).toBeTruthy()
  })

  it('desmontar com verificação pendente não deixa timer ativo', () => {
    vi.useFakeTimers()
    const { unmount } = render(<ConfigSobre settings={mockSettings()} onChange={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: 'Procurar atualização' }))
    unmount()
    expect(vi.getTimerCount()).toBe(0)
  })
})
