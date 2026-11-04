// Tarefa ui-02
import type { ReactNode } from 'react'

export interface ToggleProps { checked: boolean; onChange: (v: boolean) => void; label?: string }

// Interruptor acessível (role="switch"). Com `label`, o texto fica ao lado dentro de um <label>,
// assim o clique no texto também alterna.
export function Toggle({ checked, onChange, label }: ToggleProps): ReactNode {
  const button = (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      className={'zp-toggle' + (checked ? ' zp-toggle-on' : '')}
      onClick={() => onChange(!checked)}
    >
      <span />
    </button>
  )
  if (!label) return button
  return (
    <label>
      {button}
      <span>{label}</span>
    </label>
  )
}
