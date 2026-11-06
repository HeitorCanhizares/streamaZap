// Tarefa front-02: a barra de título só mostra o servidor com o Online ligado.
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { TitleBar } from './TitleBar'
import { resetOnlineEnabled, setOnlineEnabled } from '../bridge/onlineStore'

afterEach(() => {
  cleanup()
  resetOnlineEnabled()
})

describe('TitleBar · modo Online', () => {
  it('Online desligado: sem texto de servidor', () => {
    setOnlineEnabled(false)
    render(<TitleBar server={{ state: 'online', ping_ms: 42 }} />)
    expect(screen.queryByText(/servidor/i)).toBeNull()
  })

  it('Online ligado: mostra o ServerStatus como antes', () => {
    setOnlineEnabled(true)
    render(<TitleBar server={{ state: 'online', ping_ms: 42 }} />)
    expect(screen.getByText(/Servidor online · 42 ms/)).toBeTruthy()
  })

  it('Online desligado com `right`: o `right` aparece', () => {
    setOnlineEnabled(false)
    render(<TitleBar right={<span>métricas</span>} />)
    expect(screen.getByText('métricas')).toBeTruthy()
  })
})
