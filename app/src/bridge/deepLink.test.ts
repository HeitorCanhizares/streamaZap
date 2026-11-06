import { describe, expect, it } from 'vitest'
import { interpretarRetorno } from './deepLink'

const B = 'streamazap://auth/callback'

describe('interpretarRetorno', () => {
  it('lê o code da query', () => {
    expect(interpretarRetorno(`${B}?code=abc123`)).toEqual({ tipo: 'code', code: 'abc123' })
  })

  it('ignora maiúsculas no esquema e no host', () => {
    expect(interpretarRetorno('STREAMAZAP://AUTH/callback?code=x')).toEqual({
      tipo: 'code',
      code: 'x',
    })
  })

  it('aceita barra antes da query', () => {
    expect(interpretarRetorno(`${B}/?code=x`)).toEqual({ tipo: 'code', code: 'x' })
  })

  it('erro access_denied na query vira "Login cancelado."', () => {
    expect(
      interpretarRetorno(`${B}?error=access_denied&error_description=User+cancelled`),
    ).toEqual({ tipo: 'erro', mensagem: 'Login cancelado.' })
  })

  it('a descrição em inglês do Supabase não chega à tela', () => {
    expect(
      interpretarRetorno(`${B}#error=server_error&error_description=Unable+to+exchange+external+code`),
    ).toEqual({ tipo: 'erro', mensagem: 'Não foi possível entrar.' })
  })

  it('error_code conhecido vira texto em português', () => {
    expect(
      interpretarRetorno(`${B}#error=invalid_request&error_code=flow_state_expired&error_description=x`),
    ).toEqual({ tipo: 'erro', mensagem: 'O login expirou. Tente entrar de novo.' })
  })

  it('erro sem descrição usa a mensagem padrão', () => {
    expect(interpretarRetorno(`${B}?error=server_error`)).toEqual({
      tipo: 'erro',
      mensagem: 'Não foi possível entrar.',
    })
  })

  it('erro e code juntos -> erro', () => {
    expect(interpretarRetorno(`${B}?code=x&error=server_error`)).toEqual({
      tipo: 'erro',
      mensagem: 'Não foi possível entrar.',
    })
  })

  it('error_code=access_denied também cancela', () => {
    expect(interpretarRetorno(`${B}#error_code=access_denied&error_description=x`)).toEqual({
      tipo: 'erro',
      mensagem: 'Login cancelado.',
    })
  })

  it('a query vence o fragmento em conflito', () => {
    expect(interpretarRetorno(`${B}?code=q#code=f`)).toEqual({ tipo: 'code', code: 'q' })
  })

  it('ignora prefixo parecido, outro esquema e retorno vazio', () => {
    expect(interpretarRetorno('streamazap://auth/callbackx?code=x')).toEqual({ tipo: 'ignorar' })
    expect(interpretarRetorno('https://x.com/?code=1')).toEqual({ tipo: 'ignorar' })
    expect(interpretarRetorno(B)).toEqual({ tipo: 'ignorar' })
  })

  it('não lança com string vazia ou lixo', () => {
    expect(interpretarRetorno('')).toEqual({ tipo: 'ignorar' })
    expect(interpretarRetorno('::::')).toEqual({ tipo: 'ignorar' })
    expect(() => interpretarRetorno(`${B}?code=%E0%A4%A`)).not.toThrow()
  })
})
