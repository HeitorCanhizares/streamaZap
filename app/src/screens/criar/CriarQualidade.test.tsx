// Tarefa criar-06
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
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
    expect(radios[0].textContent).toBe('480p30 fps')
    expect(radios[1].textContent).toBe('720p30 fps · Radmin')
    expect(radios[2].textContent).toBe('720p60 fps')
    expect(radios[3].textContent).toBe('1080p60 fps')
  })

  it('aria-checked segue o preset e clicar chama onPreset com o valor', () => {
    const onPreset = vi.fn()
    const { rerender } = render(<CriarQualidade {...props({ preset: '720p30', onPreset })} />)
    let radios = screen.getAllByRole('radio')
    expect(radios.map((r) => r.getAttribute('aria-checked'))).toEqual(['false', 'true', 'false', 'false'])
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
})
