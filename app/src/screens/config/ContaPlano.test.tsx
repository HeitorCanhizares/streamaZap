import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { ContaPlano } from './ContaPlano'
import { ContaLogada } from './ContaLogada'
import * as acoes from '../../bridge/planoAcoes'
import { resetPlano, setPlanoEstado } from '../../bridge/planoStore'
import { resetConta, setContaEstado } from '../../bridge/contaStore'
import type { ConfigPublica } from '../../bridge/planoTipos'

vi.mock('../../bridge/planoAcoes', () => ({ abrirPlanos: vi.fn(), atualizarDepoisDePagar: vi.fn() }))
vi.mock('../../bridge/contaAcoes', () => ({ sair: vi.fn() }))
vi.mock('../../bridge/contaVinculo', () => ({ vincular: vi.fn(), desvincular: vi.fn(), excluirConta: vi.fn() }))

const cobranca: ConfigPublica = { cobrancaAtiva: true, planoPadrao: null, precos: null }

beforeEach(() => {
  vi.useFakeTimers({ toFake: ['Date'] })
  vi.setSystemTime(new Date('2026-11-11T12:00:00Z'))
})

afterEach(() => {
  vi.useRealTimers()
  cleanup()
  resetPlano()
  resetConta()
  vi.clearAllMocks()
})

describe('ContaPlano', () => {
  it('sem cobrança mostra só o beta gratuito', () => {
    render(<ContaPlano plano={{ id: 'fullhd', pode_entrar: true }} />)
    expect(screen.getByText('Plano: beta gratuito (Full HD)')).toBeTruthy()
    expect(screen.queryByText(/R\$/)).toBeNull()
    expect(screen.queryByText('Assinar / trocar plano')).toBeNull()
  })

  it('com cobrança mostra plano, validade e abre os planos', () => {
    setPlanoEstado({
      config: cobranca,
      assinatura: { plano: 'hd', status: 'ativa', ciclo: 'anual', validoAte: '2026-12-10T15:00:00Z' },
    })
    render(<ContaPlano plano={{ id: 'hd', pode_entrar: true }} />)
    expect(screen.getByText('Plano: Transmissor HD')).toBeTruthy()
    expect(screen.getByText('Ativa · válida até 10/12/2026')).toBeTruthy()
    fireEvent.click(screen.getByText('Assinar / trocar plano'))
    expect(acoes.abrirPlanos).toHaveBeenCalled()
  })

  it('aguardando pagamento mostra o botão de atualizar', () => {
    setPlanoEstado({ config: cobranca, aguardandoPagamento: true, info: 'Aguardando', aviso: 'Falhou' })
    render(<ContaPlano plano={{ id: 'hd', pode_entrar: true }} />)
    fireEvent.click(screen.getByText('Já assinei, atualizar'))
    expect(acoes.atualizarDepoisDePagar).toHaveBeenCalled()
    expect(screen.getByRole('status').textContent).toBe('Aguardando')
    expect(screen.getByRole('alert').textContent).toBe('Falhou')
  })

  it('ContaLogada renderiza ContaPlano', () => {
    setContaEstado({ sessao: 'dentro', email: 'a@b.com', identidades: [] })
    render(<ContaLogada plano={{ id: 'hd', pode_entrar: true }} />)
    expect(screen.getByText('Plano: beta gratuito (HD)')).toBeTruthy()
  })

  it('sem plano e sem plano padrão não mostra linha', () => {
    render(<ContaPlano />)
    expect(screen.queryByText(/Plano:/)).toBeNull()
  })
})
