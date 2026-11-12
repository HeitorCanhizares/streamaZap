// Tarefa criar-06
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import type { SourceItem } from '../../bridge/types'
import { CriarQualidade, type CriarQualidadeProps } from './CriarQualidade'

afterEach(() => cleanup())

const ENCODERS = ['NVIDIA NVENC', 'Intel QuickSync', 'CPU (x264)']

function props(over: Partial<CriarQualidadeProps> = {}): CriarQualidadeProps {
  return {
    encoders: ENCODERS,
    preset: '720p30',
    onPreset: vi.fn(),
    encoder: 'auto',
    onEncoder: vi.fn(),
    adaptavel: false,
    onAdaptavel: vi.fn(),
    ...over,
  }
}

describe('CriarQualidade', () => {
  it('titulo: mostra "06" e "Qualidade"', () => {
    render(<CriarQualidade {...props()} />)
    expect(screen.getByText('06')).toBeTruthy()
    expect(screen.getByRole('heading').textContent).toBe('06Qualidade')
  })

  it('quatro radios com res e fps dos presets', () => {
    render(<CriarQualidade {...props()} />)
    const radios = screen.getAllByRole('radio')
    expect(radios.length).toBe(4)
    expect(radios[0].textContent).toBe('720p30 fps · Radmin')
    expect(radios[1].textContent).toBe('720p60 fps')
    expect(radios[2].textContent).toBe('1080p30 fps')
    expect(radios[3].textContent).toBe('1080p60 fps')
  })

  it('aria-checked segue o preset e clicar chama onPreset com o valor', () => {
    const onPreset = vi.fn()
    const { rerender } = render(<CriarQualidade {...props({ preset: '720p30', onPreset })} />)
    let radios = screen.getAllByRole('radio')
    expect(radios.map((r) => r.getAttribute('aria-checked'))).toEqual(['true', 'false', 'false', 'false'])
    fireEvent.click(radios[3])
    expect(onPreset).toHaveBeenLastCalledWith('1080p60')
    rerender(<CriarQualidade {...props({ preset: '1080p60', onPreset })} />)
    radios = screen.getAllByRole('radio')
    expect(radios.map((r) => r.getAttribute('aria-checked'))).toEqual(['false', 'false', 'false', 'true'])
  })

  it('select de Codificador tem Automático e os encoders recebidos', () => {
    render(<CriarQualidade {...props()} />)
    const select = screen.getByLabelText('Codificador') as HTMLSelectElement
    const opcoes = Array.from(select.options).map((o) => o.textContent)
    expect(opcoes).toEqual(['Automático', ...ENCODERS])
    expect(select.value).toBe('auto')
  })

  it('lista do núcleo com "auto": uma opção só por valor, nomes em português', () => {
    render(<CriarQualidade {...props({ encoders: ['auto', 'hardware', 'software'] })} />)
    const select = screen.getByLabelText('Codificador') as HTMLSelectElement
    const opcoes = Array.from(select.options)
    expect(opcoes.map((o) => o.value)).toEqual(['auto', 'hardware', 'software'])
    expect(opcoes.map((o) => o.textContent)).toEqual(['Automático', 'Placa de vídeo', 'Processador'])
  })

  it('mudar o codificador chama onEncoder', () => {
    const onEncoder = vi.fn()
    render(<CriarQualidade {...props({ onEncoder })} />)
    fireEvent.change(screen.getByLabelText('Codificador'), { target: { value: 'CPU (x264)' } })
    expect(onEncoder).toHaveBeenLastCalledWith('CPU (x264)')
  })

  it('switch reflete aria-checked e clique chama onAdaptavel com o inverso', () => {
    const onAdaptavel = vi.fn()
    const { rerender } = render(<CriarQualidade {...props({ adaptavel: false, onAdaptavel })} />)
    const sw = screen.getByRole('switch', { name: /Qualidade adaptável/ })
    expect(sw.getAttribute('aria-checked')).toBe('false')
    fireEvent.click(sw)
    expect(onAdaptavel).toHaveBeenLastCalledWith(true)
    rerender(<CriarQualidade {...props({ adaptavel: true, onAdaptavel })} />)
    expect(screen.getByRole('switch', { name: /Qualidade adaptável/ }).getAttribute('aria-checked')).toBe('true')
    fireEvent.click(screen.getByRole('switch', { name: /Qualidade adaptável/ }))
    expect(onAdaptavel).toHaveBeenLastCalledWith(false)
  })

  it('mostra o texto explicativo do switch', () => {
    render(<CriarQualidade {...props()} />)
    expect(screen.getByText(/— baixa sozinha se alguém estiver com a conexão lenta\./)).toBeTruthy()
  })

  describe('com câmera', () => {
    it('sem props de câmera só existe o grupo "Qualidade"', () => {
      render(<CriarQualidade {...props()} />)
      expect(screen.getAllByRole('radiogroup').length).toBe(1)
      expect(screen.getByRole('radiogroup', { name: 'Qualidade' })).toBeTruthy()
      expect(screen.getAllByRole('radio').length).toBe(4)
    })

    it('com cameraPreset e onCameraPreset mostra os dois grupos', () => {
      render(<CriarQualidade {...props({ cameraPreset: '720p30', onCameraPreset: vi.fn() })} />)
      expect(screen.getByRole('radiogroup', { name: 'Qualidade da tela' })).toBeTruthy()
      expect(screen.getByRole('radiogroup', { name: 'Qualidade da câmera' })).toBeTruthy()
      expect(screen.getAllByRole('radio').length).toBe(8)
      expect(screen.getByText('Tela')).toBeTruthy()
      expect(screen.getByText('Câmera')).toBeTruthy()
    })

    it('cada grupo chama o seu callback', () => {
      const onPreset = vi.fn()
      const onCameraPreset = vi.fn()
      render(<CriarQualidade {...props({ onPreset, cameraPreset: '720p30', onCameraPreset })} />)
      const cam = within(screen.getByRole('radiogroup', { name: 'Qualidade da câmera' })).getAllByRole('radio')
      fireEvent.click(cam[2])
      expect(onCameraPreset).toHaveBeenLastCalledWith('1080p30')
      expect(onPreset).not.toHaveBeenCalled()
      const tela = within(screen.getByRole('radiogroup', { name: 'Qualidade da tela' })).getAllByRole('radio')
      fireEvent.click(tela[1])
      expect(onPreset).toHaveBeenLastCalledWith('720p60')
      expect(onCameraPreset).toHaveBeenCalledTimes(1)
    })

    it('câmera que só faz 720p30 ajusta o preset uma vez', () => {
      const onCameraPreset = vi.fn()
      const cameraInfo = { id: 'c', label: 'Cam', max_height: 720, max_fps: 30 } as unknown as SourceItem
      render(<CriarQualidade {...props({ cameraPreset: '1080p60', onCameraPreset, cameraInfo })} />)
      expect(onCameraPreset).toHaveBeenCalledTimes(1)
      expect(onCameraPreset).toHaveBeenCalledWith('720p30')
    })

    it('câmera que faz 1080p60 não muda o preset sozinha', () => {
      const onCameraPreset = vi.fn()
      const cameraInfo = { id: 'c', label: 'Cam', max_height: 1080, max_fps: 60 } as unknown as SourceItem
      render(<CriarQualidade {...props({ cameraPreset: '1080p60', onCameraPreset, cameraInfo })} />)
      expect(onCameraPreset).not.toHaveBeenCalled()
    })
  })
})
