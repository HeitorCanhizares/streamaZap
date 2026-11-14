import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { api } from '../../../bridge/api'
import { mockMusic, mockSnapshot } from '../../../bridge/mock'
import type { RoomView } from '../../../bridge/types'
import { PainelMusica, textoDoErro } from './PainelMusica'

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

function sala(over: Partial<RoomView> = {}): RoomView {
  const r = mockSnapshot().room
  if (!r) throw new Error('mock sem sala')
  return { ...r, mode: 'local', music: mockMusic(), ...over }
}

describe('PainelMusica', () => {
  it('sem música: explica por quê (Online ou dono com app antigo)', () => {
    const { rerender } = render(<PainelMusica room={sala({ music: null, mode: 'online' })} meId="p-rafa" />)
    expect(screen.getByText(/servidor desta sala ainda não tem a música/)).toBeTruthy()
    rerender(<PainelMusica room={sala({ music: null, mode: 'local' })} meId="p-rafa" />)
    expect(screen.getByText(/precisa atualizar o streamaZap/)).toBeTruthy()
  })

  it('mostra o que toca, os votos e a fila', () => {
    render(<PainelMusica room={sala()} meId="p-dudu" />)
    expect(screen.getByText('Me at the zoo')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Votar para pular · 1/2' }).getAttribute('aria-pressed')).toBe('false')
    expect(screen.getByText('lofi hip hop radio')).toBeTruthy()
    expect(screen.getByText(/AO VIVO · pôs: Marcão/)).toBeTruthy()
    expect(screen.getByText(/3:33 · pôs: Você/)).toBeTruthy()
    expect(screen.getByText('PRÓXIMAS · 2')).toBeTruthy()
  })

  it('a música pausada aparece como pausada', () => {
    const m = mockMusic()
    if (!m.current) throw new Error('mock sem música')
    render(<PainelMusica room={sala({ music: { ...m, current: { ...m.current, paused: true } } })} meId="p-dudu" />)
    expect(screen.getByText(/^pausada · jawed · pôs: Ju$/)).toBeTruthy()
  })

  it('votar chama musicVote e quem já votou desfaz', () => {
    const vote = vi.spyOn(api, 'musicVote').mockResolvedValue(undefined)
    const { rerender } = render(<PainelMusica room={sala()} meId="p-dudu" />)
    fireEvent.click(screen.getByRole('button', { name: /Votar para pular/ }))
    expect(vote).toHaveBeenLastCalledWith('mu_1', true)
    rerender(<PainelMusica room={sala()} meId="p-rafa" />)
    const votei = screen.getByRole('button', { name: 'Votei para pular · 1/2' })
    expect(votei.getAttribute('aria-pressed')).toBe('true')
    fireEvent.click(votei)
    expect(vote).toHaveBeenLastCalledWith('mu_1', false)
  })

  it('tirar e pular agora: só quem pôs e o dono', () => {
    const remove = vi.spyOn(api, 'musicRemove').mockResolvedValue(undefined)
    // Alguém que não pôs nada e não é o dono: nada para tirar.
    const { rerender } = render(<PainelMusica room={sala()} meId="p-rafa" />)
    expect(screen.queryByRole('button', { name: 'Pular agora' })).toBeNull()
    expect(screen.queryByRole('button', { name: /Tirar .* da fila/ })).toBeNull()
    // Quem pôs a atual (Ju) pula agora.
    rerender(<PainelMusica room={sala()} meId="p-ju" />)
    fireEvent.click(screen.getByRole('button', { name: 'Pular agora' }))
    expect(remove).toHaveBeenLastCalledWith('mu_1')
    // O dono (Marcão) tira qualquer uma.
    rerender(<PainelMusica room={sala()} meId="p-marcao" />)
    expect(screen.getAllByRole('button', { name: /Tirar .* da fila/ })).toHaveLength(2)
    fireEvent.click(screen.getByRole('button', { name: 'Tirar Never Gonna Give You Up da fila' }))
    expect(remove).toHaveBeenLastCalledWith('mu_3')
  })

  it('pôr na fila: manda o link, limpa o campo e mostra o erro sem o prefixo', async () => {
    const add = vi.spyOn(api, 'musicAdd').mockResolvedValueOnce(undefined)
    render(<PainelMusica room={sala()} meId="p-rafa" />)
    const campo = screen.getByLabelText('Link do YouTube') as HTMLInputElement
    fireEvent.change(campo, { target: { value: ' https://youtu.be/dQw4w9WgXcQ ' } })
    fireEvent.click(screen.getByRole('button', { name: 'Pôr na fila' }))
    await waitFor(() => expect(add).toHaveBeenCalledWith('https://youtu.be/dQw4w9WgXcQ'))
    await waitFor(() => expect(campo.value).toBe(''))

    add.mockRejectedValueOnce(new Error('Pedido inválido: esse vídeo não pode tocar fora do YouTube (ou é privado)'))
    fireEvent.change(campo, { target: { value: 'https://youtu.be/aaaaaaaaaaa' } })
    fireEvent.click(screen.getByRole('button', { name: 'Pôr na fila' }))
    expect((await screen.findByRole('alert')).textContent).toBe('Esse vídeo não pode tocar fora do YouTube (ou é privado)')
    expect(campo.value).toBe('https://youtu.be/aaaaaaaaaaa')
  })

  it('nada tocando e fila vazia', () => {
    render(<PainelMusica room={sala({ music: { queue: [], current: null } })} meId="p-rafa" />)
    expect(screen.getByText(/Nada tocando/)).toBeTruthy()
    expect(screen.getByText('A fila está vazia.')).toBeTruthy()
  })

  it('textoDoErro tira o prefixo do núcleo', () => {
    expect(textoDoErro(new Error('Não suportado: a música da sala ainda não funciona no modo Online'))).toBe('A música da sala ainda não funciona no modo Online')
    expect(textoDoErro('falhou')).toBe('Falhou')
  })
})
