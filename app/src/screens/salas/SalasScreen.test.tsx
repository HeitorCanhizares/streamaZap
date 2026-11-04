// Tarefa ui-14
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { api } from '../../bridge/api'
import { SalasScreen } from './SalasScreen'

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

function renderTela() {
  const onCreate = vi.fn()
  const onEntered = vi.fn()
  const utils = render(<SalasScreen onCreate={onCreate} onEntered={onEntered} />)
  return { ...utils, onCreate, onEntered }
}

describe('SalasScreen', () => {
  it('renderiza_com_mock', () => {
    renderTela()
    const texto = document.body.textContent ?? ''
    expect(texto).toContain('Sexta do Rush')
    expect(texto).toContain('Criar sala')
  })

  it('selecao: clicar em Filminho de domingo mostra o detalhe dessa sala', () => {
    renderTela()
    fireEvent.click(screen.getByText('Filminho de domingo'))
    expect(screen.getByRole('heading', { level: 2, name: 'Filminho de domingo' })).toBeTruthy()
  })

  it('criar: clicar em Criar sala chama onCreate', () => {
    const { onCreate } = renderTela()
    fireEvent.click(screen.getByText('Criar sala'))
    expect(onCreate).toHaveBeenCalledTimes(1)
  })

  it('entrar: clicar Entrar na sala numa sala aberta chama onEntered', async () => {
    const spy = vi.spyOn(api, 'roomJoin').mockResolvedValue()
    const { onEntered } = renderTela()
    fireEvent.click(screen.getByText('Entrar na sala'))
    await waitFor(() => expect(onEntered).toHaveBeenCalledTimes(1))
    expect(spy).toHaveBeenCalledWith({ room_id: 'zap-7k2q' })
  })

  it('senha: sala com senha pede a senha antes de chamar a ponte', async () => {
    const spy = vi.spyOn(api, 'roomJoin').mockResolvedValue()
    const { onEntered, container } = renderTela()
    fireEvent.click(screen.getByText('Filminho de domingo'))
    fireEvent.click(screen.getByText('Entrar na sala'))
    expect(spy).not.toHaveBeenCalled()
    const campo = container.querySelector('input[type="password"]') as HTMLInputElement
    expect(campo).toBeTruthy()
    fireEvent.change(campo, { target: { value: '1234' } })
    fireEvent.click(screen.getByText('Entrar'))
    await waitFor(() => expect(onEntered).toHaveBeenCalledTimes(1))
    expect(spy).toHaveBeenCalledWith({ room_id: 'zap-3m9x' }, '1234')
  })

  it('erro: falha da ponte mostra a mensagem em vermelho e não chama onEntered', async () => {
    vi.spyOn(api, 'roomJoin').mockRejectedValue(new Error('sala cheia'))
    const { onEntered } = renderTela()
    fireEvent.click(screen.getByText('Entrar na sala'))
    const alerta = await screen.findByRole('alert')
    expect(alerta.textContent).toContain('sala cheia')
    expect(onEntered).not.toHaveBeenCalled()
  })

  it('manual: IP sem porta usa a porta padrão 47800', async () => {
    const spy = vi.spyOn(api, 'roomJoin').mockResolvedValue()
    const { onEntered, container } = renderTela()
    fireEvent.click(screen.getByText('Entrar com código/IP'))
    const campo = container.querySelector('input[placeholder^="IP:porta"]') as HTMLInputElement
    fireEvent.change(campo, { target: { value: '192.168.0.10' } })
    fireEvent.click(screen.getByText('Entrar'))
    await waitFor(() => expect(onEntered).toHaveBeenCalledTimes(1))
    expect(spy).toHaveBeenCalledWith({ ip: '192.168.0.10', port: 47800 })
  })
})
