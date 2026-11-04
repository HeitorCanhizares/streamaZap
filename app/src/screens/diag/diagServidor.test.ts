import { describe, it, expect } from 'vitest'
import { mockSnapshot } from '../../bridge/mock'
import type { Snapshot } from '../../bridge/types'
import { checkServidor } from './diagServidor'

const comServer = (server: Snapshot['server']): Snapshot => {
  const s = mockSnapshot()
  s.server = server
  return s
}

describe('checkServidor', () => {
  it('sem server (undefined): soon com linha info de não configurado', () => {
    const c = checkServidor(comServer(undefined))
    expect(c.id).toBe('srv')
    expect(c.status).toBe('soon')
    expect(c.lines).toEqual([{
      tom: 'info',
      text: 'Servidor não configurado',
      sub: 'Informe o endereço em Configurações › Conexão para usar salas Online.',
    }])
  })

  it('state off: mesmo resultado de não configurado', () => {
    const c = checkServidor(comServer({ state: 'off' }))
    expect(c.id).toBe('srv')
    expect(c.status).toBe('soon')
    expect(c.lines[0].text).toBe('Servidor não configurado')
    expect(c.lines[0].tom).toBe('info')
  })

  it('connecting: warn com linha warn de conectando', () => {
    const c = checkServidor(comServer({ state: 'connecting' }))
    expect(c.id).toBe('srv')
    expect(c.status).toBe('warn')
    expect(c.lines).toEqual([{ tom: 'warn', text: 'Conectando ao servidor…' }])
  })

  it('online com nome e ping: ok, nome no texto e ping arredondado no badge', () => {
    const c = checkServidor(comServer({ state: 'online', name: 'Casa', ping_ms: 23.6 }))
    expect(c.id).toBe('srv')
    expect(c.status).toBe('ok')
    expect(c.lines).toEqual([{ tom: 'ok', text: 'Conectado a Casa', badge: '24 ms' }])
  })

  it('online sem nome e sem ping: ok, texto simples e sem badge', () => {
    const c = checkServidor(comServer({ state: 'online' }))
    expect(c.id).toBe('srv')
    expect(c.status).toBe('ok')
    expect(c.lines).toEqual([{ tom: 'ok', text: 'Conectado' }])
    expect('badge' in c.lines[0]).toBe(false)
  })

  it('error com message: warn com sub da mensagem', () => {
    const c = checkServidor(comServer({ state: 'error', message: 'Senha incorreta' }))
    expect(c.id).toBe('srv')
    expect(c.status).toBe('warn')
    expect(c.lines).toEqual([{ tom: 'bad', text: 'Servidor desconectado', sub: 'Senha incorreta' }])
  })

  it('error sem message: warn sem sub', () => {
    const c = checkServidor(comServer({ state: 'error' }))
    expect(c.id).toBe('srv')
    expect(c.status).toBe('warn')
    expect(c.lines).toEqual([{ tom: 'bad', text: 'Servidor desconectado' }])
    expect('sub' in c.lines[0]).toBe(false)
  })
})
