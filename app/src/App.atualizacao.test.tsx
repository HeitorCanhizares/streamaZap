// Tarefa front-02: App monta o aviso de atualização e respeita o Online desligado.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { App } from './App'
import { resetSnapshotForTests } from './bridge/store'
import { resetOnlineEnabled, setOnlineEnabled } from './bridge/onlineStore'

vi.mock('./components/shell/UpdateAviso', () => ({
  UpdateAviso: () => <div data-testid="update-aviso" />,
}))

afterEach(() => {
  cleanup()
  resetSnapshotForTests()
  resetOnlineEnabled()
})

describe('App (aviso de atualização e Online desligado)', () => {
  it('monta o UpdateAviso', () => {
    render(<App />)
    expect(screen.getByTestId('update-aviso')).toBeTruthy()
  })

  it('Online desligado: a barra não mostra "Servidor"', () => {
    setOnlineEnabled(false)
    render(<App />)
    expect(document.querySelector('header')?.textContent ?? '').not.toMatch(/servidor/i)
  })
})
