// Tarefa ui-04: controle deslizante nativo; entrega sempre um número ao pai.
import type { ReactNode } from 'react'

export interface SliderProps { value: number; min?: number; max?: number; step?: number; onChange: (v: number) => void }

export function Slider({ value, min = 0, max = 1, step = 0.01, onChange }: SliderProps): ReactNode {
  return (
    <input
      type="range"
      className="zp-slider"
      value={value}
      min={min}
      max={max}
      step={step}
      onChange={(e) => onChange(Number(e.target.value))}
    />
  )
}
