import { describe, expect, it } from 'vitest'
import { mensagemDaFuncao, traduzirErro } from './contaErros'

describe('traduzirErro', () => {
  it('código conhecido vira texto próprio', () => {
    expect(traduzirErro({ message: 'Email rate limit exceeded', code: 'over_email_send_rate_limit' }, 'p')).toBe(
      'Muitos e-mails pedidos. Espere um pouco e tente de novo.',
    )
  })

  it('erro de rede vira aviso de conexão', () => {
    expect(traduzirErro({ message: 'Failed to fetch', name: 'AuthRetryableFetchError' }, 'p')).toMatch(/Sem conexão/)
  })

  it('o resto usa a mensagem padrão, nunca a message em inglês', () => {
    expect(traduzirErro({ message: 'Something broke', code: 'desconhecido' }, 'padrão')).toBe('padrão')
    expect(traduzirErro(new Error('boom'), 'padrão')).toBe('padrão')
    expect(traduzirErro(null, 'padrão')).toBe('padrão')
    expect(traduzirErro({ code: 'toString' }, 'padrão')).toBe('padrão')
  })
})

describe('mensagemDaFuncao', () => {
  it('usa a mensagem do corpo {erro, mensagem}', async () => {
    const context = new Response(JSON.stringify({ erro: 'nao_autorizado', mensagem: 'Entre de novo.' }), { status: 401 })
    expect(await mensagemDaFuncao({ message: 'x', context }, 'p')).toBe('Entre de novo.')
  })

  it('corpo que não é JSON ou sem mensagem: padrão', async () => {
    expect(await mensagemDaFuncao({ message: 'x', context: new Response('oops', { status: 500 }) }, 'p')).toBe('p')
    expect(await mensagemDaFuncao({ message: 'x', context: new Response('{}', { status: 500 }) }, 'p')).toBe('p')
    expect(await mensagemDaFuncao({ message: 'x' }, 'p')).toBe('p')
  })
})
