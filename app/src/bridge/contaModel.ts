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

/** Provedores OAuth que o app sabe usar, na ordem dos botões. */
export const PROVEDORES_OAUTH: readonly ProvedorOAuth[] = ['google', 'azure']

/**
 * Provedores OAuth ligados no projeto, lidos do JSON de `GET /auth/v1/settings` do Supabase
 * (`external.google`, `external.azure` com `true`), na ordem de `PROVEDORES_OAUTH`. Formato
 * inesperado: null.
 */
export function provedoresLigados(json: unknown): ProvedorOAuth[] | null {
  if (typeof json !== 'object' || json === null) return null
  const external = (json as { external?: unknown }).external
  if (typeof external !== 'object' || external === null) return null
  const ligados = external as Record<string, unknown>
  return PROVEDORES_OAUTH.filter((p) => ligados[p] === true)
}

/** Provedor do botão -> provider do supabase-js ('azure' para Microsoft). */
export function provedorOAuth(id: 'google' | 'microsoft'): ProvedorOAuth {
  return id === 'google' ? 'google' : 'azure'
}
