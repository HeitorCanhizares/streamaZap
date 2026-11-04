// Tarefa sala-01
import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { SalaCabecalho } from './SalaCabecalho'
import { mockSnapshot } from '../../bridge/mock'
import type { RoomView, TileView } from '../../bridge/types'

afterEach(() => cleanup())

const snap = () => mockSnapshot()
const mockRoom = (): RoomView => snap().room as RoomView
const mockMain = (): TileView => snap().tiles[0] as TileView

describe('SalaCabecalho', () => {
  it('renderiza nome da sala em h1', () => {
    const room = mockRoom()
    render(<SalaCabecalho room={room} main={mockMain()} sharing={false} panelOpen={true} onOpenPanel={() => {}} />)
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe(room.name)
  })

  it('mostra chip ONLINE · ZAP-7K2Q com o room do mock', () => {
    render(<SalaCabecalho room={mockRoom()} main={mockMain()} sharing={false} panelOpen={true} onOpenPanel={() => {}} />)
    expect(screen.getByText('ONLINE · ZAP-7K2Q')).toBeTruthy()
  })

  it('mostra Assistindo Marcão · Tela com main = tiles[0]', () => {
    const { container } = render(
      <SalaCabecalho room={mockRoom()} main={mockMain()} sharing={false} panelOpen={true} onOpenPanel={() => {}} />,
    )
    expect(container.textContent).toContain('Assistindo Marcão · Tela')
  })

  it('sem main nao mostra Assistindo', () => {
    render(<SalaCabecalho room={mockRoom()} main={null} sharing={false} panelOpen={true} onOpenPanel={() => {}} />)
    expect(screen.queryByText(/Assistindo/)).toBeNull()
    expect(screen.getByText('ONLINE · ZAP-7K2Q')).toBeTruthy()
  })

  it('nao mostra mais Mbps nem ping no cabecalho', () => {
    render(<SalaCabecalho room={mockRoom()} main={mockMain()} sharing={false} panelOpen={true} onOpenPanel={() => {}} />)
    expect(screen.queryByText(/Mbps/)).toBeNull()
    expect(screen.queryByText(/Ping/)).toBeNull()
  })

  it('Copiar convite chama clipboard.writeText com invite e muda o texto para Convite copiado', () => {
    vi.useFakeTimers()
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true })
    const onCopied = vi.fn()
    render(
      <SalaCabecalho
        room={mockRoom()}
        main={mockMain()}
        sharing={false}
        panelOpen={true}
        onOpenPanel={() => {}}
        invite="convite X"
        onCopied={onCopied}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: 'Copiar convite' }))
    expect(writeText).toHaveBeenCalledWith('convite X')
    expect(onCopied).toHaveBeenCalledTimes(1)
    expect(screen.getByRole('button', { name: 'Convite copiado' })).toBeTruthy()
    act(() => {
      vi.advanceTimersByTime(2000)
    })
    expect(screen.getByRole('button', { name: 'Copiar convite' })).toBeTruthy()
    vi.useRealTimers()
  })

  it('sharing=true mostra selo Compartilhando; sharing=false não', () => {
    const { unmount } = render(
      <SalaCabecalho room={mockRoom()} main={mockMain()} sharing={true} panelOpen={true} onOpenPanel={() => {}} />,
    )
    expect(screen.getByText('Compartilhando')).toBeTruthy()
    unmount()
    render(<SalaCabecalho room={mockRoom()} main={mockMain()} sharing={false} panelOpen={true} onOpenPanel={() => {}} />)
    expect(screen.queryByText('Compartilhando')).toBeNull()
  })

  it('panelOpen=false mostra botão Painel e clique chama onOpenPanel', () => {
    const onOpenPanel = vi.fn()
    render(<SalaCabecalho room={mockRoom()} main={mockMain()} sharing={false} panelOpen={false} onOpenPanel={onOpenPanel} />)
    fireEvent.click(screen.getByRole('button', { name: 'Painel' }))
    expect(onOpenPanel).toHaveBeenCalledTimes(1)
  })

  it('panelOpen=true não mostra botão Painel', () => {
    render(<SalaCabecalho room={mockRoom()} main={mockMain()} sharing={false} panelOpen={true} onOpenPanel={() => {}} />)
    expect(screen.queryByRole('button', { name: 'Painel' })).toBeNull()
  })
})
