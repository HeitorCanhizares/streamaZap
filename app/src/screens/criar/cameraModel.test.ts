// Tarefa front-05
import { describe, expect, it } from 'vitest'
import type { SourceItem } from '../../bridge/types'
import {
  cantoDoLayout,
  jeitoDoLayout,
  layoutDe,
  opcoesDaCamera,
  presetAjustado,
  textoJeitoCamera,
} from './cameraModel'

const camera = (max_height?: number, max_fps?: number): SourceItem => ({
  id: 'cam',
  kind: 'camera',
  name: 'Webcam',
  max_height,
  max_fps,
})

const desligados = (opcoes: ReturnType<typeof opcoesDaCamera>) =>
  opcoes.filter((o) => o.desligado).map((o) => o.preset)

describe('cantoDoLayout e jeitoDoLayout', () => {
  it('sem layout cai no padrão inf_dir e no jeito canto', () => {
    expect(cantoDoLayout(undefined)).toBe('inf_dir')
    expect(jeitoDoLayout(undefined)).toBe('canto')
  })

  it('lê o canto e o jeito de um layout', () => {
    expect(cantoDoLayout({ modo: 'canto', canto: 'sup_esq' })).toBe('sup_esq')
    expect(jeitoDoLayout({ modo: 'solta' })).toBe('solta')
  })

  it('modo solta não tem canto: devolve o padrão', () => {
    expect(cantoDoLayout({ modo: 'solta' })).toBe('inf_dir')
  })
})

describe('layoutDe', () => {
  it('monta o layout de cada jeito', () => {
    expect(layoutDe('canto', 'sup_dir')).toEqual({ modo: 'canto', canto: 'sup_dir' })
    expect(layoutDe('solta', 'sup_dir')).toEqual({ modo: 'solta' })
  })
})

describe('opcoesDaCamera', () => {
  it('sem câmera devolve as 4 qualidades, nenhuma desligada', () => {
    const opcoes = opcoesDaCamera(undefined)
    expect(opcoes.map((o) => o.preset)).toEqual(['720p30', '720p60', '1080p30', '1080p60'])
    expect(desligados(opcoes)).toEqual([])
    expect(opcoes.every((o) => o.motivo === undefined)).toBe(true)
  })

  it('câmera sem os limites não desliga nada', () => {
    expect(desligados(opcoesDaCamera(camera()))).toEqual([])
  })

  it('câmera 720p30 desliga tudo menos 720p30', () => {
    expect(desligados(opcoesDaCamera(camera(720, 30)))).toEqual(['720p60', '1080p30', '1080p60'])
  })

  it('câmera 1080p30 desliga só 720p60 e 1080p60', () => {
    expect(desligados(opcoesDaCamera(camera(1080, 30)))).toEqual(['720p60', '1080p60'])
  })

  it('câmera 720p60 desliga só 1080p30 e 1080p60', () => {
    expect(desligados(opcoesDaCamera(camera(720, 60)))).toEqual(['1080p30', '1080p60'])
  })

  it('o motivo do desligado diz o que a câmera não faz', () => {
    const opcao = opcoesDaCamera(camera(720, 30)).find((o) => o.preset === '1080p60')
    expect(opcao?.desligado).toBe(true)
    expect(opcao?.motivo).toBe('Sua câmera não faz 1080p60')
  })
})

describe('presetAjustado', () => {
  it('cai para 720p30 com câmera 720p30', () => {
    expect(presetAjustado('1080p60', camera(720, 30))).toBe('720p30')
  })

  it('cai para 1080p30 com câmera 1080p30', () => {
    expect(presetAjustado('1080p60', camera(1080, 30))).toBe('1080p30')
  })

  it('cai para 720p60 com câmera 720p60', () => {
    expect(presetAjustado('1080p60', camera(720, 60))).toBe('720p60')
  })

  it('preset que cabe fica como está', () => {
    expect(presetAjustado('720p30', undefined)).toBe('720p30')
    expect(presetAjustado('1080p60', camera(1080, 60))).toBe('1080p60')
  })
})

describe('textoJeitoCamera', () => {
  it('desligada e solta não dependem do canto', () => {
    expect(textoJeitoCamera('off', 'sup_esq')).toBe('Câmera: desligada')
    expect(textoJeitoCamera('solta', 'sup_esq')).toBe('Câmera: solta')
  })

  it('cada canto aparece com o nome em minúsculas', () => {
    expect(textoJeitoCamera('canto', 'sup_esq')).toBe('Câmera: no canto superior esquerdo')
    expect(textoJeitoCamera('canto', 'sup_dir')).toBe('Câmera: no canto superior direito')
    expect(textoJeitoCamera('canto', 'inf_esq')).toBe('Câmera: no canto inferior esquerdo')
    expect(textoJeitoCamera('canto', 'inf_dir')).toBe('Câmera: no canto inferior direito')
  })
})
