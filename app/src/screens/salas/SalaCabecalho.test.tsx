// Tarefa salas-07
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { SalaCabecalho } from './SalaCabecalho'
import type { SalaVM } from './model'

afterEach(() => cleanup())

const base: SalaVM = {
  id: 'r1', nome: 'Sexta do Rush', host: 'Rafa', modo: 'online', addr: 'ZAP-7K2Q', trancada: false, versaoAviso: '',
  cena: 'game', camCena: null, hora: '21:58', ultima: 'Rafa: bora', naoLidas: 3, digitando: false, ping: 38,
  qualidade: '1080p60', bitrate: '', assistindo: 4, compartilhando: [], pessoas: [],
}

describe('SalaCabecalho', () => {
  it('renderiza nome, rótulo, host e endereço', () => {
    render(<SalaCabecalho sala={base} />)
    expect(screen.getByRole('heading', { level: 2 }).textContent).toBe('Sexta do Rush')
    expect(screen.getByText('Sala selecionada')).toBeTruthy()
    expect(screen.getByText(/^por/)).toBeTruthy()
    expect(screen.getByText('Rafa')).toBeTruthy()
    expect(screen.getByText('ZAP-7K2Q')).toBeTruthy()
  })

  it('modo online mostra ONLINE e local mostra LOCAL', () => {
    const { unmount } = render(<SalaCabecalho sala={base} />)
    expect(screen.getByText('ONLINE')).toBeTruthy()
    expect(screen.queryByText('LOCAL')).toBeNull()
    unmount()
    render(<SalaCabecalho sala={{ ...base, modo: 'local' }} />)
    expect(screen.getByText('LOCAL')).toBeTruthy()
    expect(screen.queryByText('ONLINE')).toBeNull()
  })

  it('mostra AO VIVO, a qualidade e a contagem de assistindo', () => {
    render(<SalaCabecalho sala={base} />)
    expect(screen.getByText('AO VIVO')).toBeTruthy()
    expect(screen.getByText('1080p60')).toBeTruthy()
    expect(screen.getByText('4 assistindo')).toBeTruthy()
  })

  it('endereço vazio não renderiza o separador " · "', () => {
    const { container } = render(<SalaCabecalho sala={{ ...base, addr: '' }} />)
    expect(container.textContent).not.toContain('·')
    expect(screen.getByText('Rafa')).toBeTruthy()
  })
})
