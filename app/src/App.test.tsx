import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterAll, afterEach, beforeAll, describe, expect, it } from 'vitest'
import { App } from './App'
import { getSnapshot, resetSnapshotForTests, setSnapshot } from './bridge/store'

// jsdom não tem WebCodecs: a tela Sala cria um VideoDecoder ao montar, então damos um falso.
const semVideoDecoder = !('VideoDecoder' in globalThis)
beforeAll(() => {
  if (semVideoDecoder) {
    ;(globalThis as Record<string, unknown>).VideoDecoder = class {
      configure(): void {}
      decode(): void {}
      close(): void {}
    }
  }
})
afterAll(() => {
  if (semVideoDecoder) delete (globalThis as Record<string, unknown>).VideoDecoder
})

afterEach(() => {
  cleanup()
  resetSnapshotForTests()
})

describe('App', () => {
  it('renderiza_salas', () => {
    render(<App />)
    expect(screen.getByText('streamaZap')).toBeTruthy()
    expect(screen.getByText('Salas')).toBeTruthy()
    expect(screen.getAllByText('Sexta do Rush').length).toBeGreaterThan(0)
  })

  it('navega_config', () => {
    render(<App />)
    fireEvent.click(screen.getByRole('button', { name: 'Configuracoes' }))
    expect(screen.getByText('Configurações')).toBeTruthy()
  })

  it('navega_diag', () => {
    render(<App />)
    fireEvent.click(screen.getByRole('button', { name: 'Diagnostico' }))
    expect(screen.getByText('Diagnóstico')).toBeTruthy()
  })

  it('criar', () => {
    render(<App />)
    fireEvent.click(screen.getByRole('button', { name: 'Criar sala' }))
    expect(screen.getByText('Bora transmitir?')).toBeTruthy()
  })

  it('vai para a sala quando a sala aparece e volta às salas quando ela some', () => {
    render(<App />)
    const base = getSnapshot()

    // Sala some: a view atual é Salas, então continua em Salas.
    act(() => setSnapshot({ ...base, version: base.version + 1, room: null }))
    expect(screen.getByText('Salas')).toBeTruthy()

    // Sala aparece (outro dono criou): leva para a sala.
    act(() => setSnapshot({ ...base, version: base.version + 2 }))
    expect(screen.queryByText('Bora transmitir?')).toBeNull()
    expect(screen.getByRole('button', { name: 'Salas' }).getAttribute('aria-current')).toBe('page')

    // Sala some estando em Sala: volta para a tela Salas (com o botão Criar sala).
    act(() => setSnapshot({ ...base, version: base.version + 3, room: null }))
    expect(screen.getByRole('button', { name: 'Criar sala' })).toBeTruthy()
  })
})
