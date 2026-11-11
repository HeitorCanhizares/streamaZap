// Cliente falso da conta para `npm run dev` sem Tauri (sem rede). Tarefa front-06.
import type { ContaClient, ContaEvento, ContaIdentidade, ContaSessao } from './contaTipos'

type Assinante = (evento: ContaEvento, sessao: ContaSessao | null) => void

/**
 * `ContaClient` em memória. Regras: `onAuthStateChange` entrega logo `INITIAL_SESSION` (com a
 * sessão atual ou null) e depois os eventos; `signInWithOAuth` e `exchangeCodeForSession`
 * entram direto (SIGNED_IN, e-mail 'voce@exemplo.com', token 'mock-token', identidade do provedor,
 * e uma URL https de mentira, que `api.abrirUrl` ignora fora do Tauri); `signInWithOtp` sempre ok; `verifyOtp` aceita só o token '123456' (senão o erro
 * `otp_expired`, como o Supabase) e entra com o e-mail dado (identidade 'email'); `linkIdentity` acrescenta a
 * identidade e emite SIGNED_IN; `unlinkIdentity` remove; `signOut` dispara SIGNED_OUT; (a assinatura fica no
 * 'banco'); `refreshSession` gera 'mock-token-N' e emite TOKEN_REFRESHED; `from('config_publica')` e
 * `from('assinaturas')` leem das opções; `functions.invoke('assinar')` valida como o servidor e grava a
 * assinatura na hora; os outros nomes devolvem ok.
 */
/** URL do "login" de mentira: https para passar na checagem de quem chama; nunca é aberta. */
const URL_MOCK = 'https://mock.supabase.co/auth/v1/authorize'

export const PRECOS_MOCK = {
  espectador: { mensal: 390, anual: 3900 },
  hd: { mensal: 790, anual: 7900 },
  fullhd: { mensal: 1490, anual: 14900 },
}
export interface MockContaOpcoes {
  /** Padrão: `?cobranca=1` na URL (navegador), senão false. */
  cobrancaAtiva?: boolean
  /** Linha inicial de `assinaturas` (formato do banco). */
  assinatura?: { plano: string; status: string; ciclo: string; valido_ate: string } | null
  /** `from('config_publica')` devolve erro. */
  falharConfig?: boolean
}

export function criarMockConta(opcoes: MockContaOpcoes = {}): ContaClient {
  let sessao: ContaSessao | null = null
  const cobranca =
    opcoes.cobrancaAtiva ??
    (typeof globalThis.location !== 'undefined' &&
      new URLSearchParams(globalThis.location.search).get('cobranca') === '1')
  let assinatura: MockContaOpcoes['assinatura'] = opcoes.assinatura ?? null
  let contadorToken = 1
  let identidades: ContaIdentidade[] = []
  const assinantes = new Set<Assinante>()

  const emitir = (evento: ContaEvento, atual: ContaSessao | null) => {
    for (const cb of [...assinantes]) cb(evento, atual)
  }

  const entrar = (email: string, provider: string) => {
    sessao = { access_token: 'mock-token', user: { email } }
    if (identidades.length === 0) {
      identidades = [{ identity_id: `mock-${provider}`, provider, email }]
    }
    emitir('SIGNED_IN', sessao)
  }

  return {
    auth: {
      async signInWithOAuth({ provider }) {
        entrar('voce@exemplo.com', provider)
        return { data: { url: URL_MOCK }, error: null }
      },
      async signInWithOtp() {
        return { error: null }
      },
      async verifyOtp({ email, token }) {
        if (token === '123456') {
          entrar(email, 'email')
          return { error: null }
        }
        // Como o Supabase: mensagem em inglês e o código que o app traduz.
        return { error: { message: 'Token has expired or is invalid', code: 'otp_expired' } }
      },
      async exchangeCodeForSession() {
        entrar('voce@exemplo.com', 'google')
        return { error: null }
      },
      async getUserIdentities() {
        const lista = sessao ? identidades.map((i) => ({ ...i })) : []
        return { data: { identities: lista }, error: null }
      },
      async linkIdentity({ provider }) {
        const id = `mock-${provider}`
        if (!identidades.some((i) => i.identity_id === id)) {
          identidades = [...identidades, { identity_id: id, provider, email: sessao?.user?.email }]
        }
        // Como no real: a volta do navegador troca o code pela sessão e emite SIGNED_IN.
        if (sessao) emitir('SIGNED_IN', sessao)
        return { data: { url: URL_MOCK }, error: null }
      },
      async unlinkIdentity(identity) {
        identidades = identidades.filter((i) => i.identity_id !== identity.identity_id)
        return { error: null }
      },
      async signOut() {
        sessao = null
        identidades = []
        emitir('SIGNED_OUT', null)
        return { error: null }
      },
      async refreshSession() {
        if (!sessao) return { data: { session: null }, error: null }
        contadorToken += 1
        sessao = { ...sessao, access_token: `mock-token-${contadorToken}` }
        emitir('TOKEN_REFRESHED', sessao)
        return { data: { session: sessao }, error: null }
      },
      async stopAutoRefresh() {},
      onAuthStateChange(cb) {
        assinantes.add(cb)
        queueMicrotask(() => {
          if (assinantes.has(cb)) cb('INITIAL_SESSION', sessao)
        })
        return {
          data: { subscription: { unsubscribe: () => void assinantes.delete(cb) } },
        }
      },
    },
    from(tabela) {
      return {
        select: () => ({
          maybeSingle: async () => {
            if (tabela === 'config_publica') {
              if (opcoes.falharConfig) {
                return { data: null, error: { message: 'mock: config indisponível' } }
              }
              return {
                data: { cobranca_ativa: cobranca, plano_padrao: 'fullhd', precos: PRECOS_MOCK },
                error: null,
              }
            }
            return { data: sessao && assinatura ? { ...assinatura } : null, error: null }
          },
        }),
      }
    },
    functions: {
      async invoke(nome, opcoes2) {
        if (nome !== 'assinar') return { error: null }
        const falha = (codigo: string, mensagem: string) => ({
          error: { message: mensagem, context: { json: async () => ({ erro: codigo, mensagem }) } },
        })
        const body = (opcoes2?.body ?? {}) as { plano?: string; ciclo?: string; cpf_cnpj?: string }
        if (!sessao) return falha('nao_autenticado', 'Entre na sua conta.')
        if (!cobranca) return falha('cobranca_desligada', 'As assinaturas ainda não estão abertas.')
        const { plano, ciclo, cpf_cnpj: doc } = body
        if (
          !plano ||
          !ciclo ||
          !['espectador', 'hd', 'fullhd'].includes(plano) ||
          !['mensal', 'anual'].includes(ciclo)
        ) {
          return falha('invalido', 'Pedido inválido.')
        }
        if (doc !== undefined && doc.length !== 11 && doc.length !== 14) {
          return falha('invalido', 'Informe um CPF ou CNPJ válido.')
        }
        if (assinatura?.status === 'ativa' && assinatura.plano === plano) {
          return falha('ja_assinante', 'Você já tem esse plano.')
        }
        const dias = ciclo === 'anual' ? 365 : 30
        // Como se o pagamento fosse na hora.
        assinatura = {
          plano,
          status: 'ativa',
          ciclo,
          valido_ate: new Date(Date.now() + dias * 86_400_000).toISOString(),
        }
        return { data: { link: `https://sandbox.asaas.com/i/mock-${plano}-${ciclo}` }, error: null }
      },
    },
  }
}
