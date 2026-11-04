import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { getUpdateState, nadaState, resetUpdateState, setUpdateState, useUpdateState } from './updateStore'

// Mock do módulo de eventos do Tauri: `listen` guarda o handler para o teste disparar eventos.
const { listen } = vi.hoisted(() => ({ listen: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({ listen }))

// Componente de teste: mostra "fase versao" (ex.: "pronta 3.0.1").
function Estado() {
  const s = useUpdateState()
  return <p>{`${s.fase} ${s.versao ?? ''}`.trim()}</p>
}

beforeEach(() => {
  resetUpdateState()
  listen.mockReset()
})

afterEach(() => {
  // Desmonta os componentes de cada teste: inscritos a mais impediriam o listen de subir.
  cleanup()
  delete (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
})

describe('store de atualização', () => {
  it('estado inicial é fase nada com progresso 0', () => {
    expect(getUpdateState()).toEqual(nadaState())
    expect(getUpdateState().fase).toBe('nada')
    expect(getUpdateState().progresso).toBe(0)
  })

  it('setUpdateState faz o componente re-renderizar com o novo estado', () => {
    render(<Estado />)
    act(() => {
      setUpdateState({ fase: 'pronta', progresso: 100, versao: '3.0.1', erro: null })
    })
    expect(screen.getByText('pronta 3.0.1')).toBeTruthy()
  })

  it('getUpdateState devolve a mesma referência enquanto nada muda', () => {
    expect(getUpdateState()).toBe(getUpdateState())
  })

  it('fora do Tauri não escuta nenhum evento', () => {
    const { unmount } = render(<Estado />)
    expect(listen).not.toHaveBeenCalled()
    unmount()
  })

  it('dentro do Tauri escuta zap://update e aplica o payload recebido', async () => {
    (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {}
    let handler: ((e: { payload: unknown }) => void) | undefined
    listen.mockImplementation(async (_nome: string, h: (e: { payload: unknown }) => void) => {
      handler = h
      return () => {}
    })

    render(<Estado />)
    await vi.waitFor(() => expect(listen).toHaveBeenCalledWith('zap://update', expect.any(Function)))
    await act(async () => {})

    act(() => {
      handler?.({ payload: { fase: 'baixando', progresso: 42, versao: '3.0.1', erro: null } })
    })
    expect(getUpdateState()).toEqual({ fase: 'baixando', progresso: 42, versao: '3.0.1', erro: null })
    expect(screen.getByText('baixando 3.0.1')).toBeTruthy()
  })

  it('ao desmontar o último componente desfaz o listen', async () => {
    (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {}
    const unlisten = vi.fn()
    listen.mockResolvedValue(unlisten)

    const { unmount } = render(<Estado />)
    await vi.waitFor(() => expect(listen).toHaveBeenCalled())
    await act(async () => {})

    unmount()
    expect(unlisten).toHaveBeenCalledTimes(1)
  })
})
