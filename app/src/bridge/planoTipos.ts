// Tipos de planos e cobrança no app (docs/planos/fase3-app-planos.md §4.4). Números de limite nunca
// entram aqui: só nomes, preços (do servidor) e o estado da tela.
import type { PlanId } from './types'

export type PlanoPago = 'espectador' | 'hd' | 'fullhd'
export type Ciclo = 'mensal' | 'anual'
/** Centavos (`config_publica.precos`). */
export type Precos = Record<PlanoPago, Record<Ciclo, number>>
/** Invariante: `cobrancaAtiva === true` implica `precos !== null`. */
export interface ConfigPublica { cobrancaAtiva: boolean; planoPadrao: PlanId | null; precos: Precos | null }
export type StatusAssinatura = 'ativa' | 'atrasada' | 'cancelada'
export interface Assinatura { plano: PlanoPago; status: StatusAssinatura; ciclo: Ciclo; validoAte: string }
export interface PlanoEstado {
  config: ConfigPublica
  assinatura: Assinatura | null
  /** Tela de planos aberta (só vale com cobrança). */
  telaAberta: boolean
  destaque: PlanoPago | null
  /** O link do pagamento foi aberto: o foco da janela atualiza a sessão. */
  aguardandoPagamento: boolean
  /** Plano comprado no último `assinar` (para saber quando o pagamento chegou). */
  comprado: PlanoPago | null
  ocupado: boolean
  /** Erro (role="alert"). */
  aviso: string | null
  /** Informação (role="status"). */
  info: string | null
}
