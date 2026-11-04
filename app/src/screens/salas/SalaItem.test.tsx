// Tarefa salas-04
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { SalaItem } from './SalaItem'
import type { SalaVM } from './model'

afterEach(() => cleanup())

const base: SalaVM = {
  id: 'r1', nome: 'Sexta do Rush', host: 'Rafa', modo: 'online', addr: 'ZAP-7K2Q', trancada: false, versaoAviso: '',
  cena: 'game', camCena: null, hora: '21:58', ultima: 'Rafa: bora', naoLidas: 3, digitando: false, ping: 38,
  qualidade: '1080p60', bitrate: '', assistindo: 7, compartilhando: [], pessoas: [],
}

describe('SalaItem', () => {
  it('renderiza os dados da sala', () => {
    render(<SalaItem sala={base} selecionada={false} onSelect={() => {}} />)
    for (const t of ['Sexta do Rush', '21:58', 'Rafa: bora', '3', '1080p60', '38 ms', '7']) {
      expect(screen.getByText(t)).toBeTruthy()
    }
  })

  it('selecionada marca aria-pressed e fundo', () => {
    render(<SalaItem sala={base} selecionada onSelect={() => {}} />)
    const b = screen.getByRole('button')
    expect(b.getAttribute('aria-pressed')).toBe('true')
    expect(b.style.background).toBe('rgb(18, 41, 31)')
  })

  it('digitando substitui a ultima mensagem', () => {
    render(<SalaItem sala={{ ...base, digitando: true }} selecionada={false} onSelect={() => {}} />)
    expect(screen.getByText('Rafa está digitando…')).toBeTruthy()
    expect(screen.queryByText('Rafa: bora')).toBeNull()
  })

  it('cadeado so quando trancada', () => {
    const { rerender } = render(<SalaItem sala={{ ...base, trancada: true }} selecionada={false} onSelect={() => {}} />)
    expect(screen.getByLabelText('com senha')).toBeTruthy()
    rerender(<SalaItem sala={base} selecionada={false} onSelect={() => {}} />)
    expect(screen.queryByLabelText('com senha')).toBeNull()
  })

  it('mostra versao de aviso', () => {
    render(<SalaItem sala={{ ...base, versaoAviso: '0.0.9' }} selecionada={false} onSelect={() => {}} />)
    expect(screen.getByText('v0.0.9')).toBeTruthy()
  })

  it('sem nao lidas nao mostra bolha', () => {
    render(<SalaItem sala={{ ...base, naoLidas: 0 }} selecionada={false} onSelect={() => {}} />)
    expect(screen.queryByText('3')).toBeNull()
  })

  it('clique chama onSelect com o id', () => {
    const fn = vi.fn()
    render(<SalaItem sala={base} selecionada={false} onSelect={fn} />)
    fireEvent.click(screen.getByRole('button'))
    expect(fn).toHaveBeenCalledWith('r1')
  })
})
