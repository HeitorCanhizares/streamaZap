// Tarefa criar-07
import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { CriarResumo, type CriarResumoProps } from './CriarResumo'
import type { BarraServidor } from '../../bridge/serverModel'

afterEach(() => {
  cleanup()
  vi.useRealTimers()
})

const SERVIDOR: BarraServidor = { texto: 'Servidor sa-east · 42 ms', tom: 'ok', ms: 42 }

function props(over: Partial<CriarResumoProps> = {}): CriarResumoProps {
  return {
    modo: 'online', nome: 'Sala do Heitor', acesso: 'aberta', fonte: 'Tela inteira',
    cena: 'game', mostrarPip: false, som: 'Sem som', qualidade: '720p30 · 2,5 Mbps',
    radminIp: '26.12.34.56', codigo: 'ZAP-7K2Q', servidor: SERVIDOR, ...over,
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
  })

  it('frase inclui servidor.texto quando passado', () => {
    render(<CriarResumo {...props()} />)
    expect(screen.getByText(/Servidor sa-east · 42 ms\./)).toBeTruthy()
  })

  it('sem servidor mostra so a frase', () => {
    render(<CriarResumo {...props({ servidor: null })} />)
    expect(screen.getByText('Quem tiver o app entra com esse código, de qualquer lugar.')).toBeTruthy()
    expect(screen.queryByText(/Servidor sa-east/)).toBeNull()
  })

  it('online sem codigo mostra mascara e botao desativado', () => {
    render(<CriarResumo {...props({ codigo: null })} />)
    expect(screen.getByText('ZAP-····')).toBeTruthy()
    const botao = screen.getByText('Disponível ao criar a sala').closest('button')
    expect(botao).not.toBeNull()
    expect(botao?.disabled).toBe(true)
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

  it('online copia o convite e mostra Copiado!', () => {
    vi.useFakeTimers()
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true })
    render(<CriarResumo {...props()} />)
    fireEvent.click(screen.getByText('Copiar convite'))
    expect(writeText).toHaveBeenCalledWith('Entra na minha sala "Sala do Heitor" no streamaZap: https://heitorcanhizares.github.io/streamaZap/sala/#ZAP-7K2Q (código ZAP-7K2Q)')
    expect(screen.getByText('Copiado!')).toBeTruthy()
    act(() => { vi.advanceTimersByTime(1500) })
    expect(screen.getByText('Copiar convite')).toBeTruthy()
  })

  it('semTransmitir true troca a prévia e esconde som e vídeo', () => {
    render(<CriarResumo {...props({ semTransmitir: true, som: 'Sem som', qualidade: '720p30 · 2,5 Mbps' })} />)
    expect(screen.getByText('Você entra na sala sem transmitir')).toBeTruthy()
    expect(screen.getByText('Sem transmissão por enquanto')).toBeTruthy()
    expect(screen.queryByText(/Todo o som do PC|Sem som|720p30/)).toBeNull()
    expect(screen.queryByText('Fonte')).toBeNull()
    expect(screen.getByText('Modo')).toBeTruthy()
    expect(screen.getByText('Acesso')).toBeTruthy()
  })

  it('sem semTransmitir a prévia e o som/qualidade seguem como antes', () => {
    render(<CriarResumo {...props({ som: 'Sem som', qualidade: '720p30 · 2,5 Mbps' })} />)
    expect(screen.queryByText('Sem transmissão por enquanto')).toBeNull()
    expect(screen.queryByText('Você entra na sala sem transmitir')).toBeNull()
    expect(screen.getByText('Sem som')).toBeTruthy()
    expect(screen.getByText('720p30 · 2,5 Mbps')).toBeTruthy()
  })
})
