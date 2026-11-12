// Quais provedores de login o projeto do Supabase tem ligados. Um botão de provedor desligado
// (o Microsoft, enquanto não for configurado) levaria a uma página de erro 400 no navegador.
import type { ProvedorOAuth } from './contaTipos'
import { provedoresLigados } from './contaModel'

/**
 * `GET <url>/auth/v1/settings` com o cabeçalho `apikey` (a chave pública do `conta` do servidor) e
 * `provedoresLigados` da resposta. Qualquer falha (rede, HTTP fora de 2xx, JSON inesperado): null.
 */
export async function lerProvedores(
  url: string,
  key: string,
  buscar: typeof fetch = fetch,
): Promise<ProvedorOAuth[] | null> {
  try {
    const resposta = await buscar(`${url.replace(/\/+$/, '')}/auth/v1/settings`, { headers: { apikey: key } })
    if (!resposta.ok) return null
    return provedoresLigados(await resposta.json())
  } catch {
    return null
  }
}
