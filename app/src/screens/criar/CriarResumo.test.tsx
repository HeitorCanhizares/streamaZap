// Tarefa ui-17
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { CriarResumo, type CriarResumoProps } from './CriarResumo'

afterEach(() => cleanup())

function props(over: Partial<CriarResumoProps> = {}): CriarResumoProps {
  return {
    roomName: 'Sala do Heitor',
    sourceLabel: 'Tela inteira',
    sound: 'Sem som',
    preset: '1080p60',
    encoder: 'auto',
    busy: false,
    onStart: vi.fn(),
    ...over,
  }
}

describe('CriarResumo', () => {
  it('mostra_valores: renderiza roomName, sourceLabel, sound e "1080p · 60"', () => {
    render(<CriarResumo {...props()} />)
    expect(screen.getByText('Sala do Heitor')).toBeTruthy()
    expect(screen.getByText('Tela inteira')).toBeTruthy()
    expect(screen.getByText('Sem som')).toBeTruthy()
    expect(screen.getByText('1080p · 60')).toBeTruthy()
    expect(screen.getByText('Automático')).toBeTruthy()
    expect(screen.getByText('AO VIVO')).toBeTruthy()
  })

  it('nome_vazio: roomName="" mostra "Sua sala"', () => {
    render(<CriarResumo {...props({ roomName: '' })} />)
    expect(screen.getByText('Sua sala')).toBeTruthy()
  })

  it('botao: clicar em "Iniciar transmissão" chama onStart', () => {
    const onStart = vi.fn()
    render(<CriarResumo {...props({ onStart })} />)
    fireEvent.click(screen.getByRole('button', { name: 'Iniciar transmissão' }))
    expect(onStart).toHaveBeenCalledTimes(1)
  })

  it('busy: busy=true desabilita o botão e mostra "Iniciando…"', () => {
    const onStart = vi.fn()
    render(<CriarResumo {...props({ busy: true, onStart })} />)
    const botao = screen.getByRole('button', { name: 'Iniciando…' }) as HTMLButtonElement
    expect(botao.disabled).toBe(true)
    fireEvent.click(botao)
    expect(onStart).not.toHaveBeenCalled()
  })

  it('sem_fonte: sourceLabel="" deixa o botão desabilitado', () => {
    render(<CriarResumo {...props({ sourceLabel: '' })} />)
    const botao = screen.getByRole('button', { name: 'Iniciar transmissão' }) as HTMLButtonElement
    expect(botao.disabled).toBe(true)
  })
})
