// Tarefa salas-08
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { SalaPrevia } from './SalaPrevia'
import type { SalaVM } from './model'

afterEach(() => cleanup())

const base: SalaVM = {
  id: 'r1', nome: 'Sexta do Rush', host: 'Marcão', modo: 'online', addr: 'ZAP-7K2Q', trancada: false, versaoAviso: '',
  cena: 'game', camCena: 'cama', hora: '21:58', ultima: '', naoLidas: 0, digitando: false, ping: 38,
  qualidade: '1080p60', bitrate: '', assistindo: 7, compartilhando: [], pessoas: [],
}

describe('SalaPrevia', () => {
  it('com camCena mostra o PiP e duas cenas', () => {
    const { container } = render(<SalaPrevia sala={base} />)
    expect(screen.getByText('cam · Marcão')).toBeTruthy()
    const cenas = Array.from(container.querySelectorAll('svg[data-scene]')).map((s) => s.getAttribute('data-scene'))
    expect(cenas).toEqual(['game', 'cama'])
  })

  it('sem camCena não mostra o PiP', () => {
    render(<SalaPrevia sala={{ ...base, camCena: null }} />)
    expect(screen.queryByText(/cam ·/)).toBeNull()
  })

  it('mostra legenda, qualidade e AO VIVO', () => {
    render(<SalaPrevia sala={base} />)
    expect(screen.getByText('Prévia em baixa resolução, sem som. O áudio começa quando você entra.')).toBeTruthy()
    expect(screen.getByText('1080p60')).toBeTruthy()
    expect(screen.getByText('AO VIVO')).toBeTruthy()
  })

  it('com bitrate mostra qualidade · bitrate', () => {
    render(<SalaPrevia sala={{ ...base, bitrate: '6 Mbps' }} />)
    expect(screen.getByText('1080p60 · 6 Mbps')).toBeTruthy()
  })
})
