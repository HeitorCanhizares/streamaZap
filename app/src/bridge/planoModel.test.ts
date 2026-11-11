import { describe, expect, it } from 'vitest'
import {
  CONFIG_DESLIGADA, formatarData, formatarPreco, lerAssinatura, lerConfigPublica, limparDocumento,
  melhorPermitido, mesesDeGraca, seloPlano, textoAssinatura, textoConvite, textoPlanoConta, textoPreco, travaDe,
} from './planoModel'
import { PRECOS_MOCK } from './mockConta'
import type { ConfigPublica } from './planoTipos'
import type { PresetLock } from './types'

const linha = { cobranca_ativa: true, plano_padrao: null, precos: PRECOS_MOCK }

describe('lerConfigPublica', () => {
  it('linha completa', () => {
    const c = lerConfigPublica(linha)
    expect(c.cobrancaAtiva).toBe(true)
    expect(c.precos).toEqual(PRECOS_MOCK)
  })
  it('preço faltando desliga', () => {
    const precos = { ...PRECOS_MOCK, hd: { mensal: 790 } }
    expect(lerConfigPublica({ ...linha, precos })).toEqual(CONFIG_DESLIGADA)
  })
  it('formato inválido', () => {
    for (const x of [null, 'x', 5]) expect(lerConfigPublica(x)).toEqual(CONFIG_DESLIGADA)
  })
  it('preço zero ou negativo desliga', () => {
    for (const v of [0, -5]) {
      const precos = { ...PRECOS_MOCK, hd: { mensal: v, anual: 7900 } }
      expect(lerConfigPublica({ ...linha, precos })).toEqual(CONFIG_DESLIGADA)
    }
  })
  it('plano padrão', () => {
    expect(lerConfigPublica({ ...linha, plano_padrao: 'ouro' }).planoPadrao).toBeNull()
    const c = lerConfigPublica({ cobranca_ativa: false, plano_padrao: 'hd', precos: null })
    expect(c.planoPadrao).toBe('hd')
    expect(c.cobrancaAtiva).toBe(false)
  })
})

describe('lerAssinatura', () => {
  it('válida', () => {
    expect(lerAssinatura({ plano: 'hd', status: 'ativa', ciclo: 'anual', valido_ate: '2026-12-10T15:00:00Z' }))
      .toEqual({ plano: 'hd', status: 'ativa', ciclo: 'anual', validoAte: '2026-12-10T15:00:00Z' })
  })
  it('inválida', () => {
    expect(lerAssinatura({ plano: 'hd', status: 'x', ciclo: 'anual', valido_ate: 'a' })).toBeNull()
    expect(lerAssinatura(null)).toBeNull()
  })
})

describe('preços', () => {
  it('formatarPreco', () => {
    expect(formatarPreco(390)).toBe('R$ 3,90')
    expect(formatarPreco(3900)).toBe('R$ 39')
    expect(formatarPreco(1490)).toBe('R$ 14,90')
    expect(formatarPreco(100000)).toBe('R$ 1.000')
  })
  it('textos', () => {
    expect(textoPreco(PRECOS_MOCK, 'hd')).toBe('R$ 7,90/mês ou R$ 79/ano')
    expect(textoConvite(PRECOS_MOCK)).toBe('Assine o Espectador (R$ 3,90/mês ou R$ 39/ano) para assistir')
  })
  it('mesesDeGraca', () => {
    for (const p of ['espectador', 'hd', 'fullhd'] as const) expect(mesesDeGraca(PRECOS_MOCK, p)).toBe(2)
  })
  it('seloPlano', () => {
    expect(seloPlano('nenhum')).toBe('Sem plano')
    expect(seloPlano('espectador')).toBe('Espectador')
    expect(seloPlano('hd')).toBe('HD')
    expect(seloPlano('fullhd')).toBe('Full HD')
  })
})

describe('limparDocumento', () => {
  it('casos', () => {
    expect(limparDocumento('123.456.789-09')).toBe('12345678909')
    expect(limparDocumento('')).toBe('')
    expect(limparDocumento('123456789012')).toBeNull()
    expect(limparDocumento('12345678000199')).toBe('12345678000199')
  })
})

describe('textoPlanoConta', () => {
  const sem: ConfigPublica = { cobrancaAtiva: false, planoPadrao: null, precos: null }
  const com: ConfigPublica = { cobrancaAtiva: true, planoPadrao: null, precos: PRECOS_MOCK }
  it('sem cobrança', () => {
    expect(textoPlanoConta({ id: 'fullhd', pode_entrar: true }, sem)).toBe('Plano: beta gratuito (Full HD)')
    expect(textoPlanoConta(undefined, { ...sem, planoPadrao: 'hd' })).toBe('Plano: beta gratuito (HD)')
    expect(textoPlanoConta({ id: 'nenhum', pode_entrar: true }, sem)).toBe('Plano: sem plano')
    expect(textoPlanoConta(undefined, sem)).toBeNull()
  })
  it('com cobrança', () => {
    expect(textoPlanoConta({ id: 'hd', pode_entrar: true }, com)).toBe('Plano: Transmissor HD')
    expect(textoPlanoConta(undefined, com)).toBeNull()
  })
})

describe('datas', () => {
  it('formatarData', () => {
    expect(formatarData('2026-12-10T15:00:00Z')).toBe('10/12/2026')
    expect(formatarData('lixo')).toBe('lixo')
  })
  it('textoAssinatura', () => {
    const base = { plano: 'hd', ciclo: 'mensal', validoAte: '2026-12-10T15:00:00Z' } as const
    const agora = new Date('2026-11-11T12:00:00Z')
    expect(textoAssinatura({ ...base, status: 'ativa' }, agora)).toBe('Ativa · válida até 10/12/2026')
    expect(textoAssinatura({ ...base, status: 'atrasada' }, agora)).toBe('Pagamento atrasado · vale até 10/12/2026')
    expect(textoAssinatura({ ...base, status: 'cancelada' }, agora)).toBe('Cancelada · vale até 10/12/2026')
    expect(textoAssinatura({ ...base, status: 'cancelada' }, new Date('2027-01-01T00:00:00Z'))).toBe('Encerrada em 10/12/2026')
  })
})

describe('travas', () => {
  const hd: PresetLock[] = [{ preset: '1080p30', needs: 'fullhd' }, { preset: '1080p60', needs: 'fullhd' }]
  it('melhorPermitido', () => {
    expect(melhorPermitido('1080p60', hd)).toBe('720p60')
    expect(melhorPermitido('1080p30', hd)).toBe('720p30')
    expect(melhorPermitido('720p30', hd)).toBe('720p30')
    expect(melhorPermitido('1080p60', [])).toBe('1080p60')
    const todas: PresetLock[] = (['720p30', '720p60', '1080p30', '1080p60'] as const).map((preset) => ({ preset }))
    expect(melhorPermitido('1080p60', todas)).toBe('1080p60')
  })
  it('travaDe', () => {
    expect(travaDe('1080p30', hd)).toEqual(hd[0])
    expect(travaDe('720p30', hd)).toBeUndefined()
  })
})
