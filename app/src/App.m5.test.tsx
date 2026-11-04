// Tarefa ui5-07: aviso de atualização inserido no App.
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterAll, afterEach, beforeAll, describe, expect, it } from 'vitest'
import { App } from './App'
import { mockSnapshot } from './bridge/mock'
import { resetSnapshotForTests, setSnapshot } from './bridge/store'
import { resetUpdateState, setUpdateState } from './bridge/updateStore'

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
  resetUpdateState()
})

describe('App com aviso de atualização', () => {
  it('sem atualização o App renderiza como antes', () => {
    render(<App />)
    expect(screen.queryByText(/pronta/)).toBeNull()
    expect(screen.queryByRole('button', { name: 'Atualizar' })).toBeNull()
    expect(screen.getByText('Salas')).toBeTruthy()
  })

  it('com atualização pronta mostra o aviso no topo', () => {
    render(<App />)
    act(() => setUpdateState({ fase: 'pronta', progresso: 100, versao: '3.0.1', erro: null }))
    expect(screen.getByText('Atualização 3.0.1 pronta — instala ao fechar')).toBeTruthy()
  })

  it('com newer_protocol no snapshot, Atualizar abre Configurações', async () => {
    render(<App />)
    act(() => setSnapshot({ ...mockSnapshot(), newer_protocol: 4 }))
    fireEvent.click(screen.getByRole('button', { name: 'Atualizar' }))
    // As configurações carregam de forma assíncrona (mostra "Carregando…" antes).
    expect(await screen.findByText('Procurar atualização')).toBeTruthy()
  })
})
