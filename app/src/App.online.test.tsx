// Tarefa ui-online-15: integração do modo Online no App, com o mock do navegador (sem Tauri).
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { afterAll, afterEach, beforeAll, describe, expect, it } from 'vitest'
import { App } from './App'
import { resetSnapshotForTests } from './bridge/store'

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

const cabecalho = (): string => document.querySelector('header')?.textContent ?? ''
const principal = (): HTMLElement => document.querySelector('main') as HTMLElement

describe('App · modo Online (integração no mock)', () => {
  it('barra de título mostra o servidor online com o ping do mock', () => {
    render(<App />)
    expect(cabecalho()).toContain('Servidor online · 42 ms')
  })

  it('Salas: aba Online ativa e lista a sala "Sexta do Rush" com o código ZAP-7K2Q', () => {
    render(<App />)
    const abaOnline = screen.getByRole('tab', { name: /Online/ })
    expect(abaOnline.getAttribute('aria-selected')).toBe('true')
    expect(screen.getAllByText('Sexta do Rush').length).toBeGreaterThan(0)
    expect(screen.getAllByText('ZAP-7K2Q').length).toBeGreaterThan(0)
  })

  it('Salas: "Entrar com código" abre o campo "Código da sala"', () => {
    render(<App />)
    fireEvent.click(screen.getByRole('button', { name: 'Entrar com código' }))
    expect(screen.getByLabelText('Código da sala')).toBeTruthy()
  })

  it('Nova sala: no modo Online o resumo mostra ZAP-7K2Q e o botão Copiar convite', () => {
    render(<App />)
    fireEvent.click(screen.getByRole('button', { name: 'Nova sala' }))
    fireEvent.click(screen.getByRole('radio', { name: /Online/ }))
    expect(principal().textContent).toContain('ZAP-7K2Q')
    expect(screen.getByRole('button', { name: 'Copiar convite' })).toBeTruthy()
  })

  it('Configurações: Testar conexão com endereço e senha leva a "Conectado…"', async () => {
    render(<App />)
    fireEvent.click(screen.getByRole('button', { name: 'Configurações' }))

    // As configurações carregam de forma assíncrona: espera o campo do endereço aparecer.
    const endereco = await screen.findByPlaceholderText('ex.: zap.seudominio.com:7443')
    fireEvent.change(endereco, { target: { value: 'zap.teste.local:7443' } })
    fireEvent.change(screen.getByPlaceholderText('pede pro dono do servidor'), { target: { value: 'segredo' } })

    fireEvent.click(screen.getByRole('button', { name: 'Testar conexão' }))

    const status = await screen.findByText(/^Conectado/)
    expect(status.closest('[role="status"]')).not.toBeNull()
  })

  it('Diagnóstico: o card "Servidor online" mostra "Conectado"', async () => {
    render(<App />)
    fireEvent.click(screen.getByRole('button', { name: 'Diagnóstico de rede' }))
    // O relatório do Diagnóstico chega de forma assíncrona: antes disso o card ainda é o provisório.
    expect(await within(principal()).findByText(/^Conectado/)).toBeTruthy()
  })
})
