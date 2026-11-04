// Tarefa criar-07
import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { CriarResumo, type CriarResumoProps } from './CriarResumo'

afterEach(() => {
  cleanup()
  vi.useRealTimers()
})

function props(over: Partial<CriarResumoProps> = {}): CriarResumoProps {
  return {
    modo: 'online', nome: 'Sala do Heitor', acesso: 'aberta', fonte: 'Tela inteira',
    cena: 'game', mostrarPip: false, som: 'Sem som', qualidade: '720p30 · 2,5 Mbps',
    radminIp: '26.12.34.56', ...over,
  }
}

describe('CriarResumo', () => {
  it('mostra titulo e ficha', () => {
    render(<CriarResumo {...props()} />)
    expect(screen.getByText('É isso que a galera vai ver')).toBeTruthy()
    for (const t of ['Online · via servidor', 'Sala do Heitor', 'Aberta para a rede', 'Tela inteira', 'Sem som', '720p30 · 2,5 Mbps'])
      expect(screen.getByText(t)).toBeTruthy()
    expect(screen.getByText('Conexão com o servidor autenticada')).toBeTruthy()
  })

  it('modo online mostra codigo', () => {
    render(<CriarResumo {...props()} />)
    expect(screen.getByText('Código da sala')).toBeTruthy()
    expect(screen.getByText('ZAP-7K2Q')).toBeTruthy()
    expect(screen.getByText(/Servidor sa-east · 42 ms/)).toBeTruthy()
  })

  it('modo local mostra enderecos', () => {
    render(<CriarResumo {...props({ modo: 'local' })} />)
    expect(screen.getByText('Seus endereços')).toBeTruthy()
    expect(screen.getByText('26.12.34.56:47800')).toBeTruthy()
    expect(screen.getByText('192.168.0.12:47800')).toBeTruthy()
    expect(screen.getByText('Radmin')).toBeTruthy()
    expect(screen.getByText('Firewall liberado para receber conexões')).toBeTruthy()
  })

  it('sem radminIp omite Radmin', () => {
    render(<CriarResumo {...props({ modo: 'local', radminIp: null })} />)
    expect(screen.queryByText('Radmin')).toBeNull()
    expect(screen.getByText('rede local')).toBeTruthy()
  })

  it('mostrarPip alterna a segunda cena', () => {
    const { container, rerender } = render(<CriarResumo {...props()} />)
    expect(container.querySelectorAll('svg[data-scene]').length).toBe(1)
    rerender(<CriarResumo {...props({ mostrarPip: true })} />)
    expect(container.querySelectorAll('svg[data-scene]').length).toBe(2)
    expect(container.querySelector('svg[data-scene="camc"]')).not.toBeNull()
  })

  it('nome vazio mostra Sua sala', () => {
    render(<CriarResumo {...props({ nome: '  ' })} />)
    expect(screen.getByText('Sua sala')).toBeTruthy()
  })

  it('Copiar convite copia e vira Copiado!', () => {
    vi.useFakeTimers()
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true })
    render(<CriarResumo {...props({ modo: 'local' })} />)
    fireEvent.click(screen.getByText('Copiar convite'))
    expect(writeText).toHaveBeenCalledWith('26.12.34.56:47800\n192.168.0.12:47800')
    expect(screen.getByText('Copiado!')).toBeTruthy()
    act(() => { vi.advanceTimersByTime(1500) })
    expect(screen.getByText('Copiar convite')).toBeTruthy()
  })

  it('online copia o codigo', () => {
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true })
    render(<CriarResumo {...props()} />)
    fireEvent.click(screen.getByText('Copiar convite'))
    expect(writeText).toHaveBeenCalledWith('ZAP-7K2Q')
  })
})
