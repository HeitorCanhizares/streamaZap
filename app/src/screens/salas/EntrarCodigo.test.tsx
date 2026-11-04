// Tarefa ui-online-05
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { EntrarCodigo } from './EntrarCodigo'

afterEach(() => cleanup())

function monta(p: Partial<Parameters<typeof EntrarCodigo>[0]> = {}) {
  const props = { onEntrar: vi.fn(), onCancelar: vi.fn(), ocupado: false, erro: null, ...p }
  render(<EntrarCodigo {...props} />)
  return props
}

const campoCodigo = () => screen.getByLabelText('Código da sala') as HTMLInputElement
const campoSenha = () => screen.getByLabelText('Senha da sala (se tiver)') as HTMLInputElement

describe('EntrarCodigo', () => {
  it('renderiza os campos e os botões', () => {
    monta()
    expect(campoCodigo()).toBeTruthy()
    expect(campoSenha().type).toBe('password')
    expect(screen.getByRole('button', { name: 'Entrar' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Cancelar' })).toBeTruthy()
  })

  it('código inválido mostra erro local e não chama onEntrar', () => {
    const p = monta()
    fireEvent.change(campoCodigo(), { target: { value: 'xyz' } })
    fireEvent.click(screen.getByRole('button', { name: 'Entrar' }))
    expect(screen.getByRole('alert').textContent).toContain('Código inválido')
    expect(p.onEntrar).not.toHaveBeenCalled()
  })

  it('código em minúsculas é normalizado e chama onEntrar sem senha', () => {
    const p = monta()
    fireEvent.change(campoCodigo(), { target: { value: 'zap-7k2q' } })
    fireEvent.click(screen.getByRole('button', { name: 'Entrar' }))
    expect(p.onEntrar).toHaveBeenCalledWith('ZAP-7K2Q', null)
  })

  it('com senha chama onEntrar com a senha', () => {
    const p = monta()
    fireEvent.change(campoCodigo(), { target: { value: 'zap-7k2q' } })
    fireEvent.change(campoSenha(), { target: { value: 'abc' } })
    fireEvent.click(screen.getByRole('button', { name: 'Entrar' }))
    expect(p.onEntrar).toHaveBeenCalledWith('ZAP-7K2Q', 'abc')
  })

  it('Enter no campo do código dispara entrar', () => {
    const p = monta()
    fireEvent.change(campoCodigo(), { target: { value: 'ZAP-7K2Q' } })
    fireEvent.keyDown(campoCodigo(), { key: 'Enter' })
    expect(p.onEntrar).toHaveBeenCalledWith('ZAP-7K2Q', null)
  })

  it('ocupado desativa o botão Entrar', () => {
    monta({ ocupado: true })
    const b = screen.getByRole('button', { name: 'Entrar' }) as HTMLButtonElement
    expect(b.disabled).toBe(true)
  })

  it('prop erro aparece em alert', () => {
    monta({ erro: 'Sala não encontrada' })
    expect(screen.getByRole('alert').textContent).toBe('Sala não encontrada')
  })

  it('Cancelar chama onCancelar', () => {
    const p = monta()
    fireEvent.click(screen.getByRole('button', { name: 'Cancelar' }))
    expect(p.onCancelar).toHaveBeenCalledTimes(1)
  })
})
