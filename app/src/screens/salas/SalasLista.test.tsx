// Tarefa ui-12
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import type { RoomListing } from '../../bridge/types'
import { SalasLista } from './SalasLista'

afterEach(() => cleanup())

const salas: RoomListing[] = [
  {
    room_id: 'r1',
    name: 'Sexta do Rush',
    host: 'Ana',
    participants: 4,
    has_password: false,
    mode: 'local',
    live_title: 'Rush final',
  },
  {
    room_id: 'r2',
    name: 'Filminho de domingo',
    host: 'Bruno',
    participants: 2,
    has_password: true,
    mode: 'local',
  },
  {
    room_id: 'r3',
    name: 'Live de Python',
    host: 'Carla',
    participants: 9,
    has_password: false,
    mode: 'local',
  },
]

function renderLista(over: Partial<Parameters<typeof SalasLista>[0]> = {}) {
  const props = {
    rooms: salas,
    selectedId: null,
    onSelect: vi.fn(),
    query: '',
    onQuery: vi.fn(),
    filter: 'todas' as const,
    onFilter: vi.fn(),
    ...over,
  }
  const utils = render(<SalasLista {...props} />)
  return { ...utils, props }
}

describe('SalasLista', () => {
  it('renderiza_titulo_e_salas', () => {
    const { container } = renderLista()
    const texto = container.textContent ?? ''
    expect(texto).toContain('Salas')
    expect(texto).toContain('Sexta do Rush')
    expect(texto).toContain('Filminho de domingo')
    expect(texto).toContain('Live de Python')
  })

  it('filtro_senha: com filter=senha só aparecem salas com senha', () => {
    renderLista({ filter: 'senha' })
    expect(screen.getByText('Filminho de domingo')).toBeTruthy()
    expect(screen.queryByText('Sexta do Rush')).toBeNull()
    expect(screen.queryByText('Live de Python')).toBeNull()
  })

  it('filtro abertas: esconde salas com senha', () => {
    renderLista({ filter: 'abertas' })
    expect(screen.queryByText('Filminho de domingo')).toBeNull()
    expect(screen.getByText('Sexta do Rush')).toBeTruthy()
  })

  it('busca: query=rush mostra só Sexta do Rush', () => {
    renderLista({ query: 'rush' })
    expect(screen.getByText('Sexta do Rush')).toBeTruthy()
    expect(screen.queryByText('Filminho de domingo')).toBeNull()
    expect(screen.queryByText('Live de Python')).toBeNull()
  })

  it('busca ignora acento e caixa', () => {
    renderLista({ query: 'FILMINHO' })
    expect(screen.getByText('Filminho de domingo')).toBeTruthy()
    expect(screen.queryByText('Sexta do Rush')).toBeNull()
  })

  it('busca ignora acento', () => {
    renderLista({ query: 'sexta do rúsh'.replace('ú', 'u') })
    expect(screen.getByText('Sexta do Rush')).toBeTruthy()
    renderLista({ query: 'filmínho' })
    expect(screen.getAllByText('Filminho de domingo').length).toBeGreaterThan(0)
  })

  it('clique: clicar no cartão chama onSelect com room_id', () => {
    const { props } = renderLista()
    fireEvent.click(screen.getByRole('button', { name: /Live de Python/ }))
    expect(props.onSelect).toHaveBeenCalledWith('r3')
  })

  it('marca o cartão selecionado com aria-pressed', () => {
    renderLista({ selectedId: 'r1' })
    expect(screen.getByRole('button', { name: /Sexta do Rush/ }).getAttribute('aria-pressed')).toBe(
      'true',
    )
    expect(
      screen.getByRole('button', { name: /Live de Python/ }).getAttribute('aria-pressed'),
    ).toBe('false')
  })

  it('vazio: query sem match mostra Nenhuma sala encontrada', () => {
    renderLista({ query: 'xyzabc' })
    expect(screen.getByText('Nenhuma sala encontrada')).toBeTruthy()
  })

  it('campo de busca chama onQuery ao digitar', () => {
    const { props } = renderLista()
    fireEvent.change(screen.getByPlaceholderText('Buscar sala ou pessoa'), {
      target: { value: 'live' },
    })
    expect(props.onQuery).toHaveBeenCalledWith('live')
  })
})
