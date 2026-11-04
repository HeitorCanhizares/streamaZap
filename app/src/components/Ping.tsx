// Tarefa ui-04: ping em ms, colorido pelo tom de pingTone (verde/amarelo/vermelho).
import type { ReactNode } from 'react'
import { pingTone, type PingTone } from '../pingColor'

export interface PingProps { ms: number }

const COR: Record<PingTone, string> = {
  green: 'var(--verde)',
  yellow: 'var(--amarelo)',
  red: 'var(--vermelho)',
}

export function Ping({ ms }: PingProps): ReactNode {
  const tom = pingTone(ms)
  return (
    <span className="zp-mono" data-tone={tom} style={{ color: COR[tom] }}>
      {`${Math.round(ms)} ms`}
    </span>
  )
}
