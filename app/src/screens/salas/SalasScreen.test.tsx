// Tarefa salas-14
import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { api } from '../../bridge/api'
import { getSnapshot, resetSnapshotForTests, setSnapshot } from '../../bridge/store'
import { SalasScreen } from './SalasScreen'

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
  resetSnapshotForTests()
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
    expect(texto).toContain('Nova sala')
  })

  it('selecao: clicar em Filminho de domingo mostra o detalhe', () => {
    renderTela()
    fireEvent.click(screen.getByText('Filminho de domingo'))
    expect(screen.getByRole('heading', { level: 2, name: 'Filminho de domingo' })).toBeTruthy()
  })

  it('criar: Nova sala chama onCreate', () => {
    const { onCreate } = renderTela()
    fireEvent.click(screen.getByText('Nova sala'))
    expect(onCreate).toHaveBeenCalledTimes(1)
  })

  it('entrar: sala aberta chama roomJoin e onEntered', async () => {
    const spy = vi.spyOn(api, 'roomJoin').mockResolvedValue()
    const { onEntered } = renderTela()
    fireEvent.click(screen.getByText('Entrar na sala'))
    await waitFor(() => expect(onEntered).toHaveBeenCalledTimes(1))
    expect(spy).toHaveBeenCalledWith({ room_id: 'zap-7k2q' })
  })

  it('senha: sem digitar mostra erro; com 1234 chama a ponte', async () => {
    const spy = vi.spyOn(api, 'roomJoin').mockResolvedValue()
    const { onEntered, container } = renderTela()
    fireEvent.click(screen.getByText('Filminho de domingo'))
    fireEvent.click(screen.getByText('Entrar na sala'))
    expect(spy).not.toHaveBeenCalled()
    expect(screen.getByText('Digite a senha da sala')).toBeTruthy()
    const campo = container.querySelector('input[type="password"]') as HTMLInputElement
    fireEvent.change(campo, { target: { value: '1234' } })
    fireEvent.click(screen.getByText('Entrar na sala'))
    await waitFor(() => expect(onEntered).toHaveBeenCalledTimes(1))
    expect(spy).toHaveBeenCalledWith({ room_id: 'zap-3m9x' }, '1234')
  })

  it('erro: falha da ponte mostra alert e não chama onEntered', async () => {
    vi.spyOn(api, 'roomJoin').mockRejectedValue(new Error('sala cheia'))
    const { onEntered } = renderTela()
    fireEvent.click(screen.getByText('Entrar na sala'))
    const alerta = await screen.findByRole('alert')
    expect(alerta.textContent).toContain('sala cheia')
    expect(onEntered).not.toHaveBeenCalled()
  })

  it('modo: aba Local mostra Live de Python e esconde Sexta do Rush', () => {
    renderTela()
    fireEvent.click(screen.getByRole('tab', { name: /Local/ }))
    expect(screen.getAllByText('Live de Python').length).toBeGreaterThan(0)
    expect(screen.queryByText('Sexta do Rush')).toBeNull()
  })

  it('manual: IP sem porta usa 47800', async () => {
    const spy = vi.spyOn(api, 'roomJoin').mockResolvedValue()
    const { onEntered, container } = renderTela()
    fireEvent.click(screen.getByRole('tab', { name: /Local/ }))
    fireEvent.click(screen.getByText('Entrar por IP'))
    const campo = container.querySelector('input[placeholder^="IP:porta"]') as HTMLInputElement
    fireEvent.change(campo, { target: { value: '192.168.0.10' } })
    fireEvent.click(screen.getByRole('button', { name: 'Entrar' }))
    await waitFor(() => expect(onEntered).toHaveBeenCalledTimes(1))
    expect(spy).toHaveBeenCalledWith({ ip: '192.168.0.10', port: 47800 })
  })

  it('manual: IP inválido mostra erro', () => {
    const { container } = renderTela()
    fireEvent.click(screen.getByRole('tab', { name: /Local/ }))
    fireEvent.click(screen.getByText('Entrar por IP'))
    const campo = container.querySelector('input[placeholder^="IP:porta"]') as HTMLInputElement
    fireEvent.change(campo, { target: { value: ':99' } })
    fireEvent.click(screen.getByRole('button', { name: 'Entrar' }))
    expect(screen.getByRole('alert').textContent).toContain('Informe um IP válido')
  })

  it('busca: filmin filtra a lista', () => {
    renderTela()
    fireEvent.change(screen.getByLabelText('Buscar sala'), { target: { value: 'filmin' } })
    const lista = within(screen.getByRole('region', { name: 'Salas na rede' }))
    expect(lista.getByText('Filminho de domingo')).toBeTruthy()
    expect(lista.queryByText('Sexta do Rush')).toBeNull()
  })

  // Tarefa ui-online-07: Entrar com código no modo Online.
  it('online: Entrar com código abre o campo de código, não o de IP', () => {
    const { container } = renderTela()
    fireEvent.click(screen.getByText('Entrar com código'))
    expect(screen.getByLabelText('Código da sala')).toBeTruthy()
    expect(container.querySelector('input[placeholder^="IP:porta"]')).toBeNull()
  })

  it('online: código válido chama roomJoin({code}) e onEntered', async () => {
    const spy = vi.spyOn(api, 'roomJoin').mockResolvedValue()
    const { onEntered } = renderTela()
    fireEvent.click(screen.getByText('Entrar com código'))
    fireEvent.change(screen.getByLabelText('Código da sala'), { target: { value: 'zap-7k2q' } })
    fireEvent.click(screen.getByRole('button', { name: 'Entrar' }))
    await waitFor(() => expect(onEntered).toHaveBeenCalledTimes(1))
    expect(spy).toHaveBeenCalledWith({ code: 'ZAP-7K2Q' })
  })

  it('online: código inválido não chama roomJoin', () => {
    const spy = vi.spyOn(api, 'roomJoin').mockResolvedValue()
    renderTela()
    fireEvent.click(screen.getByText('Entrar com código'))
    fireEvent.change(screen.getByLabelText('Código da sala'), { target: { value: 'xyz' } })
    fireEvent.click(screen.getByRole('button', { name: 'Entrar' }))
    expect(spy).not.toHaveBeenCalled()
    expect(screen.getByRole('alert').textContent).toContain('Código inválido')
  })

  it('online: com senha chama roomJoin({code}, senha)', async () => {
    const spy = vi.spyOn(api, 'roomJoin').mockResolvedValue()
    const { onEntered } = renderTela()
    fireEvent.click(screen.getByText('Entrar com código'))
    fireEvent.change(screen.getByLabelText('Código da sala'), { target: { value: 'ZAP-7K2Q' } })
    fireEvent.change(screen.getByLabelText('Senha da sala (se tiver)'), { target: { value: 'abc' } })
    fireEvent.click(screen.getByRole('button', { name: 'Entrar' }))
    await waitFor(() => expect(onEntered).toHaveBeenCalledTimes(1))
    expect(spy).toHaveBeenCalledWith({ code: 'ZAP-7K2Q' }, 'abc')
  })

  it('online: falha do roomJoin aparece no painel de código', async () => {
    vi.spyOn(api, 'roomJoin').mockRejectedValue(new Error('senha errada'))
    const { onEntered } = renderTela()
    fireEvent.click(screen.getByText('Entrar com código'))
    fireEvent.change(screen.getByLabelText('Código da sala'), { target: { value: 'ZAP-7K2Q' } })
    fireEvent.click(screen.getByRole('button', { name: 'Entrar' }))
    const alerta = await screen.findByRole('alert')
    expect(alerta.textContent).toContain('senha errada')
    expect(onEntered).not.toHaveBeenCalled()
  })

  it('firewall: erro do snapshot mostra o aviso', () => {
    const base = getSnapshot()
    act(() => setSnapshot({ ...base, version: base.version + 1, errors: ['Firewall do Windows bloqueando'] }))
    renderTela()
    expect(screen.getByText('Corrigir firewall')).toBeTruthy()
  })
})
