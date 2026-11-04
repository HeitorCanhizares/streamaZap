import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react'
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
    expect(document.querySelector('header')?.textContent).toContain('streamaZap')
    expect(screen.getByText('Salas')).toBeTruthy()
    expect(screen.getAllByText('Sexta do Rush').length).toBeGreaterThan(0)
  })

  it('navega_config', () => {
    render(<App />)
    fireEvent.click(screen.getByRole('button', { name: 'Configurações' }))
    // O breadcrumb também diz "Configurações"; o h1 confirma que a tela abriu.
    expect(screen.getByRole('heading', { name: 'Configurações' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Configurações' }).getAttribute('aria-current')).toBe('page')
  })

  it('navega_diag', () => {
    render(<App />)
    fireEvent.click(screen.getByRole('button', { name: 'Diagnóstico de rede' }))
    // "Diagnóstico" (exato) é o sobretítulo da tela; o breadcrumb diz "Diagnóstico de rede".
    expect(screen.getByText('Diagnóstico')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Diagnóstico de rede' }).getAttribute('aria-current')).toBe('page')
  })

  it('criar', () => {
    render(<App />)
    fireEvent.click(screen.getByRole('button', { name: 'Nova sala' }))
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
    expect(screen.getByRole('button', { name: 'Sala atual' }).getAttribute('aria-current')).toBe('page')

    // Sala some estando em Sala: volta para a tela Salas (com o botão Nova sala).
    act(() => setSnapshot({ ...base, version: base.version + 3, room: null }))
    expect(within(document.querySelector('main') as HTMLElement).getByRole('button', { name: 'Nova sala' })).toBeTruthy()
  })

  it('trilho_em_sala', () => {
    render(<App />)
    const base = getSnapshot()
    const trilho = within(screen.getByRole('navigation', { name: 'Navegação principal' }))

    // Sem sala: o trilho oferece "Criar sala".
    act(() => setSnapshot({ ...base, version: base.version + 1, room: null }))
    expect(trilho.getByRole('button', { name: 'Criar sala' })).toBeTruthy()
    expect(trilho.queryByRole('button', { name: 'Sala atual' })).toBeNull()

    // Com a sala do mock: "Sala atual" no lugar de "Criar sala".
    act(() => setSnapshot({ ...base, version: base.version + 2 }))
    expect(trilho.getByRole('button', { name: 'Sala atual' })).toBeTruthy()
    expect(screen.queryByRole('button', { name: 'Criar sala' })).toBeNull()
  })

  it('trilho_salas_com_sala_ativa', () => {
    render(<App />)
    const base = getSnapshot()
    act(() => setSnapshot({ ...base, version: base.version + 1, room: null }))
    act(() => setSnapshot({ ...base, version: base.version + 2 }))
    const header = (): string => document.querySelector('header')?.textContent ?? ''
    const main = (): HTMLElement => document.querySelector('main') as HTMLElement

    // Entrou na sala: breadcrumb com o nome dela e métricas no lugar do status do servidor.
    expect(header()).toContain('Salas / Sexta do Rush')
    expect(header()).toContain('via servidor sa-east')
    expect(header()).not.toContain('Servidor online')

    // "Salas" leva à lista mesmo com sala ativa.
    fireEvent.click(screen.getByRole('button', { name: 'Salas' }))
    expect(screen.getByRole('button', { name: 'Salas' }).getAttribute('aria-current')).toBe('page')
    expect(within(main()).getByRole('button', { name: 'Nova sala' })).toBeTruthy()
    expect(header()).toContain('Servidor online')

    // "Sala atual" volta para a sala.
    fireEvent.click(screen.getByRole('button', { name: 'Sala atual' }))
    expect(screen.getByRole('button', { name: 'Sala atual' }).getAttribute('aria-current')).toBe('page')
    expect(header()).toContain('Salas / Sexta do Rush')
  })

  it('breadcrumb_criar', () => {
    render(<App />)
    const base = getSnapshot()
    act(() => setSnapshot({ ...base, version: base.version + 1, room: null }))
    const trilho = within(screen.getByRole('navigation', { name: 'Navegação principal' }))
    fireEvent.click(trilho.getByRole('button', { name: 'Criar sala' }))
    expect(trilho.getByRole('button', { name: 'Criar sala' }).getAttribute('aria-current')).toBe('page')
    expect(document.querySelector('header')?.textContent).toContain('Salas / Nova sala')
  })

  it('breadcrumb_config', () => {
    render(<App />)
    fireEvent.click(screen.getByRole('button', { name: 'Configurações' }))
    expect(document.querySelector('header')?.textContent).toContain('Configurações')
  })

  it('status_no_mock', () => {
    render(<App />)
    const txt = document.querySelector('header')?.textContent
    expect(txt).toContain('Servidor online · 42 ms')
    expect(txt).toContain('Radmin · 26.14.7.88')
  })
})
