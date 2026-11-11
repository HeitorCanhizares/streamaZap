// Funções puras de planos e preços (docs/planos/fase3-app-planos.md §4.4).
import type { Preset, PlanId, PlanView, PresetLock } from './types'
import type { Assinatura, ConfigPublica, PlanoPago, Precos } from './planoTipos'
import { nomePlano } from './contaModel'
import { MEDIDAS_PRESET, PRESETS_ORDEM } from './presets'

export const CONFIG_DESLIGADA: ConfigPublica = { cobrancaAtiva: false, planoPadrao: null, precos: null }

const PLANOS_PAGOS: PlanoPago[] = ['espectador', 'hd', 'fullhd']
const IDS_PLANO: PlanId[] = ['nenhum', 'espectador', 'hd', 'fullhd']

function ehObjeto(x: unknown): x is Record<string, unknown> {
  return typeof x === 'object' && x !== null && !Array.isArray(x)
}

function inteiroPositivo(x: unknown): x is number {
  return typeof x === 'number' && Number.isInteger(x) && x > 0
}

function lerPrecos(x: unknown): Precos | null {
  if (!ehObjeto(x)) return null
  const saida: Partial<Precos> = {}
  for (const p of PLANOS_PAGOS) {
    const linha = x[p]
    if (!ehObjeto(linha)) return null
    const { mensal, anual } = linha
    if (!inteiroPositivo(mensal) || !inteiroPositivo(anual)) return null
    saida[p] = { mensal, anual }
  }
  return saida as Precos
}

/** Linha de `config_publica`. Fora do formato = `CONFIG_DESLIGADA` (com o `planoPadrao` lido, se válido). */
export function lerConfigPublica(data: unknown): ConfigPublica {
  if (!ehObjeto(data)) return CONFIG_DESLIGADA
  const padrao = data.plano_padrao
  const planoPadrao = IDS_PLANO.find((id) => id === padrao) ?? null
  const precos = data.cobranca_ativa === true ? lerPrecos(data.precos) : null
  if (precos === null) return { cobrancaAtiva: false, planoPadrao, precos: null }
  return { cobrancaAtiva: true, planoPadrao, precos }
}

/** Linha de `assinaturas`; fora do formato ou null = null. */
export function lerAssinatura(data: unknown): Assinatura | null {
  if (!ehObjeto(data)) return null
  const { plano, status, ciclo, valido_ate } = data
  const p = PLANOS_PAGOS.find((x) => x === plano)
  const s = (['ativa', 'atrasada', 'cancelada'] as const).find((x) => x === status)
  const c = (['mensal', 'anual'] as const).find((x) => x === ciclo)
  if (!p || !s || !c || typeof valido_ate !== 'string' || valido_ate === '') return null
  return { plano: p, status: s, ciclo: c, validoAte: valido_ate }
}

/** 390 → 'R$ 3,90'; 3900 → 'R$ 39'; 1490 → 'R$ 14,90'; 100000 → 'R$ 1.000'. */
export function formatarPreco(centavos: number): string {
  const reais = Math.floor(centavos / 100)
  const resto = centavos % 100
  const milhar = String(reais).replace(/\B(?=(\d{3})+(?!\d))/g, '.')
  return resto !== 0 ? `R$ ${milhar},${String(resto).padStart(2, '0')}` : `R$ ${milhar}`
}

/** 'R$ 3,90/mês ou R$ 39/ano'. */
export function textoPreco(precos: Precos, plano: PlanoPago): string {
  return `${formatarPreco(precos[plano].mensal)}/mês ou ${formatarPreco(precos[plano].anual)}/ano`
}

/** 'Assine o Espectador (R$ 3,90/mês ou R$ 39/ano) para assistir'. */
export function textoConvite(precos: Precos): string {
  return `Assine o Espectador (${textoPreco(precos, 'espectador')}) para assistir`
}

/** max(0, 12 - round(anual / mensal)). */
export function mesesDeGraca(precos: Precos, plano: PlanoPago): number {
  return Math.max(0, 12 - Math.round(precos[plano].anual / precos[plano].mensal))
}

/** nenhum 'Sem plano', espectador 'Espectador', hd 'HD', fullhd 'Full HD'. */
export function seloPlano(id: PlanId): string {
  switch (id) {
    case 'nenhum':
      return 'Sem plano'
    case 'espectador':
      return 'Espectador'
    case 'hd':
      return 'HD'
    case 'fullhd':
      return 'Full HD'
  }
}

/** Só dígitos. '' → ''; 11 ou 14 dígitos → os dígitos; outro tamanho → null. */
export function limparDocumento(texto: string): string | null {
  const d = texto.replace(/\D/g, '')
  if (d === '') return ''
  return d.length === 11 || d.length === 14 ? d : null
}

/** Linha "Plano:" da Conta. */
export function textoPlanoConta(plano: PlanView | undefined, config: ConfigPublica): string | null {
  if (!config.cobrancaAtiva) {
    const id = plano?.id ?? config.planoPadrao
    if (id === null) return null
    return id === 'nenhum' ? 'Plano: sem plano' : `Plano: beta gratuito (${seloPlano(id)})`
  }
  return plano ? `Plano: ${nomePlano(plano)}` : null
}

/** dd/mm/aaaa no fuso de São Paulo. */
export function formatarData(iso: string): string {
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return iso
  return new Intl.DateTimeFormat('pt-BR', {
    timeZone: 'America/Sao_Paulo',
    day: '2-digit',
    month: '2-digit',
    year: 'numeric',
  }).format(d)
}

/** Linha de validade da assinatura na Conta. */
export function textoAssinatura(a: Assinatura, agora: Date): string {
  const data = formatarData(a.validoAte)
  switch (a.status) {
    case 'ativa':
      return `Ativa · válida até ${data}`
    case 'atrasada':
      return `Pagamento atrasado · vale até ${data}`
    case 'cancelada':
      return new Date(a.validoAte).getTime() > agora.getTime()
        ? `Cancelada · vale até ${data}`
        : `Encerrada em ${data}`
  }
}

/** A trava do preset, se houver. */
export function travaDe(preset: Preset, travas: PresetLock[]): PresetLock | undefined {
  return travas.find((t) => t.preset === preset)
}

/** O maior preset sem trava com altura e fps <= os do pedido; sem nenhum livre, o próprio pedido. */
export function melhorPermitido(preset: Preset, travas: PresetLock[]): Preset {
  const alvo = MEDIDAS_PRESET[preset]
  let melhor: Preset | null = null
  for (const p of PRESETS_ORDEM) {
    const m = MEDIDAS_PRESET[p]
    if (travaDe(p, travas) || m.altura > alvo.altura || m.fps > alvo.fps) continue
    if (melhor === null) {
      melhor = p
      continue
    }
    const b = MEDIDAS_PRESET[melhor]
    if (m.altura > b.altura || (m.altura === b.altura && m.fps > b.fps)) melhor = p
  }
  return melhor ?? preset
}
