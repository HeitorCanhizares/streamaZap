// Tarefa salas-11
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { SalaNaSala } from './SalaNaSala'
import type { SalaVM } from './model'

afterEach(() => cleanup())

const base: SalaVM = {
  id: 'r1', nome: 'Sexta do Rush', host: 'Rafa', modo: 'online', addr: 'ZAP-7K2Q', trancada: false, versaoAviso: '',
  cena: 'game', camCena: null, hora: '21:58', ultima: 'Rafa: bora', naoLidas: 0, digitando: false, ping: 38,
  qualidade: '1080p60', bitrate: '', assistindo: 7, compartilhando: [], pessoas: [],
}

describe('SalaNaSala', () => {
  it('mostra titulo com contagem, nomes, papeis e ping', () => {
    const sala: SalaVM = {
      ...base,
      pessoas: [
        { name: 'Rafa', role: 'host · compartilhando', ping_ms: 12 },
        { name: 'Bia', role: 'assistindo', ping_ms: 112 },
        { name: 'Léo', role: 'assistindo', ping_ms: 45 },
        { name: 'Ju', role: 'assistindo', ping_ms: 70 },
      ],
    }
    render(<SalaNaSala sala={sala} />)
    expect(screen.getByText('Na sala · 4')).toBeTruthy()
    for (const t of ['Rafa', 'Bia', 'Léo', 'Ju']) expect(screen.getByText(t)).toBeTruthy()
    expect(screen.getByText('host · compartilhando')).toBeTruthy()
    expect(screen.getByText('12 ms')).toBeTruthy()
  })

  it('colore o ping pela faixa (112 vermelho, 45 verde)', () => {
    const sala: SalaVM = {
      ...base,
      pessoas: [
        { name: 'Bia', role: 'assistindo', ping_ms: 112 },
        { name: 'Léo', role: 'assistindo', ping_ms: 45 },
      ],
    }
    render(<SalaNaSala sala={sala} />)
    // jsdom devolve a cor em rgb(...): #FF8592 = rgb(255, 133, 146), #3BF08A = rgb(59, 240, 138).
    expect((screen.getByText('112 ms') as HTMLElement).style.color).toBe('rgb(255, 133, 146)')
    expect((screen.getByText('45 ms') as HTMLElement).style.color).toBe('rgb(59, 240, 138)')
  })

  it('a inicial do avatar e a primeira letra maiuscula', () => {
    const sala: SalaVM = {
      ...base,
      pessoas: [{ name: 'bia', role: 'assistindo', ping_ms: 30 }],
    }
    const { container } = render(<SalaNaSala sala={sala} />)
    const avatar = container.querySelector('li > span') as HTMLElement
    expect(avatar.textContent).toBe('B')
  })
})
