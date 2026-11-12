import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { ConfirmarConvite } from './ConfirmarConvite'

afterEach(() => cleanup())

function montar(over: { ocupado?: boolean; erro?: string | null } = {}) {
  const onConfirmar = vi.fn()
  const onCancelar = vi.fn()
  render(
    <ConfirmarConvite destino="ZAP-7K2Q" salaAtual="Sexta do Rush" ocupado={over.ocupado ?? false} erro={over.erro ?? null} onConfirmar={onConfirmar} onCancelar={onCancelar} />,
  )
  return { onConfirmar, onCancelar }
}

describe('ConfirmarConvite', () => {
  it('diz a sala atual e o destino', () => {
    montar()
    const dialogo = screen.getByRole('dialog', { name: 'Entrar na sala do convite?' })
    expect(dialogo.textContent).toContain('Sexta do Rush')
    expect(dialogo.textContent).toContain('ZAP-7K2Q')
  })

  it('Sair e entrar confirma; Agora não e Esc cancelam', () => {
    const { onConfirmar, onCancelar } = montar()
    fireEvent.click(screen.getByText('Sair e entrar'))
    expect(onConfirmar).toHaveBeenCalledTimes(1)
    fireEvent.click(screen.getByText('Agora não'))
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(onCancelar).toHaveBeenCalledTimes(2)
  })

  it('ocupado desliga os botões; erro aparece como alerta', () => {
    montar({ ocupado: true, erro: 'Não foi possível sair da sala.' })
    expect((screen.getByText('Sair e entrar') as HTMLButtonElement).disabled).toBe(true)
    expect(screen.getByRole('alert').textContent).toBe('Não foi possível sair da sala.')
  })
})
