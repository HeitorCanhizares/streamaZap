// Leitura da URL de retorno do login (`streamazap://auth/callback?code=...`). Tarefa front-04.

/** O que a URL do deep link quer dizer. */
export type RetornoLogin =
  | { tipo: 'code'; code: string }
  | { tipo: 'erro'; mensagem: string }
  | { tipo: 'ignorar' }

import { traduzirErro } from './contaErros'

const PREFIXO = 'streamazap://auth/callback'

/** Junta os parâmetros de uma query/fragmento já sem o `?`/`#`. Nunca lança. */
function lerParametros(texto: string): Map<string, string> {
  const mapa = new Map<string, string>()
  if (!texto) return mapa
  try {
    new URLSearchParams(texto).forEach((valor, chave) => {
      if (!mapa.has(chave)) mapa.set(chave, valor)
    })
  } catch {
    // Texto ilegível: sem parâmetros.
  }
  return mapa
}

/**
 * `?code=X` -> code. `error`/`error_description` (na query ou no `#fragmento`) -> erro com
 * mensagem em português (código conhecido do Supabase ou a padrão; 'access_denied' -> 'Login cancelado.').
 * URL que não começa com `streamazap://auth/callback`, sem code nem erro, ou inválida -> ignorar.
 */
export function interpretarRetorno(url: string): RetornoLogin {
  const ignorar: RetornoLogin = { tipo: 'ignorar' }
  if (typeof url !== 'string') return ignorar

  const entrada = url.trim()
  // Esquema e host não diferenciam maiúsculas; o resto da string é preservado.
  if (entrada.slice(0, PREFIXO.length).toLowerCase() !== PREFIXO) return ignorar
  const resto = entrada.slice(PREFIXO.length)
  // Depois do prefixo só pode vir fim, '?', '#' ou '/'.
  if (resto !== '' && !'?#/'.includes(resto[0])) return ignorar

  // O fragmento começa no primeiro '#'; a query, no primeiro '?' antes dele.
  const posFragmento = resto.indexOf('#')
  const antesFragmento = posFragmento >= 0 ? resto.slice(0, posFragmento) : resto
  const fragmento = posFragmento >= 0 ? resto.slice(posFragmento + 1) : ''
  const posQuery = antesFragmento.indexOf('?')
  const query = posQuery >= 0 ? antesFragmento.slice(posQuery + 1) : ''

  // Em conflito, a query vence o fragmento.
  const params = lerParametros(fragmento)
  lerParametros(query).forEach((valor, chave) => params.set(chave, valor))

  const erro = params.get('error') ?? ''
  const descricao = params.get('error_description') ?? ''
  if (erro !== '' || descricao !== '') {
    if (erro === 'access_denied' || params.get('error_code') === 'access_denied') {
      return { tipo: 'erro', mensagem: 'Login cancelado.' }
    }
    // A descrição do Supabase vem em inglês: só o código conhecido vira texto próprio.
    return { tipo: 'erro', mensagem: traduzirErro({ code: params.get('error_code') ?? erro }, 'Não foi possível entrar.') }
  }

  const code = params.get('code') ?? ''
  if (code !== '') return { tipo: 'code', code }
  return ignorar
}
