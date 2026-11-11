// Funções puras da conta (plano contas-planos-camera §7). Tarefa front-03.
import type { AccountView, PlanId, PlanView } from './types'
import type { ContaIdentidade, ProvedorOAuth } from './contaTipos'

/** Nome do provedor na tela: google -> 'Google', azure -> 'Microsoft', email -> 'E-mail'; outro: o próprio texto. */
export function rotuloProvedor(provider: string): string {
  switch (provider) {
    case 'google':
      return 'Google'
    case 'azure':
      return 'Microsoft'
    case 'email':
      return 'E-mail'
    default:
      return provider
  }
}

/** Nome do plano (sem preço): nenhum 'Sem plano', espectador 'Espectador', hd 'Transmissor HD', fullhd 'Transmissor Full HD'. */
export function nomePlano(plano: PlanView | { id: PlanId }): string {
  switch (plano.id) {
    case 'nenhum':
      return 'Sem plano'
    case 'espectador':
      return 'Espectador'
    case 'hd':
      return 'Transmissor HD'
    case 'fullhd':
      return 'Transmissor Full HD'
  }
}

/** Desvincular só com 2 ou mais identidades. */
export function podeDesvincular(identidades: ContaIdentidade[]): boolean {
  return identidades.length >= 2
}

/** O servidor oferece login? (`account_provider` presente e provedor 'supabase'). */
export function temLogin(conta: AccountView | undefined): boolean {
  const provedor = conta?.account_provider
  if (!provedor) return false
  return provedor.provider === 'supabase' && provedor.url !== '' && provedor.key !== ''
}

/** Provedor do botão -> provider do supabase-js ('azure' para Microsoft). */
export function provedorOAuth(id: 'google' | 'microsoft'): ProvedorOAuth {
  return id === 'google' ? 'google' : 'azure'
}
