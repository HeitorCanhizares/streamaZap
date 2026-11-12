import { describe, expect, it } from 'vitest'
import { alvoParaEntrar, linkConvite, rotuloAlvo } from './convite'

describe('linkConvite', () => {
  it('link https da página sala/ com o alvo no fragmento', () => {
    expect(linkConvite('ZAP-7K2Q')).toBe('https://heitorcanhizares.github.io/streamaZap/sala/#ZAP-7K2Q')
    expect(linkConvite('26.14.7.88')).toBe('https://heitorcanhizares.github.io/streamaZap/sala/#26.14.7.88')
  })
})

describe('alvoParaEntrar', () => {
  it('código vira { code } normalizado', () => {
    expect(alvoParaEntrar('ZAP-7K2Q')).toEqual({ code: 'ZAP-7K2Q' })
    expect(alvoParaEntrar(' zap-7k2q ')).toEqual({ code: 'ZAP-7K2Q' })
    expect(alvoParaEntrar('7K2Q')).toEqual({ code: 'ZAP-7K2Q' })
  })

  it('IPv4 vira { ip, port }, com 47800 quando falta a porta', () => {
    expect(alvoParaEntrar('26.14.7.88:47800')).toEqual({ ip: '26.14.7.88', port: 47800 })
    expect(alvoParaEntrar('26.14.7.88:5000')).toEqual({ ip: '26.14.7.88', port: 5000 })
    expect(alvoParaEntrar('192.168.0.10')).toEqual({ ip: '192.168.0.10', port: 47800 })
  })

  it('o resto é null', () => {
    for (const t of ['', 'ZAP-7K2O', 'ZAP-7K2QQ', 'exemplo.com', '26.14.7.300', '26.14.7.88:0', '26.14.7.88:70000', '26.14.7.88:abc', '[::1]:47800']) {
      expect(alvoParaEntrar(t), t).toBeNull()
    }
  })
})

describe('rotuloAlvo', () => {
  it('código, ip:porta ou o id da sala', () => {
    expect(rotuloAlvo({ code: 'ZAP-7K2Q' })).toBe('ZAP-7K2Q')
    expect(rotuloAlvo({ ip: '26.14.7.88', port: 47800 })).toBe('26.14.7.88:47800')
    expect(rotuloAlvo({ room_id: 'r1' })).toBe('r1')
  })
})
