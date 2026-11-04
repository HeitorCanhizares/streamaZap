// Tarefa sala-06
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render, screen, within } from '@testing-library/react'
import { PainelPessoas } from './PainelPessoas'
import type { Participant } from '../../bridge/types'

afterEach(() => cleanup())

const mock: Participant[] = [
  { id: 'p-marcao', name: 'Marcão', color: '#FFB547', ping_ms: 12, sharing: true, camera: false },
  { id: 'eu', name: 'Eu', color: '#3BF08A', ping_ms: 38, sharing: false, camera: false },
  { id: 'p-ju', name: 'Ju', color: '#7CF2C0', ping_ms: 120, sharing: false, camera: true },
]

describe('PainelPessoas', () => {
  it('renderiza os nomes e marca "(você)" só no meId', () => {
    render(<PainelPessoas participants={mock} meId="eu" hostId="p-marcao" />)
    expect(screen.getByText('Marcão')).toBeTruthy()
    expect(screen.getByText('Ju')).toBeTruthy()
    expect(screen.getByText('Eu (você)')).toBeTruthy()
    expect(screen.queryByText(/Marcão \(você\)/)).toBeNull()
    expect(screen.queryByText(/Ju \(você\)/)).toBeNull()
  })

  it('mostra o status: compartilhando a tela, webcam ligada e assistindo', () => {
    render(<PainelPessoas participants={mock} meId="eu" hostId="p-marcao" />)
    expect(screen.getByText('compartilhando a tela')).toBeTruthy()
    expect(screen.getByText('webcam ligada')).toBeTruthy()
    expect(screen.getAllByText('assistindo')).toHaveLength(1)
  })

  it('formata o ping em ms', () => {
    render(<PainelPessoas participants={mock} meId="eu" hostId="p-marcao" />)
    expect(screen.getByText('38 ms')).toBeTruthy()
    expect(screen.getByText('12 ms')).toBeTruthy()
    expect(screen.getByText('120 ms')).toBeTruthy()
  })

  it('mostra o selo host só no hostId', () => {
    render(<PainelPessoas participants={mock} meId="eu" hostId="p-marcao" />)
    const badges = screen.getAllByText('host')
    expect(badges).toHaveLength(1)
    const linha = badges[0].closest('li') as HTMLElement
    expect(within(linha).getByText('Marcão')).toBeTruthy()
  })

  it('sem hostId não mostra nenhum selo host', () => {
    render(<PainelPessoas participants={mock} meId="eu" />)
    expect(screen.queryByText('host')).toBeNull()
  })
})
