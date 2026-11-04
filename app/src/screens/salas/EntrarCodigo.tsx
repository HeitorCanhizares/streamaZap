// Campo "Entrar com código" (modo Online da tela Salas). Tarefa ui-online-05.
import type { ReactNode } from 'react'

export interface EntrarCodigoProps {
  /** Chamado com o código já normalizado ('ZAP-7K2Q') e a senha (null se vazia). */
  onEntrar: (codigo: string, senha: string | null) => void
  onCancelar: () => void
  ocupado: boolean
  erro: string | null
}

export function EntrarCodigo(props: EntrarCodigoProps): ReactNode {
  void props
  throw new Error('tarefa ui-online-05')
}
