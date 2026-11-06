// Pedido de transmissão montado pela tela: aba da fonte, som e "só o som".
import { describe, expect, it } from 'vitest'
import { abaDaFonte, erroDoPedido, listaDaAba, montarPedido, semTransmitir, somDoPedido, textoBotaoCriar } from './criarModel'
import type { Sources } from '../../bridge/types'

const fontes: Sources = {
  screens: [{ id: 'screen:1', kind: 'screen', name: 'Tela 1' }],
  windows: [{ id: 'window:42', kind: 'window', name: 'Jogo' }],
  cameras: [{ id: 'camera:usb1', kind: 'camera', name: 'Webcam' }],
  apps: [],
  encoders: [],
}

describe('criarModel: pedido de transmissão', () => {
  it('listaDaAba: cada aba tem as suas fontes; só o som não tem fonte de vídeo', () => {
    expect(listaDaAba(fontes, 'telas').map((f) => f.id)).toEqual(['screen:1'])
    expect(listaDaAba(fontes, 'janelas').map((f) => f.id)).toEqual(['window:42'])
    expect(listaDaAba(fontes, 'webcam').map((f) => f.id)).toEqual(['camera:usb1'])
    expect(listaDaAba(fontes, 'audio')).toEqual([])
  })

  it('abaDaFonte reconhece cada tipo de id', () => {
    expect(abaDaFonte('screen:2')).toBe('telas')
    expect(abaDaFonte('synthetic:1920x1080')).toBe('telas')
    expect(abaDaFonte('window:-7')).toBe('janelas')
    expect(abaDaFonte('camera:usb1')).toBe('webcam')
    expect(abaDaFonte('none')).toBe('audio')
  })

  it('somDoPedido volta para a escolha da interface', () => {
    expect(somDoPedido('none')).toEqual({ sound: 'none', marcados: [] })
    expect(somDoPedido('all')).toEqual({ sound: 'all', marcados: [] })
    expect(somDoPedido(['app:1', 'app:2'])).toEqual({ sound: 'apps', marcados: ['app:1', 'app:2'] })
  })

  it('montarPedido: só o som manda source none; sem fonte escolhida devolve null', () => {
    expect(montarPedido('audio', null, 'all', [], '720p30', 'auto')).toEqual({ source: 'none', audio: 'all', preset: '720p30', encoder: 'auto' })
    expect(montarPedido('telas', 'screen:1', 'apps', ['app:9'], '1080p60', 'auto')).toEqual({ source: 'screen:1', audio: ['app:9'], preset: '1080p60', encoder: 'auto' })
    expect(montarPedido('telas', null, 'none', [], '720p30', 'auto')).toBeNull()
  })

  it('erroDoPedido: só o som exige som; apps exige um app marcado; vídeo exige fonte', () => {
    expect(erroDoPedido('audio', null, 'none', [])).toMatch(/escolha o som/)
    expect(erroDoPedido('audio', null, 'all', [])).toBeNull()
    expect(erroDoPedido('telas', 'screen:1', 'apps', [])).toMatch(/Marque pelo menos um app/)
    expect(erroDoPedido('telas', null, 'none', [])).toMatch(/Escolha o que compartilhar/)
    expect(erroDoPedido('janelas', 'window:42', 'none', [])).toBeNull()
  })

  it('"nada" (B1): não monta pedido e não dá erro, mesmo com som apps sem marcados', () => {
    expect(montarPedido('nada', null, 'none', [], '720p30', 'auto')).toBeNull()
    expect(montarPedido('nada', 'screen:0', 'all', [], '720p30', 'auto')).toBeNull()
    expect(erroDoPedido('nada', null, 'none', [])).toBeNull()
    expect(erroDoPedido('nada', null, 'apps', [])).toBeNull()
  })

  it('erroDoPedido de telas sem fonte continua pedindo a escolha', () => {
    expect(erroDoPedido('telas', null, 'none', [])).toBe('Escolha o que compartilhar.')
  })

  it('textoBotaoCriar e semTransmitir seguem a aba', () => {
    expect(textoBotaoCriar('nada', false)).toBe('Criar sala')
    expect(textoBotaoCriar('nada', true)).toBe('Criando…')
    expect(textoBotaoCriar('telas', false)).toBe('Iniciar transmissão')
    expect(textoBotaoCriar('audio', true)).toBe('Iniciando…')
    expect(semTransmitir('nada')).toBe(true)
    expect(semTransmitir('telas')).toBe(false)
  })

  it('listaDaAba de "nada" é vazia', () => {
    expect(listaDaAba(fontes, 'nada')).toEqual([])
  })
})
