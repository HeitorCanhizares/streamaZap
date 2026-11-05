// Painel "Transmissão" da sala: começar, trocar e parar sem sair da sala.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'

import { api } from '../../bridge/api'
import { mockSources } from '../../bridge/mock'
import type { ShareView } from '../../bridge/types'
import { SalaTransmissao } from './SalaTransmissao'

afterEach(() => { cleanup(); vi.restoreAllMocks() })

function base() {
  vi.spyOn(api, 'sourcesList').mockResolvedValue(mockSources())
  const start = vi.spyOn(api, 'shareStart').mockResolvedValue(undefined)
  const update = vi.spyOn(api, 'shareUpdate').mockResolvedValue(undefined)
  const stop = vi.spyOn(api, 'shareStop').mockResolvedValue(undefined)
  return { start, update, stop }
}

const transmitindo: ShareView = {
  source: 'screen:1', audio: 'Todo o PC', preset: '1080p60', encoder: 'NVENC', layers: [0], viewers_per_layer: [1, 0, 0], encode_ms: 3, conversion: [],
  request: { source: 'screen:1', audio: 'all', preset: '1080p60', encoder: 'auto' },
}

describe('SalaTransmissao', () => {
  it('sem transmissão: só o som manda source none com o som do PC', async () => {
    const { start } = base()
    const onClose = vi.fn()
    render(<SalaTransmissao share={null} camOn={false} onToggleCamera={vi.fn()} onCameraTrocada={vi.fn()} onClose={onClose} />)
    fireEvent.click(screen.getByRole('tab', { name: 'Só som' }))
    expect(screen.getByText('Só o som vai para a sala')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Começar a transmitir' }))
    await waitFor(() => expect(start).toHaveBeenCalled())
    expect(start.mock.calls[0][0]).toMatchObject({ source: 'none', audio: 'all' })
    expect(onClose).toHaveBeenCalled()
  })

  it('transmitindo: abre com o pedido atual e Aplicar chama shareUpdate', async () => {
    const { update, start } = base()
    render(<SalaTransmissao share={transmitindo} camOn={false} onToggleCamera={vi.fn()} onCameraTrocada={vi.fn()} onClose={vi.fn()} />)
    expect(screen.getByRole('tab', { name: 'Telas' }).getAttribute('aria-selected')).toBe('true')
    expect(screen.getByRole('radio', { name: 'Todo o PC' }).getAttribute('aria-checked')).toBe('true')
    fireEvent.click(screen.getByRole('button', { name: 'Aplicar' }))
    await waitFor(() => expect(update).toHaveBeenCalled())
    expect(update.mock.calls[0][0]).toMatchObject({ source: 'screen:1', audio: 'all', preset: '1080p60' })
    expect(start).not.toHaveBeenCalled()
  })

  it('transmitindo: Parar transmissão chama shareStop', async () => {
    const { stop } = base()
    render(<SalaTransmissao share={transmitindo} camOn={false} onToggleCamera={vi.fn()} onCameraTrocada={vi.fn()} onClose={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: 'Parar transmissão' }))
    await waitFor(() => expect(stop).toHaveBeenCalled())
  })

  it('só o som sem som mostra o erro e não chama o núcleo', async () => {
    const { start } = base()
    render(<SalaTransmissao share={null} camOn={false} onToggleCamera={vi.fn()} onCameraTrocada={vi.fn()} onClose={vi.fn()} />)
    fireEvent.click(screen.getByRole('tab', { name: 'Só som' }))
    fireEvent.click(screen.getByRole('radio', { name: 'Sem som' }))
    fireEvent.click(screen.getByRole('button', { name: 'Começar a transmitir' }))
    expect((await screen.findByRole('alert')).textContent).toMatch(/escolha o som/)
    expect(start).not.toHaveBeenCalled()
  })

  it('o interruptor da câmera chama onToggleCamera', () => {
    base()
    const onToggleCamera = vi.fn()
    render(<SalaTransmissao share={null} camOn={false} onToggleCamera={onToggleCamera} onCameraTrocada={vi.fn()} onClose={vi.fn()} />)
    fireEvent.click(screen.getByRole('switch', { name: 'Minha câmera' }))
    expect(onToggleCamera).toHaveBeenCalled()
  })
})
