import { describe, it, expect } from 'vitest'
import { barraServidor, textoTeste, normalizarCodigo, conviteTexto } from './serverModel'
import type { ServerTestResult } from './types'

describe('barraServidor', () => {
  it('sem status ou desligado: não configurado', () => {
    expect(barraServidor(undefined)).toEqual({ texto: 'Servidor · não configurado', tom: 'off', ms: null })
    expect(barraServidor({ state: 'off' })).toEqual({ texto: 'Servidor · não configurado', tom: 'off', ms: null })
  })
  it('conectando: aviso', () => {
    expect(barraServidor({ state: 'connecting' })).toEqual({ texto: 'Servidor · conectando…', tom: 'warn', ms: null })
  })
  it('online com ping: arredonda o ms', () => {
    expect(barraServidor({ state: 'online', ping_ms: 41.6 })).toEqual({ texto: 'Servidor online · 42 ms', tom: 'ok', ms: 42 })
  })
  it('online sem ping', () => {
    expect(barraServidor({ state: 'online' })).toEqual({ texto: 'Servidor online', tom: 'ok', ms: null })
  })
  it('erro: desconectado', () => {
    expect(barraServidor({ state: 'error', message: 'x' })).toEqual({ texto: 'Servidor · desconectado', tom: 'bad', ms: null })
  })
})

describe('textoTeste', () => {
  const ok = (ping: number | null): ServerTestResult => ({ ok: true, name: 'Sala', ping_ms: ping, code: null, message: '' })
  const falha = (message: string): ServerTestResult => ({ ok: false, name: null, ping_ms: null, code: null, message })

  it('testando tem prioridade sobre o resultado', () => {
    expect(textoTeste(ok(10), true)).toEqual({ texto: 'Conectando…', cor: '#A3BDB1' })
  })
  it('ainda não testado', () => {
    expect(textoTeste(null, false)).toEqual({ texto: 'Ainda não testado', cor: '#7E978B' })
  })
  it('sucesso com ping', () => {
    expect(textoTeste(ok(41.6), false)).toEqual({ texto: 'Conectado · senha aceita · 42 ms', cor: '#3BF08A' })
  })
  it('sucesso sem ping', () => {
    expect(textoTeste(ok(null), false)).toEqual({ texto: 'Conectado · senha aceita', cor: '#3BF08A' })
  })
  it('falha com mensagem', () => {
    expect(textoTeste(falha('senha errada'), false)).toEqual({ texto: 'senha errada', cor: '#FF8592' })
  })
  it('falha com mensagem vazia', () => {
    expect(textoTeste(falha(''), false)).toEqual({ texto: 'Não foi possível conectar', cor: '#FF8592' })
  })
})

describe('normalizarCodigo', () => {
  it('aceita formatos válidos', () => {
    expect(normalizarCodigo('zap-7k2q')).toBe('ZAP-7K2Q')
    expect(normalizarCodigo(' 7k2q ')).toBe('ZAP-7K2Q')
    expect(normalizarCodigo('ZAP7K2Q')).toBe('ZAP-7K2Q')
  })
  it('rejeita inválidos', () => {
    for (const t of ['ZAP-7K2', 'ZAP-7K2QQ', 'ZAP-0K2Q', '']) {
      expect(normalizarCodigo(t)).toBeNull()
    }
  })
})

describe('conviteTexto', () => {
  it('com nome da sala', () => {
    expect(conviteTexto('ZAP-7K2Q', ' Jogatina ')).toBe('Entra na minha sala "Jogatina" no streamaZap: https://heitorcanhizares.github.io/streamaZap/sala/#ZAP-7K2Q (código ZAP-7K2Q)')
  })
  it('sem nome da sala', () => {
    expect(conviteTexto('ZAP-7K2Q', '   ')).toBe('Entra na minha sala no streamaZap: https://heitorcanhizares.github.io/streamaZap/sala/#ZAP-7K2Q (código ZAP-7K2Q)')
  })
})
