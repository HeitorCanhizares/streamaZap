// Erros do Supabase em português (plano contas-planos-camera §7). O supabase-js devolve
// `message` em inglês; aqui só os códigos conhecidos viram texto próprio, o resto vira a
// mensagem padrão de quem chamou.

const SEM_CONEXAO = 'Sem conexão com o servidor de login. Confira a internet e tente de novo.'

/** Códigos do Supabase Auth (`error.code`) com texto próprio. */
const POR_CODIGO: Record<string, string> = {
  otp_expired: 'O código expirou ou está errado. Peça um código novo.',
  over_email_send_rate_limit: 'Muitos e-mails pedidos. Espere um pouco e tente de novo.',
  over_request_rate_limit: 'Muitas tentativas. Espere um pouco e tente de novo.',
  email_address_invalid: 'E-mail inválido.',
  validation_failed: 'Confira o e-mail digitado.',
  identity_already_exists: 'Essa conta já está ligada a outro usuário.',
  single_identity_not_deletable: 'Mantenha ao menos uma forma de entrar.',
  bad_code_verifier: 'O login expirou. Tente entrar de novo.',
  flow_state_not_found: 'O login expirou. Tente entrar de novo.',
  flow_state_expired: 'O login expirou. Tente entrar de novo.',
  manual_linking_disabled: 'Este servidor não permite vincular contas.',
  provider_disabled: 'Este jeito de entrar está desligado no servidor.',
  email_provider_disabled: 'O login por e-mail está desligado no servidor.',
  signup_disabled: 'Este servidor não aceita contas novas.',
  otp_disabled: 'O login por e-mail está desligado no servidor.',
  user_banned: 'Esta conta está bloqueada.',
}

/** Nomes das classes de erro de rede do supabase-js (auth e functions). */
const ERROS_DE_REDE = new Set(['AuthRetryableFetchError', 'FunctionsFetchError', 'FunctionsRelayError'])

/**
 * Texto em português para um erro do supabase-js (ou exceção). Código conhecido → texto próprio;
 * falha de rede → aviso de conexão; o resto → `padrao`. Nunca devolve a `message` em inglês.
 */
export function traduzirErro(erro: unknown, padrao: string): string {
  if (erro && typeof erro === 'object') {
    const { code, name } = erro as { code?: unknown; name?: unknown }
    if (typeof code === 'string' && Object.prototype.hasOwnProperty.call(POR_CODIGO, code)) {
      return POR_CODIGO[code]
    }
    if (typeof name === 'string' && ERROS_DE_REDE.has(name)) return SEM_CONEXAO
  }
  return padrao
}

/**
 * Erro de `functions.invoke`: a função do servidor responde `{"erro","mensagem"}` (mensagem já em
 * português) e o supabase-js põe a resposta em `error.context`. Usa essa `mensagem`; sem ela,
 * cai em `traduzirErro`.
 */
export async function mensagemDaFuncao(erro: unknown, padrao: string): Promise<string> {
  const contexto = erro && typeof erro === 'object' ? (erro as { context?: unknown }).context : undefined
  if (contexto && typeof (contexto as { json?: unknown }).json === 'function') {
    try {
      const corpo: unknown = await (contexto as { json(): Promise<unknown> }).json()
      const mensagem = corpo && typeof corpo === 'object' ? (corpo as { mensagem?: unknown }).mensagem : undefined
      if (typeof mensagem === 'string' && mensagem.trim() !== '') return mensagem.trim()
    } catch {
      // Corpo que não é JSON: fica com a mensagem padrão.
    }
  }
  return traduzirErro(erro, padrao)
}
