// Convite pelo link (`streamazap://sala/<alvo>`): a tela entra direto ou abre o painel preenchido.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import { api } from '../../bridge/api'
import { resetOnlineEnabled, setOnlineEnabled } from '../../bridge/onlineStore'
import { resetSnapshotForTests } from '../../bridge/store'
import { SalasScreen } from './SalasScreen'

vi.mock('./AtualizadoCard', () => ({ AtualizadoCard: () => <div data-testid="atualizado-card" /> }))

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
  resetOnlineEnabled()
  resetSnapshotForTests()
})

function montar(alvo: string, seq = 1) {
  const onEntered = vi.fn()
  const onConviteTratado = vi.fn()
  const utils = render(
    <SalasScreen onCreate={() => {}} onEntered={onEntered} convite={{ alvo, seq }} onConviteTratado={onConviteTratado} />,
  )
  return { ...utils, onEntered, onConviteTratado }
}

describe('SalasScreen com convite', () => {
  it('código com o Online ligado: entra pelo código e avisa que tratou', async () => {
    setOnlineEnabled(true)
    const join = vi.spyOn(api, 'roomJoin').mockResolvedValue()
    const { onEntered, onConviteTratado } = montar('ZAP-7K2Q')
    await waitFor(() => expect(onEntered).toHaveBeenCalledTimes(1))
    expect(join).toHaveBeenCalledTimes(1)
    expect(join).toHaveBeenCalledWith({ code: 'ZAP-7K2Q' })
    expect(onConviteTratado).toHaveBeenCalled()
  })

  it('a sala pede senha: o painel fica aberto com o código preenchido e o erro', async () => {
    setOnlineEnabled(true)
    vi.spyOn(api, 'roomJoin').mockRejectedValue('Essa sala pede senha.')
    const { onEntered } = montar('ZAP-7K2Q')
    await waitFor(() => expect(screen.getByText('Essa sala pede senha.')).toBeTruthy())
    expect((screen.getByLabelText('Código da sala') as HTMLInputElement).value).toBe('ZAP-7K2Q')
    expect(onEntered).not.toHaveBeenCalled()
  })

  it('código com o Online desligado: não entra e explica', async () => {
    setOnlineEnabled(false)
    const join = vi.spyOn(api, 'roomJoin').mockResolvedValue()
    montar('ZAP-7K2Q')
    await waitFor(() => expect(screen.getByText(/ative o modo Online em Configurações para entrar/)).toBeTruthy())
    expect(join).not.toHaveBeenCalled()
  })

  it('IP da sala Local: entra por ip e porta', async () => {
    setOnlineEnabled(true)
    const join = vi.spyOn(api, 'roomJoin').mockResolvedValue()
    const { onEntered } = montar('26.14.7.88:47800')
    await waitFor(() => expect(onEntered).toHaveBeenCalledTimes(1))
    expect(join).toHaveBeenCalledWith({ ip: '26.14.7.88', port: 47800 })
  })

  it('alvo inválido: não chama a ponte', async () => {
    setOnlineEnabled(true)
    const join = vi.spyOn(api, 'roomJoin').mockResolvedValue()
    const { onConviteTratado } = montar('exemplo.com')
    await waitFor(() => expect(onConviteTratado).toHaveBeenCalled())
    expect(join).not.toHaveBeenCalled()
  })

  it('o mesmo convite (mesmo seq) não entra duas vezes ao re-renderizar', async () => {
    setOnlineEnabled(true)
    const join = vi.spyOn(api, 'roomJoin').mockResolvedValue()
    const { rerender, onEntered } = montar('ZAP-7K2Q')
    await waitFor(() => expect(onEntered).toHaveBeenCalledTimes(1))
    rerender(<SalasScreen onCreate={() => {}} onEntered={onEntered} convite={{ alvo: 'ZAP-7K2Q', seq: 1 }} />)
    await new Promise((r) => setTimeout(r, 0))
    expect(join).toHaveBeenCalledTimes(1)
  })
})
