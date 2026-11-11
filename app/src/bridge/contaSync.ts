// Liga o login ao app: cria o cliente, repassa o token ao núcleo e ouve o deep link. Tarefa front-10.
import { useEffect } from 'react'
import type { AccountProviderView } from './types'
import { api } from './api'
import { obterCliente, descartarCliente } from './contaClient'
import { setContaEstado, resetConta } from './contaStore'
import { tratarDeepLink } from './contaAcoes'
import { carregarIdentidades } from './contaVinculo'
import { iniciarPlanos, type PlanosSync } from './planoSync'
import { resetPlano } from './planoStore'
import { useSnapshot } from './store'
import { useOnlineEnabled } from './onlineStore'

/**
 * Sem `conta` (undefined ou `provider !== 'supabase'`) ou com `ligado === false`: não faz nada
 * (nem `import()` do supabase) e devolve uma função vazia. Senão: `obterCliente(conta)`,
 * `auth.onAuthStateChange`:
 *  - INITIAL_SESSION / SIGNED_IN / TOKEN_REFRESHED com sessão: `setContaEstado({ sessao:'dentro',
 *    email })`, `api.onlineSetAccount(access_token)`, e (SIGNED_IN/INITIAL_SESSION) `carregarIdentidades()`;
 *  - INITIAL_SESSION sem sessão ou SIGNED_OUT: `{ sessao:'fora', email:null, identidades:[],
 *    otpEmail:null }` e `api.onlineSetAccount(null)`;
 *  - outros eventos: ignora.
 * Também `api.escutarDeepLink(tratarDeepLink)` e, depois, os callbacks que chegaram antes
 * (`api.deepLinkPendentes()`, o app aberto do zero pela volta do login). A função devolvida cancela os dois (mesmo que
 * ainda estejam subindo), chama `descartarCliente()` e `resetConta()`. Falha ao criar o cliente:
 * `aviso` em português. `api.onlineSetAccount` que rejeita é engolido.
 */
export function iniciarConta(conta: AccountProviderView | undefined, ligado: boolean): () => void {
  if (!conta || conta.provider !== 'supabase' || !ligado) return () => undefined

  let cancelado = false
  let unsubAuth: (() => void) | null = null
  let unsubLink: (() => void) | null = null
  let planos: PlanosSync | null = null

  // Repassar o token ao núcleo é best-effort: rejeição é engolida.
  const enviarToken = (token: string | null): void => {
    void api.onlineSetAccount(token).catch(() => undefined)
  }

  void (async () => {
    try {
      const cliente = await obterCliente(conta)
      if (cancelado) return
      planos = iniciarPlanos()
      const { data } = cliente.auth.onAuthStateChange((evento, sessao) => {
        if (cancelado) return
        if (evento === 'INITIAL_SESSION' || evento === 'SIGNED_IN' || evento === 'TOKEN_REFRESHED') {
          if (sessao) {
            const email = sessao.user?.email ?? null
            setContaEstado(
              evento === 'SIGNED_IN' ? { sessao: 'dentro', email, aviso: null } : { sessao: 'dentro', email },
            )
            enviarToken(sessao.access_token)
            if (evento !== 'TOKEN_REFRESHED') {
              // Fora do callback: chamar o auth lá dentro trava o supabase-js (deadlock).
              setTimeout(() => {
                if (cancelado) return
                void carregarIdentidades()
                planos?.aoEntrar()
              }, 0)
            }
          } else if (evento === 'INITIAL_SESSION') {
            setContaEstado({ sessao: 'fora', email: null, identidades: [], otpEmail: null })
            planos?.aoSair()
            enviarToken(null)
          }
          return
        }
        if (evento === 'SIGNED_OUT') {
          setContaEstado({ sessao: 'fora', email: null, identidades: [], otpEmail: null })
          planos?.aoSair()
          enviarToken(null)
        }
      })
      unsubAuth = () => data.subscription.unsubscribe()
      const unlisten = await api.escutarDeepLink((u) => void tratarDeepLink(u))
      if (cancelado) {
        unlisten()
        return
      }
      unsubLink = unlisten
      // O app pode ter sido aberto do zero pela volta do login: a URL ficou esperando.
      for (const url of await api.deepLinkPendentes().catch(() => [] as string[])) {
        if (cancelado) return
        void tratarDeepLink(url)
      }
    } catch {
      if (!cancelado) setContaEstado({ sessao: 'fora', aviso: 'Não foi possível iniciar o login.' })
    }
  })()

  return () => {
    cancelado = true
    unsubAuth?.()
    unsubLink?.()
    planos?.parar()
    void descartarCliente()
    resetConta()
    resetPlano()
  }
}

/** Hook para o App: `iniciarConta(snap.account?.account_provider, useOnlineEnabled())` num efeito que refaz quando `url`/`key`/`provider`/`ligado` mudam. */
export function useContaSync(): void {
  const conta = useSnapshot().account?.account_provider
  const ligado = useOnlineEnabled()
  // Dependências por campo: o objeto `conta` muda a cada snapshot.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => iniciarConta(conta, ligado), [conta?.provider, conta?.url, conta?.key, ligado])
}
