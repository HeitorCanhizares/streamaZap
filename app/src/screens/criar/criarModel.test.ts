// Tarefa criar-01
import { describe, expect, it } from 'vitest'
import {
  ACESSOS,
  MODOS,
  PRESETS_UI,
  acessoTexto,
  cenaDaFonte,
  fonteMeta,
  modoHint,
  modoTexto,
  qualidadeTexto,
  seguroTexto,
  somTexto,
} from './criarModel'

describe('criarModel: textos de modo', () => {
  it('modo_textos', () => {
    expect(modoHint('online')).toBe(
      'A sala vai pro servidor e aparece pra todo mundo que usa o app. Pra entrar, é só mandar o código no zap.',
    )
    expect(modoHint('local')).toBe(
      'A sala aparece sozinha pra quem estiver na mesma rede do Radmin ou na sua rede local.',
    )
    expect(modoTexto('online')).toBe('Online · via servidor')
    expect(modoTexto('local')).toBe('Local · Radmin / rede local')
    expect(seguroTexto('online')).toBe('Conexão com o servidor autenticada')
    expect(seguroTexto('local')).toBe('Firewall liberado para receber conexões')
  })
})

describe('criarModel: acesso e som', () => {
  it('acesso_e_som', () => {
    expect(acessoTexto('senha')).toBe('Com senha')
    expect(acessoTexto('aberta')).toBe('Aberta para a rede')

    expect(somTexto('none', [])).toBe('Sem som')
    expect(somTexto('none', ['cs2.exe'])).toBe('Sem som')
    expect(somTexto('all', [])).toBe('Todo o som do PC')
    expect(somTexto('apps', [])).toBe('Nenhum app marcado')
    expect(somTexto('apps', ['cs2.exe', 'Spotify.exe'])).toBe('Só cs2.exe, Spotify.exe')
  })
})

describe('criarModel: qualidade', () => {
  it('qualidade_texto', () => {
    expect(qualidadeTexto('720p30', true)).toBe('720p30 · 2,5 Mbps · adaptável')
    expect(qualidadeTexto('720p30', false)).toBe('720p30 · 2,5 Mbps')
  })
})

describe('criarModel: fonte', () => {
  it('fonte_meta', () => {
    expect(fonteMeta({ id: 's1', kind: 'screen', name: 'Tela 1' })).toBe('Tela inteira')
    expect(fonteMeta({ id: 'w1', kind: 'window', name: 'Counter-Strike 2' })).toBe('Janela')
    expect(fonteMeta({ id: 'c1', kind: 'camera', name: 'Webcam USB' })).toBe('Webcam')
    expect(fonteMeta({ id: 'a1', kind: 'app', name: 'Spotify.exe' })).toBe('Spotify.exe')
  })

  it('cena_da_fonte', () => {
    expect(cenaDaFonte('telas', 0)).toBe('game')
    expect(cenaDaFonte('telas', 1)).toBe('desktop')
    expect(cenaDaFonte('telas', 2)).toBe('game')
    expect(cenaDaFonte('janelas', 2)).toBe('code')
    expect(cenaDaFonte('janelas', 3)).toBe('game')
    expect(cenaDaFonte('webcam', 0)).toBe('camc')
    expect(cenaDaFonte('webcam', 1)).toBe('cama')
    expect(cenaDaFonte('webcam', 2)).toBe('camc')
  })
})

describe('criarModel: constantes', () => {
  it('constantes', () => {
    expect(MODOS).toHaveLength(2)
    expect(MODOS.map((m) => m.value)).toEqual(['online', 'local'])
    expect(MODOS[0].cor).toBe('#3BF08A')
    expect(MODOS[1].cor).toBe('#5CD6FF')

    expect(ACESSOS).toHaveLength(2)
    expect(ACESSOS.map((a) => a.value)).toEqual(['aberta', 'senha'])

    expect(PRESETS_UI).toHaveLength(4)
    expect(PRESETS_UI.map((p) => p.value)).toEqual(['480p30', '720p30', '720p60', '1080p60'])
    expect(PRESETS_UI[1]).toEqual({ value: '720p30', res: '720p', fps: '30 fps · Radmin', bitrate: '2,5 Mbps' })
  })
})
