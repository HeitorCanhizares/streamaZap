// Parte "entrar" da seção Conta: botões Google e Microsoft, e-mail com código de 6 dígitos. Tarefa front-11.
import { useState } from 'react'
import type { CSSProperties, ReactNode } from 'react'
import { Button } from '../../components/Button'
import { cancelarCodigo, confirmarCodigo, entrarEmail, entrarOAuth } from '../../bridge/contaAcoes'
import { useContaEstado } from '../../bridge/contaStore'

const rotulo: CSSProperties = { fontSize: 12.5, color: '#A3BDB1' }
const campo: CSSProperties = {
  height: 44, padding: '0 14px', borderRadius: 12, border: '1px solid #2A443C', background: '#0F1916', color: '#E6F4EC',
}
const coluna: CSSProperties = { display: 'flex', flexDirection: 'column', gap: 6 }
const linha: CSSProperties = { display: 'flex', flexWrap: 'wrap', gap: 10 }

/**
 * Lê o `useContaEstado()`. Botões "Entrar com Google" e "Entrar com Microsoft" (`entrarOAuth`),
 * campo "E-mail" + botão "Enviar código" (`entrarEmail`); com `otpEmail` mostra o campo "Código
 * de 6 dígitos" + "Confirmar" (`confirmarCodigo`) e "Cancelar" (`cancelarCodigo`). `aviso` em
 * `role="alert"`. Tudo desabilitado com `ocupado`.
 */
export function ContaEntrar(): ReactNode {
  const estado = useContaEstado()
  const [email, setEmail] = useState('')
  const [codigo, setCodigo] = useState('')
  const { ocupado, otpEmail, aviso } = estado

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
      <div style={linha}>
        <Button disabled={ocupado} onClick={() => void entrarOAuth('google')}>Entrar com Google</Button>
        <Button variant="ghost" disabled={ocupado} onClick={() => void entrarOAuth('azure')}>Entrar com Microsoft</Button>
      </div>
      {otpEmail == null ? (
        <>
          <div style={coluna}>
            <label htmlFor="conta-email" style={rotulo}>E-mail</label>
            <input
              id="conta-email"
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              style={campo}
            />
          </div>
          <div style={linha}>
            <Button disabled={ocupado} onClick={() => void entrarEmail(email)}>Enviar código</Button>
          </div>
        </>
      ) : (
        <>
          <span style={{ fontSize: 13, color: '#A3BDB1' }}>Enviamos um código de 6 dígitos para {otpEmail}.</span>
          <div style={coluna}>
            <label htmlFor="conta-codigo" style={rotulo}>Código de 6 dígitos</label>
            <input
              id="conta-codigo"
              inputMode="numeric"
              maxLength={6}
              value={codigo}
              onChange={(e) => setCodigo(e.target.value.replace(/\D/g, '').slice(0, 6))}
              style={{ ...campo, width: 160, fontFamily: "'JetBrains Mono', monospace" }}
            />
          </div>
          <div style={linha}>
            <Button disabled={ocupado} onClick={() => void confirmarCodigo(codigo)}>Confirmar</Button>
            <Button variant="ghost" disabled={ocupado} onClick={() => { setCodigo(''); void cancelarCodigo() }}>Cancelar</Button>
          </div>
        </>
      )}
      {aviso != null && <span role="alert" style={{ fontSize: 13, color: '#FF6B6B' }}>{aviso}</span>}
    </div>
  )
}
