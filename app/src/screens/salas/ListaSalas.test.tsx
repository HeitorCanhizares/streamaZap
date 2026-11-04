// Tarefa salas-05
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { ListaSalas } from './ListaSalas'
import type { ListaSalasProps } from './ListaSalas'
import type { SalaVM } from './model'

afterEach(() => cleanup())

const sala = (id: string, nome: string, trancada: boolean): SalaVM => ({
  id, nome, host: 'Rafa', modo: 'online', addr: 'ZAP-7K2Q', trancada, versaoAviso: '',
  cena: 'game', camCena: null, hora: '21:58', ultima: 'oi', naoLidas: 0, digitando: false, ping: 38,
  qualidade: '1080p60', bitrate: '', assistindo: 7, compartilhando: [], pessoas: [],
})

function props(extra: Partial<ListaSalasProps> = {}): ListaSalasProps {
  return {
    modo: 'online', onModo: vi.fn(), salas: [sala('a', 'Sala Alfa', false), sala('b', 'Sala Beta', true)],
    contagemOnline: 2, contagemLocal: 0, contagens: { todas: 2, abertas: 1, senha: 1 },
    filtro: 'todas', onFiltro: vi.fn(), busca: '', onBusca: vi.fn(), selecionadaId: null, onSelect: vi.fn(),
    apelido: 'Heitor', onApelido: vi.fn(), onApelidoConfirmar: vi.fn(), onCriar: vi.fn(), onEntrarManual: vi.fn(),
    ...extra,
  }
}

describe('ListaSalas', () => {
  it('renderiza cabecalho, filtros, salas e apelido', () => {
    render(<ListaSalas {...props()} />)
    expect(screen.getByRole('heading', { name: 'Salas' })).toBeTruthy()
    for (const t of ['2 salas', 'Nova sala', 'Todas · 2', 'Abertas · 1', 'Com senha · 1', 'Sala Alfa', 'Sala Beta', 'Você aparece como']) {
      expect(screen.getByText(t)).toBeTruthy()
    }
    expect(screen.getByPlaceholderText('Buscar sala ou pessoa')).toBeTruthy()
    expect((screen.getByLabelText('Você aparece como') as HTMLInputElement).value).toBe('Heitor')
  })

  it('Nova sala chama onCriar', () => {
    const p = props()
    render(<ListaSalas {...p} />)
    fireEvent.click(screen.getByText('Nova sala'))
    expect(p.onCriar).toHaveBeenCalled()
  })

  it('busca chama onBusca', () => {
    const p = props()
    render(<ListaSalas {...p} />)
    fireEvent.change(screen.getByPlaceholderText('Buscar sala ou pessoa'), { target: { value: 'x' } })
    expect(p.onBusca).toHaveBeenCalledWith('x')
  })

  it('filtro chama onFiltro', () => {
    const p = props()
    render(<ListaSalas {...p} />)
    fireEvent.click(screen.getByText('Abertas · 1'))
    expect(p.onFiltro).toHaveBeenCalledWith('abertas')
  })

  it('lista vazia mostra mensagem', () => {
    render(<ListaSalas {...props({ salas: [] })} />)
    expect(screen.getByText('Nenhuma sala com esse nome.')).toBeTruthy()
  })

  it('modo local', () => {
    const p = props({ modo: 'local' })
    render(<ListaSalas {...p} />)
    expect(screen.getByText('Procurando mais salas no Radmin e na rede local…')).toBeTruthy()
    fireEvent.click(screen.getByText('Entrar por IP'))
    expect(p.onEntrarManual).toHaveBeenCalled()
  })

  it('modo online', () => {
    render(<ListaSalas {...props()} />)
    expect(screen.getByText('Entrar com código')).toBeTruthy()
  })

  it('blur e Enter no apelido confirmam', () => {
    const p = props()
    render(<ListaSalas {...p} />)
    const i = screen.getByLabelText('Você aparece como')
    fireEvent.blur(i)
    fireEvent.keyDown(i, { key: 'Enter' })
    expect(p.onApelidoConfirmar).toHaveBeenCalledTimes(2)
  })

  it('clicar numa sala chama onSelect', () => {
    const p = props()
    render(<ListaSalas {...p} />)
    fireEvent.click(screen.getByText('Sala Alfa'))
    expect(p.onSelect).toHaveBeenCalledWith('a')
  })
})
