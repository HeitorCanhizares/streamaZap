// Tarefa ui-05
import type { ReactNode } from 'react'

export interface SelectProps {
  options: { value: string; label: string }[]
  value: string
  onChange: (v: string) => void
  ariaLabel?: string
}

// Select controlado: o valor vem de fora e a mudança sobe como string
export function Select({ options, value, onChange, ariaLabel }: SelectProps): ReactNode {
  return (
    <select className="zp-input" value={value} aria-label={ariaLabel} onChange={(e) => onChange(e.target.value)}>
      {options.map((o) => (
        <option key={o.value} value={o.value}>{o.label}</option>
      ))}
    </select>
  )
}
