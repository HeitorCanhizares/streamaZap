// Campo "Entrar com código" (modo Online da tela Salas). Tarefa ui-online-05.
import { useState } from 'react'
import type { KeyboardEvent, ReactNode } from 'react'
import { Button } from '../../components/Button'
import { normalizarCodigo } from '../../bridge/serverModel'

export interface EntrarCodigoProps {
  /** Código com que o campo abre (convite pelo link); vazio por padrão. */
  codigoInicial?: string
  /** Chamado com o código já normalizado ('ZAP-7K2Q') e a senha (null se vazia). */
  onEntrar: (codigo: string, senha: string | null) => void
  onCancelar: () => void
  ocupado: boolean
  erro: string | null
}

const MSG_CODIGO_INVALIDO = 'Código inválido. Ele tem o formato ZAP-XXXX.'

// Os campos são <input> diretos (e não o TextField) porque precisamos de aria-label,
// da fonte mono e do onKeyDown para o Enter, que o TextField não expõe.
export function EntrarCodigo({ codigoInicial = '', onEntrar, onCancelar, ocupado, erro }: EntrarCodigoProps): ReactNode {
  const [codigo, setCodigo] = useState(codigoInicial)
  const [senha, setSenha] = useState('')
  const [erroLocal, setErroLocal] = useState<string | null>(null)

  const entrar = (): void => {
    if (ocupado) return
    const normalizado = normalizarCodigo(codigo)
    if (normalizado === null) {
      setErroLocal(MSG_CODIGO_INVALIDO)
      return
    }
    setErroLocal(null)
    onEntrar(normalizado, senha === '' ? null : senha)
  }

  const aoTeclar = (e: KeyboardEvent<HTMLInputElement>): void => {
    if (e.key === 'Enter') entrar()
  }

  const msgErro = erroLocal ?? erro

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 10, padding: '16px 32px 0' }}>
      <input
        className="zp-input"
        type="text"
        aria-label="Código da sala"
        placeholder="Código da sala (ex.: ZAP-7K2Q)"
        value={codigo}
        style={{ fontFamily: 'var(--font-mono)' }}
        onChange={(e) => {
          setCodigo(e.target.value)
          setErroLocal(null)
        }}
        onKeyDown={aoTeclar}
      />
      <input
        className="zp-input"
        type="password"
        aria-label="Senha da sala (se tiver)"
        placeholder="Senha, se a sala tiver"
        value={senha}
        onChange={(e) => setSenha(e.target.value)}
        onKeyDown={aoTeclar}
      />
      {msgErro !== null ? (
        <p role="alert" style={{ margin: 0, color: 'var(--vermelho)', fontSize: 13 }}>{msgErro}</p>
      ) : null}
      <div style={{ display: 'flex', gap: 10 }}>
        <Button variant="primary" disabled={ocupado} onClick={entrar}>Entrar</Button>
        <Button variant="ghost" onClick={onCancelar}>Cancelar</Button>
      </div>
    </div>
  )
}
