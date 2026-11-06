// Ações de entrada na conta (plano §7). Tarefa front-08. Usam `clienteAtual()` e o `contaStore`;
// sem cliente: aviso 'Login indisponível neste servidor.' e nada mais. Falha vira `aviso` em
// português (nunca lança); `ocupado` fica true durante a chamada.
import { api } from './api'
import { clienteAtual } from './contaClient'
import { getContaEstado, setContaEstado } from './contaStore'
import { interpretarRetorno } from './deepLink'
import { traduzirErro } from './contaErros'
import type { ContaClient, ProvedorOAuth } from './contaTipos'

export const REDIRECT = 'streamazap://auth/callback'

/** Roda `fn` com o cliente atual, marcando `ocupado`; nunca lança. */
async function comCliente(fn: (cliente: ContaClient) => Promise<void>): Promise<void> {
  const cliente = clienteAtual()
  if (!cliente) {
    setContaEstado({ aviso: 'Login indisponível neste servidor.' })
    return
  }
  setContaEstado({ ocupado: true, aviso: null })
  try {
    await fn(cliente)
  } catch (e) {
    setContaEstado({ aviso: traduzirErro(e, 'Não foi possível entrar.') })
  } finally {
    setContaEstado({ ocupado: false })
  }
}

/**
 * Pede a URL do OAuth com `signInWithOAuth({ provider, options: { redirectTo: REDIRECT,
 * skipBrowserRedirect: true, scopes: 'email' só para 'azure' } })` e abre com `api.abrirUrl`.
 * Só aceita URL `https://`. A volta chega por `tratarDeepLink`.
 */
export async function entrarOAuth(provedor: ProvedorOAuth): Promise<void> {
  await comCliente(async (cliente) => {
    const { data, error } = await cliente.auth.signInWithOAuth({
      provider: provedor,
      options: {
        redirectTo: REDIRECT,
        skipBrowserRedirect: true,
        ...(provedor === 'azure' ? { scopes: 'email' } : {}),
      },
    })
    if (error) {
      setContaEstado({ aviso: traduzirErro(error, 'Não foi possível abrir o login.') })
      return
    }
    const url = data?.url
    if (typeof url !== 'string' || !url.startsWith('https://')) {
      setContaEstado({ aviso: 'Não foi possível abrir o login.' })
      return
    }
    await api.abrirUrl(url)
  })
}

/** `signInWithOtp({ email })` (email com trim, vazio = aviso); sucesso: `otpEmail = email`. */
export async function entrarEmail(email: string): Promise<void> {
  const limpo = email.trim()
  if (limpo === '') {
    setContaEstado({ aviso: 'Digite seu e-mail.' })
    return
  }
  await comCliente(async (cliente) => {
    const { error } = await cliente.auth.signInWithOtp({ email: limpo })
    if (error) setContaEstado({ aviso: traduzirErro(error, 'Não foi possível enviar o código.') })
    else setContaEstado({ otpEmail: limpo })
  })
}

/** `verifyOtp({ email: otpEmail, token, type: 'email' })`; token = só dígitos, 6 (senão aviso); sucesso zera `otpEmail`. */
export async function confirmarCodigo(token: string): Promise<void> {
  const digitos = token.replace(/\D/g, '')
  const otpEmail = getContaEstado().otpEmail
  if (otpEmail === null || digitos.length !== 6) {
    setContaEstado({ aviso: 'Digite o código de 6 dígitos.' })
    return
  }
  await comCliente(async (cliente) => {
    const { error } = await cliente.auth.verifyOtp({ email: otpEmail, token: digitos, type: 'email' })
    if (error) setContaEstado({ aviso: traduzirErro(error, 'Código errado ou expirado.') })
    else setContaEstado({ otpEmail: null })
  })
}

/** Fecha o campo do código (`otpEmail = null`, sem aviso). */
export function cancelarCodigo(): void {
  setContaEstado({ otpEmail: null, aviso: null })
}

/** `signOut()`; o SIGNED_OUT chega por `onAuthStateChange` (contaSync). */
export async function sair(): Promise<void> {
  await comCliente(async (cliente) => {
    const { error } = await cliente.auth.signOut()
    if (error) setContaEstado({ aviso: traduzirErro(error, 'Não foi possível sair da conta.') })
  })
}

/**
 * Volta do navegador: `interpretarRetorno(url)`; `code` -> `exchangeCodeForSession(code)`;
 * `erro` -> `aviso = mensagem`; `ignorar` -> nada.
 */
export async function tratarDeepLink(url: string): Promise<void> {
  const r = interpretarRetorno(url)
  if (r.tipo === 'erro') {
    setContaEstado({ aviso: r.mensagem })
    return
  }
  if (r.tipo !== 'code') return
  await comCliente(async (cliente) => {
    const { error } = await cliente.auth.exchangeCodeForSession(r.code)
    if (error) setContaEstado({ aviso: traduzirErro(error, 'Não foi possível concluir o login. Tente de novo.') })
  })
}
