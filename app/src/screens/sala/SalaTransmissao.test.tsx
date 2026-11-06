// Painel "Transmissão" da sala: começar, trocar e parar sem sair da sala.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'

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

  describe('câmera: qualidade e jeito', () => {
    function cam() {
      base()
      const q = vi.spyOn(api, 'cameraQualitySet').mockResolvedValue(undefined)
      const l = vi.spyOn(api, 'cameraLayoutSet').mockResolvedValue(undefined)
      const set = vi.spyOn(api, 'settingsSet').mockResolvedValue(undefined as never)
      return { q, l, set }
    }
    const mostrar = async (camOn: boolean, onToggle = vi.fn()) => {
      render(<SalaTransmissao share={null} camOn={camOn} onToggleCamera={onToggle} onCameraTrocada={vi.fn()} onClose={vi.fn()} />)
      await screen.findByRole('option', { name: /Webcam/ })
    }
    const qualidade = () => within(screen.getByRole('radiogroup', { name: 'Qualidade da câmera' }))
    const guardou = (set: ReturnType<typeof cam>['set'], f: (v: any) => boolean) => set.mock.calls.some((c) => f((c[0] as any).video))

    it('mostra a qualidade e o jeito da câmera', async () => {
      cam()
      await mostrar(false)
      expect(screen.getByRole('radiogroup', { name: 'Qualidade da câmera' })).toBeTruthy()
      for (const n of ['Desligada', 'No canto', 'Solta']) expect(screen.getByRole('radio', { name: n })).toBeTruthy()
    })

    it('qualidade com câmera ligada aplica e guarda', async () => {
      const { q, set } = cam()
      await mostrar(true)
      fireEvent.click(qualidade().getByRole('radio', { name: /1080p30/ }))
      await waitFor(() => expect(q).toHaveBeenCalledWith('1080p30'))
      expect(guardou(set, (v) => v?.camera_preset === '1080p30')).toBe(true)
    })

    it('qualidade com câmera desligada só guarda', async () => {
      const { q, set } = cam()
      await mostrar(false)
      fireEvent.click(qualidade().getByRole('radio', { name: /1080p30/ }))
      await waitFor(() => expect(guardou(set, (v) => v?.camera_preset === '1080p30')).toBe(true))
      expect(q).not.toHaveBeenCalled()
    })

    it('jeito e canto com câmera ligada chamam cameraLayoutSet', async () => {
      const { l } = cam()
      await mostrar(true)
      fireEvent.click(screen.getByRole('radio', { name: 'Solta' }))
      await waitFor(() => expect(l).toHaveBeenCalledWith({ modo: 'solta' }))
      fireEvent.click(screen.getByRole('radio', { name: 'No canto' }))
      fireEvent.click(await screen.findByRole('radio', { name: 'Superior esquerdo' }))
      await waitFor(() => expect(l).toHaveBeenCalledWith({ modo: 'canto', canto: 'sup_esq' }))
    })

    it('trocar o jeito com câmera desligada só guarda', async () => {
      const { l, set } = cam()
      await mostrar(false)
      fireEvent.click(screen.getByRole('radio', { name: 'Solta' }))
      await waitFor(() => expect(guardou(set, (v) => v?.camera_layout?.modo === 'solta')).toBe(true))
      expect(l).not.toHaveBeenCalled()
    })

    it('"Desligada" com câmera ligada chama onToggleCamera', async () => {
      cam()
      const onToggle = vi.fn()
      await mostrar(true, onToggle)
      fireEvent.click(screen.getByRole('radio', { name: 'Desligada' }))
      expect(onToggle).toHaveBeenCalled()
    })

    it('câmera 720p/30 deixa 1080p60 desabilitado', async () => {
      base()
      vi.spyOn(api, 'sourcesList').mockResolvedValue({ ...mockSources(), cameras: [{ id: 'camera:0', kind: 'camera', name: 'Webcam simples', max_height: 720, max_fps: 30 }] })
      await mostrar(false)
      expect((qualidade().getByRole('radio', { name: /1080p60/ }) as HTMLButtonElement).disabled).toBe(true)
    })
  })
})
