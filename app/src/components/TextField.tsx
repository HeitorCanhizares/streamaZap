// Tarefa ui-05
import type { ReactNode } from 'react'

export interface TextFieldProps { value: string; onChange: (v: string) => void; placeholder?: string; type?: 'text' | 'password' }

// Campo de texto controlado; type 'password' esconde o conteúdo na tela
export function TextField({ value, onChange, placeholder, type = 'text' }: TextFieldProps): ReactNode {
  return (
    <input
      className="zp-input"
      type={type}
      value={value}
      placeholder={placeholder}
      onChange={(e) => onChange(e.target.value)}
    />
  )
}
