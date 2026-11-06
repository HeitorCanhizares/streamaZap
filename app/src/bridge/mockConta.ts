// Cliente falso da conta para `npm run dev` sem Tauri (sem rede). Tarefa front-06.
import type { ContaClient, ContaEvento, ContaIdentidade, ContaSessao } from './contaTipos'

type Assinante = (evento: ContaEvento, sessao: ContaSessao | null) => void

/**
 * `ContaClient` em memória. Regras: `onAuthStateChange` entrega logo `INITIAL_SESSION` (com a
 * sessão atual ou null) e depois os eventos; `signInWithOAuth` e `exchangeCodeForSession`
 * entram direto (SIGNED_IN, e-mail 'voce@exemplo.com', token 'mock-token', identidade do provedor,
 * e uma URL https de mentira, que `api.abrirUrl` ignora fora do Tauri); `signInWithOtp` sempre ok; `verifyOtp` aceita só o token '123456' (senão o erro
 * `otp_expired`, como o Supabase) e entra com o e-mail dado (identidade 'email'); `linkIdentity` acrescenta a
 * identidade e emite SIGNED_IN; `unlinkIdentity` remove; `signOut` dispara SIGNED_OUT; `functions.invoke` ok.
 */
/** URL do "login" de mentira: https para passar na checagem de quem chama; nunca é aberta. */
const URL_MOCK = 'https://mock.supabase.co/auth/v1/authorize'

export function criarMockConta(): ContaClient {
  let sessao: ContaSessao | null = null
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
    functions: {
      async invoke() {
        return { error: null }
      },
    },
  }
}
