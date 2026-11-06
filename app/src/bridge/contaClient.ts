// Cliente do Supabase criado sob demanda a partir do `conta` do servidor (plano §7). Tarefa front-05.
// `@supabase/supabase-js` só entra por `import()` dinâmico, e só aqui. Fora do Tauri devolve o
// mock (mockConta.ts), sem rede.
import type { AccountProviderView } from './types'
import type { ContaClient } from './contaTipos'
import { isTauri } from './api'
import { authStorage } from './authStorage'
import { criarMockConta } from './mockConta'

/**
 * Devolve o cliente para `conta` (`provider === 'supabase'`; outro provedor rejeita com Error em
 * português). Mesmo `url` e `key` reaproveita o cliente; mudou, descarta o antigo e cria outro.
 * Dentro do Tauri: `createClient(url, key, { auth: { flowType: 'pkce', detectSessionInUrl: false,
 * persistSession: true, autoRefreshToken: true, storage: authStorage } })`, convertido para
 * `ContaClient` aqui (único `as unknown as`).
 */
interface Estado { url: string; key: string; client: ContaClient }
interface EmAndamento { url: string; key: string; promessa: Promise<ContaClient> }

let atual: Estado | null = null
let emAndamento: EmAndamento | null = null
/** Muda a cada descarte: criação que terminar depois dele não vira o cliente atual. */
let geracao = 0

async function criar(url: string, key: string): Promise<ContaClient> {
  if (!isTauri()) return criarMockConta()
  const { createClient } = await import('@supabase/supabase-js')
  const client = createClient(url, key, {
    auth: {
      flowType: 'pkce',
      detectSessionInUrl: false,
      persistSession: true,
      autoRefreshToken: true,
      storage: authStorage,
    },
  })
  // O tipo do supabase-js é bem maior que o `ContaClient` (só o que usamos); uma conversão só.
  return client as unknown as ContaClient
}

export async function obterCliente(conta: AccountProviderView): Promise<ContaClient> {
  if (conta.provider !== 'supabase') throw new Error('Provedor de login não suportado.')
  const { url, key } = conta
  if (atual && atual.url === url && atual.key === key) return atual.client
  if (emAndamento && emAndamento.url === url && emAndamento.key === key) return emAndamento.promessa

  const promessa = (async () => {
    await descartarCliente()
    const minha = geracao
    const client = await criar(url, key)
    if (minha === geracao) {
      atual = { url, key, client }
      if (emAndamento?.url === url && emAndamento.key === key) emAndamento = null
    }
    return client
  })()
  emAndamento = { url, key, promessa }
  // Falha na criação não deve deixar a promessa ruim guardada.
  promessa.catch(() => {
    if (emAndamento?.promessa === promessa) emAndamento = null
  })
  return promessa
}

/** Último cliente criado por `obterCliente` (null = nenhum ou descartado). */
export function clienteAtual(): ContaClient | null {
  return atual ? atual.client : null
}

/** Esquece o cliente e chama `auth.stopAutoRefresh?.()` (engole erro). Seguro sem cliente. */
export async function descartarCliente(): Promise<void> {
  const antigo = atual
  atual = null
  geracao += 1
  emAndamento = null
  if (!antigo) return
  try {
    await antigo.client.auth.stopAutoRefresh?.()
  } catch {
    // Parar a renovação é só limpeza: erro aqui não interessa a ninguém.
  }
}
