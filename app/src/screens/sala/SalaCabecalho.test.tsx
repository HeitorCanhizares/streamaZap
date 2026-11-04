// Tarefa sala-01
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { SalaCabecalho } from './SalaCabecalho'
import { mockSnapshot } from '../../bridge/mock'
import type { RoomView, TileView } from '../../bridge/types'

afterEach(() => cleanup())

const snap = () => mockSnapshot()
const mockRoom = (): RoomView => snap().room as RoomView
const mockMain = (): TileView => snap().tiles[0] as TileView

describe('SalaCabecalho', () => {
  it('renderiza nome da sala em h1 e métricas do main', () => {
    const room = mockRoom()
    render(<SalaCabecalho room={room} main={mockMain()} sharing={false} panelOpen={true} onOpenPanel={() => {}} />)
    const titulo = screen.getByRole('heading', { level: 1 })
    expect(titulo.textContent).toBe(room.name)
    expect(screen.getByText('via Radmin')).toBeTruthy()
    expect(screen.getByText(/suav\. \d+ ms/)).toBeTruthy()
    expect(screen.getByText(/Mbps$/)).toBeTruthy()
    expect(screen.getByText('1280x720@60')).toBeTruthy()
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

  it('main=null não quebra e omite as métricas', () => {
    const room = mockRoom()
    render(<SalaCabecalho room={room} main={null} sharing={false} panelOpen={true} onOpenPanel={() => {}} />)
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe(room.name)
    expect(screen.queryByText(/via /)).toBeNull()
    expect(screen.queryByText(/Mbps/)).toBeNull()
  })
})
