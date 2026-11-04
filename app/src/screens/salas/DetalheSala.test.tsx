// Tarefa salas-12
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { DetalheSala, type DetalheSalaProps } from './DetalheSala'
import type { SalaVM } from './model'

afterEach(() => cleanup())

const sala: SalaVM = {
  id: 'r1', nome: 'Sexta do Rush', host: 'Rafa', modo: 'online', addr: 'ZAP-7K2Q', trancada: false, versaoAviso: '',
  cena: 'game', camCena: null, hora: '21:58', ultima: 'Rafa: bora', naoLidas: 3, digitando: false, ping: 38,
  qualidade: '1080p60', bitrate: '', assistindo: 4, compartilhando: [], pessoas: [],
}

const base: DetalheSalaProps = {
  sala: null,
  mostrarFirewall: false,
  onCorrigirFirewall: () => {},
  senha: '',
  onSenha: () => {},
  ocupado: false,
  erro: null,
  onEntrar: () => {},
}

describe('DetalheSala', () => {
  it('sala null mostra a dica para escolher uma sala', () => {
    render(<DetalheSala {...base} />)
    expect(screen.getByText('Escolha uma sala na lista')).toBeTruthy()
    expect(screen.queryByRole('heading', { level: 2 })).toBeNull()
  })

  it('com sala mostra o nome, as seções e o botão de entrar', () => {
    render(<DetalheSala {...base} sala={sala} />)
    expect(screen.getByRole('heading', { level: 2 }).textContent).toBe('Sexta do Rush')
    expect(screen.getByText(/^Compartilhando agora/)).toBeTruthy()
    expect(screen.getByText(/^Na sala/)).toBeTruthy()
    expect(screen.getByText('Entrar na sala')).toBeTruthy()
    expect(screen.queryByText('Escolha uma sala na lista')).toBeNull()
  })

  it('mostrarFirewall true mostra o aviso e false não', () => {
    const { unmount } = render(<DetalheSala {...base} sala={sala} mostrarFirewall />)
    expect(screen.getByText('Corrigir firewall')).toBeTruthy()
    unmount()
    render(<DetalheSala {...base} sala={sala} mostrarFirewall={false} />)
    expect(screen.queryByText('Corrigir firewall')).toBeNull()
  })

  it('sala null com firewall ainda mostra o aviso', () => {
    render(<DetalheSala {...base} mostrarFirewall />)
    expect(screen.getByRole('alert')).toBeTruthy()
    expect(screen.getByText('Corrigir firewall')).toBeTruthy()
    expect(screen.getByText('Escolha uma sala na lista')).toBeTruthy()
  })
})
