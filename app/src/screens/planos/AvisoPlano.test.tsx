// Tarefa telas-04
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mockSnapshot } from '../../bridge/mock'
import { PRECOS_MOCK } from '../../bridge/mockConta'
import { resetPlano, setPlanoEstado } from '../../bridge/planoStore'
import { resetSnapshotForTests, setSnapshot } from '../../bridge/store'
import type { PlanNotice } from '../../bridge/types'
import { abrirPlanos } from '../../bridge/planoAcoes'
import { AvisoPlano } from './AvisoPlano'

vi.mock('../../bridge/planoAcoes', () => ({ abrirPlanos: vi.fn() }))

let versao = 1
function poe(plan_notice?: PlanNotice): void {
  act(() => {
    setSnapshot({ ...mockSnapshot(), version: ++versao, plan_notice })
  })
}
const aviso = (extra: Partial<PlanNotice> = {}): PlanNotice => ({
  seq: 1,
  plan: 'espectador',
  action: 'transmitir',
  message: 'Seu plano não permite isso.',
  ...extra,
})
function cobranca(ativa: boolean): void {
  act(() => {
    setPlanoEstado({ config: { cobrancaAtiva: ativa, planoPadrao: null, precos: PRECOS_MOCK } })
  })
}

beforeEach(() => {
  vi.clearAllMocks()
  resetSnapshotForTests()
  resetPlano()
})
afterEach(() => {
  cleanup()
  resetSnapshotForTests()
  resetPlano()
})

describe('AvisoPlano', () => {
  it('mostra a mensagem do plan_notice', () => {
    poe(aviso())
    render(<AvisoPlano />)
    expect(screen.getByRole('alert').textContent).toContain('Seu plano não permite isso.')
  })

  it('Fechar esconde e um seq maior volta a mostrar', () => {
    poe(aviso())
    render(<AvisoPlano />)
    fireEvent.click(screen.getByText('Fechar'))
    expect(screen.queryByRole('alert')).toBeNull()
    poe(aviso({ seq: 2, message: 'Outro aviso.' }))
    expect(screen.getByText('Outro aviso.')).toBeTruthy()
  })

  it('sem cobrança não há botão Ver planos', () => {
    poe(aviso())
    render(<AvisoPlano />)
    expect(screen.queryByText('Ver planos')).toBeNull()
  })

  it('com cobrança, Ver planos abre o destaque certo', () => {
    cobranca(true)
    poe(aviso({ action: 'transmitir', plan: 'hd' }))
    render(<AvisoPlano />)
    fireEvent.click(screen.getByText('Ver planos'))
    expect(abrirPlanos).toHaveBeenLastCalledWith('fullhd')

    poe(aviso({ seq: 2, action: 'entrar', plan: 'espectador' }))
    fireEvent.click(screen.getByText('Ver planos'))
    expect(abrirPlanos).toHaveBeenLastCalledWith('espectador')

    poe(aviso({ seq: 3, action: 'transmitir', plan: 'espectador' }))
    fireEvent.click(screen.getByText('Ver planos'))
    expect(abrirPlanos).toHaveBeenLastCalledWith('hd')
  })

  it('sem plan_notice não renderiza nada', () => {
    poe(undefined)
    const { container } = render(<AvisoPlano />)
    expect(container.innerHTML).toBe('')
  })
})
