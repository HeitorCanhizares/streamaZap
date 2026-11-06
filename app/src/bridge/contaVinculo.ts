// Contas vinculadas e exclusão (plano §7). Tarefa front-09. Mesmas regras de contaAcoes.ts
// (clienteAtual, aviso em português, nunca lança, `ocupado`).
import { api } from './api'
import { REDIRECT, sair } from './contaAcoes'
import { clienteAtual } from './contaClient'
import { mensagemDaFuncao, traduzirErro } from './contaErros'
import { podeDesvincular } from './contaModel'
import { getContaEstado, setContaEstado } from './contaStore'
import type { ContaClient, ContaIdentidade, ProvedorOAuth } from './contaTipos'

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
    setContaEstado({ aviso: traduzirErro(e, 'Não foi possível concluir.') })
  } finally {
    setContaEstado({ ocupado: false })
  }
}

/** `getUserIdentities()` -> `identidades` no store (erro: aviso, lista intacta). Não liga `ocupado`. */
export async function carregarIdentidades(): Promise<void> {
  const cliente = clienteAtual()
  if (!cliente) {
    setContaEstado({ aviso: 'Login indisponível neste servidor.' })
    return
  }
  try {
    const { data, error } = await cliente.auth.getUserIdentities()
    if (error || !data) {
      setContaEstado({ aviso: traduzirErro(error, 'Não foi possível carregar as contas vinculadas.') })
      return
    }
    setContaEstado({ identidades: data.identities })
  } catch (e) {
    setContaEstado({ aviso: traduzirErro(e, 'Não foi possível carregar as contas vinculadas.') })
  }
}

/** `linkIdentity({ provider, options: { redirectTo: REDIRECT, skipBrowserRedirect: true } })` e abre a URL https por `api.abrirUrl`; a volta é `tratarDeepLink`. */
export async function vincular(provedor: ProvedorOAuth): Promise<void> {
  await comCliente(async (cliente) => {
    const { data, error } = await cliente.auth.linkIdentity({
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

/** `unlinkIdentity(identidade)` só se `podeDesvincular(identidades)`; senão aviso 'Mantenha ao menos uma forma de entrar.'; depois `carregarIdentidades()`. */
export async function desvincular(identidade: ContaIdentidade): Promise<void> {
  if (!podeDesvincular(getContaEstado().identidades)) {
    setContaEstado({ aviso: 'Mantenha ao menos uma forma de entrar.' })
    return
  }
  let ok = false
  await comCliente(async (cliente) => {
    const { error } = await cliente.auth.unlinkIdentity(identidade)
    if (error) setContaEstado({ aviso: traduzirErro(error, 'Não foi possível desvincular.') })
    else ok = true
  })
  if (ok) await carregarIdentidades()
}

/**
 * `functions.invoke('excluir-conta')`; só se der certo chama `sair()`. Falha: a `mensagem` que o
 * servidor devolveu (`{"erro","mensagem"}`) ou 'Não foi possível excluir a conta.'.
 */
export async function excluirConta(): Promise<void> {
  const cliente = clienteAtual()
  if (!cliente) {
    setContaEstado({ aviso: 'Login indisponível neste servidor.' })
    return
  }
  let ok = false
  setContaEstado({ ocupado: true, aviso: null })
  try {
    const { error } = await cliente.functions.invoke('excluir-conta')
    if (error) setContaEstado({ aviso: await mensagemDaFuncao(error, 'Não foi possível excluir a conta.') })
    else ok = true
  } catch {
    setContaEstado({ aviso: 'Não foi possível excluir a conta.' })
  } finally {
    setContaEstado({ ocupado: false })
  }
  if (ok) await sair()
}
