// Tarefa salas-09
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { SalaAcesso } from './SalaAcesso'
import type { SalaVM } from './model'

afterEach(() => cleanup())

const base: SalaVM = {
  id: 'r1', nome: 'Sexta do Rush', host: 'Ju', modo: 'online', addr: 'ZAP-7K2Q', trancada: false, versaoAviso: '',
  cena: 'game', camCena: null, hora: '21:58', ultima: '', naoLidas: 0, digitando: false, ping: 38,
  qualidade: '1080p60', bitrate: '', assistindo: 7, compartilhando: [], pessoas: [],
}

function monta(sala: Partial<SalaVM> = {}, p: Partial<Parameters<typeof SalaAcesso>[0]> = {}) {
  const props = { sala: { ...base, ...sala }, senha: '', onSenha: vi.fn(), ocupado: false, erro: null, onEntrar: vi.fn(), ...p }
  render(<SalaAcesso {...props} />)
  return props
}

describe('SalaAcesso', () => {
  it('aberta online', () => {
    monta()
    expect(screen.getByText('Qualquer pessoa com o app entra pelo servidor, de qualquer lugar.')).toBeTruthy()
    expect(screen.getByRole('button', { name: /Entrar na sala/ })).toBeTruthy()
    expect(screen.queryByPlaceholderText('Digite a senha')).toBeNull()
  })

  it('aberta local', () => {
    monta({ modo: 'local' })
    expect(screen.getByText('Qualquer pessoa na mesma rede do Radmin entra direto, sem senha.')).toBeTruthy()
  })

  it('trancada mostra campo e dica', () => {
    monta({ trancada: true })
    expect(screen.getByPlaceholderText('Digite a senha')).toBeTruthy()
    expect(screen.getByText('Senha da sala')).toBeTruthy()
    expect(screen.getByText('Não sabe? Pede pro Ju no zap.')).toBeTruthy()
  })

  it('digitar chama onSenha e Enter chama onEntrar', () => {
    const p = monta({ trancada: true })
    const campo = screen.getByPlaceholderText('Digite a senha')
    fireEvent.change(campo, { target: { value: 'abc' } })
    expect(p.onSenha).toHaveBeenCalledWith('abc')
    fireEvent.keyDown(campo, { key: 'Enter' })
    expect(p.onEntrar).toHaveBeenCalledTimes(1)
  })

  it('versaoAviso desabilita', () => {
    monta({ versaoAviso: '0.9.1' })
    const b = screen.getByRole('button', { name: 'Atualize para entrar' }) as HTMLButtonElement
    expect(b.disabled).toBe(true)
    expect(screen.queryByText(/Entrar na sala/)).toBeNull()
    expect(screen.getByText(/versão 0\.9\.1/)).toBeTruthy()
  })

  it('erro em alert', () => {
    monta({}, { erro: 'Senha errada' })
    expect(screen.getByRole('alert').textContent).toBe('Senha errada')
  })

  it('ocupado desabilita', () => {
    monta({}, { ocupado: true })
    expect((screen.getByRole('button', { name: /Entrar na sala/ }) as HTMLButtonElement).disabled).toBe(true)
  })

  it('mostra ping e suavização', () => {
    monta()
    expect(screen.getByText('Ping')).toBeTruthy()
    expect(screen.getByText('38 ms')).toBeTruthy()
    expect(screen.getByText('Suavização')).toBeTruthy()
    expect(screen.getByText('auto')).toBeTruthy()
  })

  it('clique em Entrar chama onEntrar', () => {
    const p = monta()
    fireEvent.click(screen.getByRole('button', { name: /Entrar na sala/ }))
    expect(p.onEntrar).toHaveBeenCalledTimes(1)
  })
})
