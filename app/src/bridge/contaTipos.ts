// Tipos mínimos do cliente do Supabase usados pelo app (plano contas-planos-camera §7).
// O cliente real (`SupabaseClient`) é convertido para `ContaClient` num só lugar (contaClient.ts);
// os testes e o mock passam objetos que cumprem só esta interface.
export type ProvedorOAuth = 'google' | 'azure'

/**
 * Erro do supabase-js. `message` vem em inglês e não vai para a tela (ver contaErros.ts); `code` é o
 * código do Supabase Auth, `name` a classe do erro e `context` a resposta de `functions.invoke`.
 */
export interface ContaErro { message: string; code?: string; name?: string; context?: unknown }
export interface ContaIdentidade { identity_id: string; provider: string; email?: string }
export interface ContaSessao { access_token: string; user?: { email?: string } }
/** INITIAL_SESSION, SIGNED_IN, TOKEN_REFRESHED, SIGNED_OUT (outros eventos são ignorados). */
export type ContaEvento = string

export interface ContaResposta<T> { data: T | null; error: ContaErro | null }
export interface ContaConsulta { select(colunas: string): { maybeSingle(): PromiseLike<ContaResposta<unknown>> } }

export interface ContaClient {
  auth: {
    signInWithOAuth(args: {
      provider: ProvedorOAuth
      options: { redirectTo: string; skipBrowserRedirect: boolean; scopes?: string }
    }): Promise<{ data: { url: string | null }; error: ContaErro | null }>
    signInWithOtp(args: { email: string }): Promise<{ error: ContaErro | null }>
    verifyOtp(args: { email: string; token: string; type: 'email' }): Promise<{ error: ContaErro | null }>
    exchangeCodeForSession(code: string): Promise<{ error: ContaErro | null }>
    getUserIdentities(): Promise<{ data: { identities: ContaIdentidade[] } | null; error: ContaErro | null }>
    linkIdentity(args: {
      provider: ProvedorOAuth
      options: { redirectTo: string; skipBrowserRedirect: boolean; scopes?: string }
    }): Promise<{ data: { url: string | null }; error: ContaErro | null }>
    unlinkIdentity(identity: ContaIdentidade): Promise<{ error: ContaErro | null }>
    signOut(): Promise<{ error: ContaErro | null }>
    refreshSession(): Promise<{ data: { session: ContaSessao | null } | null; error: ContaErro | null }>
    /** Para os timers de renovação (o cliente real tem; o mock pode omitir). */
    stopAutoRefresh?(): Promise<void>
    onAuthStateChange(cb: (evento: ContaEvento, sessao: ContaSessao | null) => void): {
      data: { subscription: { unsubscribe(): void } }
    }
  }
  /** Só o que o app usa do PostgREST: `from(t).select(cols).maybeSingle()`. */
  from(tabela: 'config_publica' | 'assinaturas'): ContaConsulta
  functions: { invoke(nome: string, opcoes?: { body?: unknown }): Promise<{ data?: unknown; error: ContaErro | null }> }
}

/** Estado da conta para a interface (contaStore.ts). */
export interface ContaEstado {
  /** 'carregando' até o INITIAL_SESSION; 'dentro' com sessão; 'fora' sem. */
  sessao: 'carregando' | 'fora' | 'dentro'
  email: string | null
  identidades: ContaIdentidade[]
  /** Mensagem de erro/aviso em português (null = nada a mostrar). */
  aviso: string | null
  /** E-mail para o qual o código de 6 dígitos foi pedido (null = campo do código escondido). */
  otpEmail: string | null
  ocupado: boolean
}
