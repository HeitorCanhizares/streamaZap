import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('../../bridge/planoAcoes', () => ({ abrirPlanos: vi.fn() }))

import { PRECOS_MOCK } from '../../bridge/mockConta'
import { abrirPlanos } from '../../bridge/planoAcoes'
import { resetPlano, setPlanoEstado } from '../../bridge/planoStore'
import { ConviteEspectador } from './ConviteEspectador'
import { VireTransmissor } from './VireTransmissor'

function comCobranca(ativa: boolean): void {
  act(() => {
    setPlanoEstado({
      config: { cobrancaAtiva: ativa, planoPadrao: null, precos: ativa ? PRECOS_MOCK : null },
    })
  })
}

describe('convites de planos', () => {
  beforeEach(() => {
    cleanup()
    resetPlano()
    vi.mocked(abrirPlanos).mockClear()
  })

  it('Vire Transmissor com cobrança abre planos em hd', () => {
    comCobranca(true)
    render(<VireTransmissor />)
    expect(
      screen.getByText('Para transmitir no Online, assine o Transmissor HD ou o Transmissor Full HD.'),
    ).toBeTruthy()
    fireEvent.click(screen.getByText('Ver planos'))
    expect(abrirPlanos).toHaveBeenCalledWith('hd')
  })

  it('Vire Transmissor sem cobrança mostra só o texto', () => {
    comCobranca(false)
    const { container } = render(<VireTransmissor />)
    expect(screen.getByText('Seu plano não permite transmitir no Online')).toBeTruthy()
    expect(screen.queryByText('Ver planos')).toBeNull()
    expect(container.textContent).not.toContain('R$')
  })

  it('Convite com cobrança mostra preço e abre planos em espectador', () => {
    comCobranca(true)
    render(<ConviteEspectador />)
    expect(screen.getByText('Assine o Espectador (R$ 3,90/mês ou R$ 39/ano) para assistir')).toBeTruthy()
    fireEvent.click(screen.getByText('Ver planos'))
    expect(abrirPlanos).toHaveBeenCalledWith('espectador')
  })

  it('Convite sem cobrança mostra só o texto', () => {
    comCobranca(false)
    const { container } = render(<ConviteEspectador />)
    expect(screen.getByText('Sua conta não tem um plano para entrar em salas Online')).toBeTruthy()
    expect(screen.queryByText('Ver planos')).toBeNull()
    expect(container.textContent).not.toContain('R$')
  })
})
