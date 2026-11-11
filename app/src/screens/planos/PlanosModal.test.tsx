import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('../../bridge/contaClient', () => ({ clienteAtual: vi.fn() }))

import { api } from '../../bridge/api'
import { clienteAtual } from '../../bridge/contaClient'
import { criarMockConta, PRECOS_MOCK } from '../../bridge/mockConta'
import { getPlanoEstado, resetPlano, setPlanoEstado } from '../../bridge/planoStore'
import { PlanosModal } from './PlanosModal'

const CONFIG = { cobrancaAtiva: true, planoPadrao: null, precos: PRECOS_MOCK }
let abrir: ReturnType<typeof vi.spyOn>

async function preparar() {
  const cliente = criarMockConta({ cobrancaAtiva: true })
  await cliente.auth.signInWithOAuth({ provider: 'google', options: { redirectTo: 'http://localhost', skipBrowserRedirect: true } })
  vi.mocked(clienteAtual).mockReturnValue(cliente)
  return cliente
}

function botaoDo(plano: string): HTMLElement {
  return document.querySelector(`[data-plano="${plano}"] button`) as HTMLElement
}

beforeEach(() => {
  resetPlano()
  abrir = vi.spyOn(api, 'abrirUrl').mockResolvedValue(undefined)
})
afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

describe('PlanosModal', () => {
  it('fechado ou sem cobrança não renderiza', () => {
    const { container } = render(<PlanosModal />)
    expect(container.firstChild).toBeNull()
    act(() => setPlanoEstado({ config: { cobrancaAtiva: false, planoPadrao: null, precos: null }, telaAberta: true }))
    expect(container.firstChild).toBeNull()
  })

  it('mostra três cartões em ordem, anual por padrão', () => {
    setPlanoEstado({ config: CONFIG, telaAberta: true })
    render(<PlanosModal />)
    expect(screen.getByRole('dialog', { name: 'Planos do Online' })).toBeTruthy()
    const ids = Array.from(document.querySelectorAll('[data-plano]')).map((e) => e.getAttribute('data-plano'))
    expect(ids).toEqual(['espectador', 'hd', 'fullhd'])
    expect(screen.getByText('2 meses de graça')).toBeTruthy()
    expect(document.querySelector('[data-plano="hd"]')?.textContent).toContain('R$ 79/ano')
  })

  it('ciclo mensal mostra preço por mês', () => {
    setPlanoEstado({ config: CONFIG, telaAberta: true })
    render(<PlanosModal />)
    fireEvent.click(screen.getByRole('radio', { name: 'Mensal no cartão' }))
    expect(document.querySelector('[data-plano="hd"]')?.textContent).toContain('R$ 7,90/mês')
  })

  it('destaque ganha borda verde', () => {
    setPlanoEstado({ config: CONFIG, telaAberta: true, destaque: 'fullhd' })
    render(<PlanosModal />)
    const el = document.querySelector('[data-plano="fullhd"]') as HTMLElement
    expect(el.getAttribute('data-destaque')).toBe('true')
    expect(el.style.border).toContain('var(--verde)')
  })

  it('assinar abre o link e atualiza depois de pagar', async () => {
    await preparar()
    setPlanoEstado({ config: CONFIG, telaAberta: true })
    render(<PlanosModal />)
    await act(async () => {
      fireEvent.click(botaoDo('hd'))
    })
    expect(abrir).toHaveBeenCalledWith('https://sandbox.asaas.com/i/mock-hd-anual')
    const atualizar = await screen.findByText('Já assinei, atualizar')
    await act(async () => {
      fireEvent.click(atualizar)
    })
    expect(await screen.findByText('Plano atualizado: Transmissor HD.')).toBeTruthy()
  })

  it('com assinatura ativa mostra plano atual e troca', () => {
    const validoAte = new Date(Date.now() + 86_400_000).toISOString()
    setPlanoEstado({ config: CONFIG, telaAberta: true, assinatura: { plano: 'hd', status: 'ativa', ciclo: 'anual', validoAte } })
    render(<PlanosModal />)
    expect((screen.getByText('Plano atual') as HTMLButtonElement).disabled).toBe(true)
    expect(screen.getAllByText('Trocar para este')).toHaveLength(2)
    expect(screen.getByText('Trocar de plano cancela a assinatura atual; a nova começa sem desconto proporcional.')).toBeTruthy()
  })

  it('Esc fecha', () => {
    setPlanoEstado({ config: CONFIG, telaAberta: true })
    render(<PlanosModal />)
    act(() => {
      fireEvent.keyDown(window, { key: 'Escape' })
    })
    expect(getPlanoEstado().telaAberta).toBe(false)
  })

  it('documento inválido mostra alerta', async () => {
    await preparar()
    setPlanoEstado({ config: CONFIG, telaAberta: true })
    render(<PlanosModal />)
    fireEvent.change(screen.getByLabelText('CPF ou CNPJ (opcional)'), { target: { value: '123456789012' } })
    await act(async () => {
      fireEvent.click(botaoDo('hd'))
    })
    expect(screen.getByRole('alert').textContent).toBe('Informe um CPF ou CNPJ válido.')
  })
})
