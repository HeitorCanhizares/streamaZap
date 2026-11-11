import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react'

vi.mock('../../bridge/planoAcoes', () => ({ abrirPlanos: vi.fn() }))

import { abrirPlanos } from '../../bridge/planoAcoes'
import { PRECOS_MOCK } from '../../bridge/mockConta'
import { resetPlano, setPlanoEstado } from '../../bridge/planoStore'
import type { PresetLock, SourceItem } from '../../bridge/types'
import { CriarQualidade } from './CriarQualidade'
import { CameraQualidade } from './CameraQualidade'

const TRAVAS: PresetLock[] = [
  { preset: '1080p30', needs: 'fullhd' },
  { preset: '1080p60', needs: 'fullhd' },
]

function tela(onPreset = vi.fn(), travasTela?: PresetLock[]) {
  render(
    <CriarQualidade
      encoders={[]} preset="720p30" onPreset={onPreset} encoder="auto" onEncoder={() => {}}
      adaptavel={false} onAdaptavel={() => {}} travasTela={travasTela}
    />,
  )
  return onPreset
}

const radio = (nome: RegExp) => screen.getAllByRole('radio').find((r) => nome.test(r.textContent ?? '')) as HTMLElement

beforeEach(() => { resetPlano(); vi.mocked(abrirPlanos).mockClear() })
afterEach(cleanup)

describe('cadeados na qualidade', () => {
  it('opção travada mostra selo, atributos e título', () => {
    tela(vi.fn(), TRAVAS)
    const r = screen.getAllByRole('radio')[3]
    expect(r.getAttribute('data-travado')).toBe('true')
    expect(r.getAttribute('aria-disabled')).toBe('true')
    expect(within(r).getByText('Full HD')).toBeTruthy()
    expect(r.getAttribute('title')).toBe('Precisa do plano Full HD')
  })

  it('clique sem cobrança explica e não troca o preset', () => {
    const onPreset = tela(vi.fn(), TRAVAS)
    fireEvent.click(screen.getAllByRole('radio')[3])
    expect(onPreset).not.toHaveBeenCalled()
    expect(screen.getByRole('status').textContent).toBe('Seu plano não permite essa qualidade')
  })

  it('clique com cobrança abre os planos em fullhd', () => {
    act(() => setPlanoEstado({ config: { cobrancaAtiva: true, planoPadrao: null, precos: PRECOS_MOCK } }))
    const onPreset = tela(vi.fn(), TRAVAS)
    fireEvent.click(screen.getAllByRole('radio')[3])
    expect(abrirPlanos).toHaveBeenCalledWith('fullhd')
    expect(onPreset).not.toHaveBeenCalled()
  })

  it('opção livre ainda chama onPreset', () => {
    const onPreset = tela(vi.fn(), TRAVAS)
    fireEvent.click(screen.getAllByRole('radio')[1])
    expect(onPreset).toHaveBeenCalledWith('720p60')
  })

  it('câmera: o que ela não faz fica disabled sem selo; o livre funciona', () => {
    const cam = { id: 'camera:0', kind: 'camera', name: 'c', max_height: 720, max_fps: 60 } as SourceItem
    const onPreset = vi.fn()
    render(<CameraQualidade preset="720p30" onPreset={onPreset} camera={cam} travas={TRAVAS} />)
    const radios = screen.getAllByRole('radio') as HTMLButtonElement[]
    expect(radios[2].disabled).toBe(true)
    expect(radios[2].getAttribute('data-travado')).toBeNull()
    expect(within(radios[2]).queryByTestId('selo-plano')).toBeNull()
    fireEvent.click(radios[1])
    expect(onPreset).toHaveBeenCalledWith('720p60')
  })

  it('sem props de trava nada fica travado', () => {
    tela()
    render(<CameraQualidade preset="720p30" onPreset={() => {}} />)
    expect(document.querySelector('[data-travado]')).toBeNull()
    expect(radio(/720p/)).toBeTruthy()
  })
})
