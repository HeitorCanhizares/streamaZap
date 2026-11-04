// Tarefa criar-04
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { mockSources } from '../../bridge/mock'
import { CriarFontes, type CriarFontesProps } from './CriarFontes'

afterEach(() => cleanup())

function props(over: Partial<CriarFontesProps> = {}): CriarFontesProps {
  return {
    sources: mockSources(),
    tab: 'telas',
    onTab: vi.fn(),
    selected: null,
    onSelect: vi.fn(),
    cameraCorner: false,
    onCameraCorner: vi.fn(),
    onRefresh: vi.fn(),
    ...over,
  }
}

describe('CriarFontes', () => {
  it('lista_telas: mostra os nomes das telas na aba Telas', () => {
    render(<CriarFontes {...props({ tab: 'telas' })} />)
    expect(screen.getByText('Tela 1 (1920x1080)')).toBeTruthy()
    expect(screen.getByText('Tela 2 (2560x1440)')).toBeTruthy()
  })

  it('troca_aba: clicar em Janelas chama onTab("janelas")', () => {
    const onTab = vi.fn()
    render(<CriarFontes {...props({ onTab })} />)
    fireEvent.click(screen.getByRole('tab', { name: 'Janelas' }))
    expect(onTab).toHaveBeenCalledWith('janelas')
  })

  it('selecao: clique no cartão chama onSelect com o id e o selecionado tem aria-pressed', () => {
    const onSelect = vi.fn()
    const { rerender } = render(<CriarFontes {...props({ onSelect })} />)
    const primeiro = screen.getByRole('button', { name: /Tela 1 \(1920x1080\)/ })
    expect(primeiro.getAttribute('aria-pressed')).toBe('false')
    fireEvent.click(primeiro)
    expect(onSelect).toHaveBeenCalledWith('screen:0')

    rerender(<CriarFontes {...props({ onSelect, selected: 'screen:0' })} />)
    expect(screen.getByRole('button', { name: /Tela 1 \(1920x1080\)/ }).getAttribute('aria-pressed')).toBe('true')
  })

  it('toggle_camera: clicar no Toggle chama onCameraCorner(true) quando estava false', () => {
    const onCameraCorner = vi.fn()
    render(<CriarFontes {...props({ cameraCorner: false, onCameraCorner })} />)
    fireEvent.click(screen.getByRole('switch'))
    expect(onCameraCorner).toHaveBeenCalledWith(true)
  })

  it('vazio: aba Webcam sem câmeras mostra "Nada encontrado" e esconde o Toggle', () => {
    render(<CriarFontes {...props({ tab: 'webcam', sources: { ...mockSources(), cameras: [] } })} />)
    expect(screen.getByText('Nada encontrado')).toBeTruthy()
    expect(screen.queryByRole('switch')).toBeNull()
  })

  it('abas: aria-selected na aba ativa', () => {
    render(<CriarFontes {...props({ tab: 'janelas' })} />)
    expect(screen.getByRole('tab', { name: 'Janelas' }).getAttribute('aria-selected')).toBe('true')
    expect(screen.getByRole('tab', { name: 'Telas' }).getAttribute('aria-selected')).toBe('false')
    expect(screen.getByRole('tab', { name: 'Webcam' })).toBeTruthy()
  })

  it('check: so o cartão ativo mostra o check', () => {
    render(<CriarFontes {...props({ selected: 'screen:0' })} />)
    expect(screen.getAllByTestId('fonte-check')).toHaveLength(1)
  })

  it('meta: mostra Tela inteira, Janela e Webcam', () => {
    const { unmount } = render(<CriarFontes {...props()} />)
    expect(screen.getAllByText('Tela inteira').length).toBeGreaterThan(0)
    unmount()
    const j = render(<CriarFontes {...props({ tab: 'janelas' })} />)
    expect(screen.getAllByText('Janela').length).toBeGreaterThan(0)
    j.unmount()
    render(<CriarFontes {...props({ tab: 'webcam' })} />)
    expect(screen.getAllByText('Webcam').length).toBeGreaterThan(0)
  })

  it('refresh: botão Atualizar chama onRefresh', () => {
    const onRefresh = vi.fn()
    render(<CriarFontes {...props({ onRefresh })} />)
    fireEvent.click(screen.getByRole('button', { name: 'Atualizar lista de telas e janelas' }))
    expect(onRefresh).toHaveBeenCalled()
  })

  it('titulo e legenda', () => {
    render(<CriarFontes {...props()} />)
    expect(screen.getByText('04')).toBeTruthy()
    expect(screen.getByText('O que compartilhar')).toBeTruthy()
    expect(screen.getByText('Sua câmera aparece por cima da tela, no canto inferior direito.')).toBeTruthy()
  })

  it('switch some na aba webcam', () => {
    render(<CriarFontes {...props({ tab: 'webcam' })} />)
    expect(screen.queryByRole('switch')).toBeNull()
  })
})
