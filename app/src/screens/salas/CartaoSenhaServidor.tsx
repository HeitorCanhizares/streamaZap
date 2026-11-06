// Cartão da tela Salas → Online quando o login falhou por `senha_servidor`. Tarefa front-14.
import { useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../../bridge/api'
import { Button } from '../../components/Button'

// Mesmo encaixe do AtualizadoCard no detalhe da tela Salas.
const estiloCartao = {
  display: 'flex',
  flexDirection: 'column',
  gap: 10,
  margin: '16px 32px 0',
  padding: '14px 18px',
  borderRadius: 14,
  border: '1px solid #1F332D',
  background: '#0A100E',
  fontSize: 12.5,
  color: '#A3BDB1',
} as const

const estiloCampo = {
  flex: '1 1 200px',
  minWidth: 0,
  height: 40,
  padding: '0 14px',
  borderRadius: 12,
  border: '1px solid #2A443C',
  background: '#0F1916',
  color: '#E6F4EC',
  letterSpacing: '0.1em',
} as const

const estiloLinha = { display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 10 } as const

/**
 * Pede a senha do servidor. Ao enviar, lê as settings, grava `server` completo com a senha
 * por último (vence a antiga) e o núcleo tenta entrar de novo sozinho.
 */
export function CartaoSenhaServidor(): ReactNode {
  const [senha, setSenha] = useState('')
  const [enviando, setEnviando] = useState(false)
  const [erro, setErro] = useState<string | null>(null)

  const desabilitado = senha.trim() === '' || enviando

  const enviar = async (): Promise<void> => {
    if (desabilitado) return
    const password = senha
    setEnviando(true)
    setErro(null)
    try {
      const atual = await api.settingsGet()
      await api.settingsSet({
        server: { enabled: true, address: '', use_relay: true, ...atual.server, password },
      })
      setSenha('')
    } catch (e) {
      setErro(String(e))
    } finally {
      setEnviando(false)
    }
  }

  return (
    <div data-testid="cartao-senha-servidor" style={estiloCartao}>
      <strong style={{ color: '#E6F4EC', fontSize: 14 }}>O Online está em teste fechado</strong>
      <span>Digite a senha do servidor (pede pro dono do servidor).</span>
      <div style={estiloLinha}>
        <label htmlFor="senha-servidor-salas" style={{ position: 'absolute', width: 1, height: 1, overflow: 'hidden', clip: 'rect(0 0 0 0)' }}>
          Senha do servidor
        </label>
        <input
          id="senha-servidor-salas"
          type="password"
          placeholder="Senha do servidor"
          style={estiloCampo}
          value={senha}
          onChange={(e) => setSenha(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') void enviar()
          }}
          autoComplete="off"
        />
        <Button onClick={() => void enviar()} disabled={desabilitado}>
          Entrar
        </Button>
      </div>
      {erro && (
        <p role="alert" style={{ margin: 0, color: 'var(--vermelho)' }}>
          {erro}
        </p>
      )}
    </div>
  )
}
