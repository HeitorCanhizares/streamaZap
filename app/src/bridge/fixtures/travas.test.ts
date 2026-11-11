import { describe, expect, it } from 'vitest'
import { travasDoPlano } from './travas'

const TRAVAS_SEM_TRANSMISSAO = [
  { preset: '720p30', needs: 'hd' },
  { preset: '720p60', needs: 'hd' },
  { preset: '1080p30', needs: 'fullhd' },
  { preset: '1080p60', needs: 'fullhd' },
]

describe('travasDoPlano', () => {
  it('nenhum: não transmite e trava todos os presets', () => {
    const t = travasDoPlano('nenhum')
    expect(t.can_publish).toBe(false)
    expect(t.screen).toEqual(TRAVAS_SEM_TRANSMISSAO)
    expect(t.camera).toEqual(TRAVAS_SEM_TRANSMISSAO)
  })

  it('espectador: não transmite e trava os mesmos presets', () => {
    const t = travasDoPlano('espectador')
    expect(t.can_publish).toBe(false)
    expect(t.screen).toEqual(TRAVAS_SEM_TRANSMISSAO)
    expect(t.camera).toEqual(TRAVAS_SEM_TRANSMISSAO)
  })

  it('hd: transmite e trava só 1080p', () => {
    const t = travasDoPlano('hd')
    expect(t.can_publish).toBe(true)
    const esperado = [
      { preset: '1080p30', needs: 'fullhd' },
      { preset: '1080p60', needs: 'fullhd' },
    ]
    expect(t.screen).toEqual(esperado)
    expect(t.camera).toEqual(esperado)
  })

  it('fullhd: transmite sem travas', () => {
    const t = travasDoPlano('fullhd')
    expect(t.can_publish).toBe(true)
    expect(t.screen).toEqual([])
    expect(t.camera).toEqual([])
  })

  it('screen e camera têm o mesmo conteúdo, mas arrays diferentes', () => {
    const t = travasDoPlano('nenhum')
    expect(t.screen).toEqual(t.camera)
    expect(t.screen).not.toBe(t.camera)
    expect(t.screen[0]).not.toBe(t.camera[0])
  })

  it('cada chamada devolve objetos novos (quem chama pode mutar)', () => {
    const a = travasDoPlano('hd')
    const b = travasDoPlano('hd')
    expect(a).not.toBe(b)
    expect(a.screen).not.toBe(b.screen)
    expect(a.screen[0]).not.toBe(b.screen[0])
    a.screen[0].needs = 'hd'
    expect(b.screen[0].needs).toBe('fullhd')
  })
})
