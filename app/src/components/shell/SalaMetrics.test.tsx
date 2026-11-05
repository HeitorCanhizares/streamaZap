// Tarefa shell-03
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render } from '@testing-library/react'
import { SalaMetrics } from './SalaMetrics'
import type { ShareView, TileView } from '../../bridge/types'

afterEach(() => cleanup())

function tileComPing(ping_ms: number): TileView {
  return {
    tile: 1,
    track_id: 'trk-1',
    layer: 0,
    width: 1920,
    height: 1080,
    fps: 60,
    kbps: 6200,
    ping_ms,
    smoothing_ms: 120,
    reason: 'teste',
  }
}

const share: ShareView = {
  source: 'screen:0',
  audio: 'none',
  preset: '720p60',
  encoder: 'x264',
  layers: [0],
  viewers_per_layer: [1],
  encode_ms: 4,
  conversion: [{ layer: 0, converter: 'gpu' }],
}

// Pega o span colorido do ping (o que tem o texto "N ms" ou "—" no fim do primeiro bloco).
function spanPing(container: HTMLElement): HTMLElement {
  const alvo = Array.from(container.querySelectorAll('span')).find(
    (s) => /ms$|^—$/.test(s.textContent ?? '') && s.style.color !== '',
  )
  if (!alvo) throw new Error('span do ping não encontrado')
  return alvo as HTMLElement
}

describe('SalaMetrics', () => {
  it('com tile mostra ping, suavização e qualidade', () => {
    const { container } = render(<SalaMetrics tile={tileComPing(38)} share={null} />)
    const texto = container.textContent ?? ''
    expect(texto).toContain('via servidor sa-east · 38 ms')
    expect(texto).toContain('suav. 120 ms auto')
    expect(texto).toContain('1080p60 · 6,2 Mbps')
  })

  it('cor do ping segue a faixa: verde, amarelo, vermelho', () => {
    const verde = render(<SalaMetrics tile={tileComPing(38)} share={null} />)
    expect(spanPing(verde.container).style.color).toBe('var(--verde)')
    verde.unmount()

    const amarelo = render(<SalaMetrics tile={tileComPing(75)} share={null} />)
    expect(spanPing(amarelo.container).style.color).toBe('var(--amarelo)')
    amarelo.unmount()

    const vermelho = render(<SalaMetrics tile={tileComPing(140)} share={null} />)
    expect(spanPing(vermelho.container).style.color).toBe('var(--vermelho-texto)')
  })

  it('sem tile e sem share mostra travessão e não lança', () => {
    const { container } = render(<SalaMetrics tile={null} share={null} />)
    const texto = container.textContent ?? ''
    expect(texto).toContain('—')
    expect(texto).toContain('suav. —')
    expect(spanPing(container).style.color).toBe('var(--texto-3)')
  })

  it('sem tile mas com share mostra o preset da transmissão', () => {
    const { container } = render(<SalaMetrics tile={null} share={share} />)
    expect(container.textContent ?? '').toContain('720p60')
  })
})
