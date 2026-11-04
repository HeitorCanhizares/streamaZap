// Tarefa criar-03
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { CriarNomeAcesso, type CriarNomeAcessoProps } from './CriarNomeAcesso'

afterEach(() => cleanup())

function props(over: Partial<CriarNomeAcessoProps> = {}): CriarNomeAcessoProps {
  return { nome: 'Sala do Zé', onNome: vi.fn(), acesso: 'aberta', onAcesso: vi.fn(), senha: '', onSenha: vi.fn(), ...over }
}

describe('CriarNomeAcesso', () => {
  it('titulos: mostra "02", "Nome da sala", "03", "Quem pode entrar" e a legenda', () => {
    render(<CriarNomeAcesso {...props()} />)
    expect(screen.getByText('02')).toBeTruthy()
    expect(screen.getByText('Nome da sala')).toBeTruthy()
    expect(screen.getByText('03')).toBeTruthy()
    expect(screen.getByText('Quem pode entrar')).toBeTruthy()
    expect(screen.getByText('É o nome que a galera vê na lista.')).toBeTruthy()
  })

  it('input do nome mostra o valor e digitar chama onNome', () => {
    const onNome = vi.fn()
    render(<CriarNomeAcesso {...props({ nome: 'Sala', onNome })} />)
    const input = screen.getByLabelText(/Nome da sala/) as HTMLInputElement
    expect(input.value).toBe('Sala')
    fireEvent.change(input, { target: { value: 'Nova' } })
    expect(onNome).toHaveBeenLastCalledWith('Nova')
  })

  it('botoes Aberta e Com senha refletem aria-pressed e clique chama onAcesso', () => {
    const onAcesso = vi.fn()
    const { rerender } = render(<CriarNomeAcesso {...props({ acesso: 'aberta', onAcesso })} />)
    expect(screen.getByRole('button', { name: /Aberta/ }).getAttribute('aria-pressed')).toBe('true')
    expect(screen.getByRole('button', { name: /Com senha/ }).getAttribute('aria-pressed')).toBe('false')
    fireEvent.click(screen.getByRole('button', { name: /Com senha/ }))
    expect(onAcesso).toHaveBeenLastCalledWith('senha')
    rerender(<CriarNomeAcesso {...props({ acesso: 'senha', onAcesso })} />)
    expect(screen.getByRole('button', { name: /Aberta/ }).getAttribute('aria-pressed')).toBe('false')
    expect(screen.getByRole('button', { name: /Com senha/ }).getAttribute('aria-pressed')).toBe('true')
    fireEvent.click(screen.getByRole('button', { name: /Aberta/ }))
    expect(onAcesso).toHaveBeenLastCalledWith('aberta')
  })

  it('campo de senha so aparece com acesso "senha" e digitar chama onSenha', () => {
    const onSenha = vi.fn()
    const { rerender } = render(<CriarNomeAcesso {...props({ acesso: 'aberta', onSenha })} />)
    expect(screen.queryByPlaceholderText('Crie uma senha')).toBeNull()
    rerender(<CriarNomeAcesso {...props({ acesso: 'senha', senha: '', onSenha })} />)
    const campo = screen.getByPlaceholderText('Crie uma senha')
    expect(campo.getAttribute('type')).toBe('password')
    expect(screen.getByLabelText('Senha da sala')).toBeTruthy()
    fireEvent.change(campo, { target: { value: 'abc' } })
    expect(onSenha).toHaveBeenLastCalledWith('abc')
  })

  it('hints: mostra "Qualquer um na rede entra" e "Só quem souber a senha"', () => {
    render(<CriarNomeAcesso {...props()} />)
    expect(screen.getByText('Qualquer um na rede entra')).toBeTruthy()
    expect(screen.getByText('Só quem souber a senha')).toBeTruthy()
  })
})
