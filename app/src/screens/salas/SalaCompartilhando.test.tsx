// Tarefa salas-10
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { SalaCompartilhando } from './SalaCompartilhando'
import type { SalaVM } from './model'

afterEach(() => cleanup())

const base: SalaVM = {
  id: 'r1', nome: 'Sexta do Rush', host: 'Rafa', modo: 'online', addr: 'ZAP-7K2Q', trancada: false, versaoAviso: '',
  cena: 'game', camCena: null, hora: '21:58', ultima: 'Rafa: bora', naoLidas: 0, digitando: false, ping: 38,
  qualidade: '1080p60', bitrate: '', assistindo: 7, compartilhando: [], pessoas: [],
}

describe('SalaCompartilhando', () => {
  it('mostra titulo com contagem, nomes, tipos e cenas', () => {
    const sala: SalaVM = {
      ...base,
      compartilhando: [
        { name: 'Rafa', kind: 'Tela', scene: 'game' },
        { name: 'Bia', kind: 'Webcam', scene: 'cama' },
        { name: 'Léo', kind: 'Janela', scene: 'code' },
      ],
    }
    const { container } = render(<SalaCompartilhando sala={sala} />)
    expect(screen.getByText('Compartilhando agora · 3')).toBeTruthy()
    for (const t of ['Rafa', 'Bia', 'Léo']) expect(screen.getByText(t)).toBeTruthy()
    expect(screen.getByText('· Tela')).toBeTruthy()
    expect(screen.getByText('· Webcam')).toBeTruthy()
    expect(screen.getByText('· Janela')).toBeTruthy()
    const cenas = Array.from(container.querySelectorAll('svg[data-scene]')).map((el) => el.getAttribute('data-scene'))
    expect(cenas).toEqual(['game', 'cama', 'code'])
  })

  it('lista vazia mostra contagem 0 e nenhuma miniatura', () => {
    const { container } = render(<SalaCompartilhando sala={base} />)
    expect(screen.getByText('Compartilhando agora · 0')).toBeTruthy()
    expect(container.querySelectorAll('svg[data-scene]').length).toBe(0)
  })
})
