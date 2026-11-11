// Selo do plano que libera algo (cadeado opcional). Tarefa telas-01.
import type { ReactNode } from 'react'
import type { PlanId } from '../bridge/types'
import { seloPlano } from '../bridge/planoModel'

export interface SeloPlanoProps { plano: PlanId; cadeado?: boolean }

/** Ícone de cadeado (SVG em linha) + `seloPlano(plano)`. Sem texto de preço. */
export function SeloPlano({ plano, cadeado = false }: SeloPlanoProps): ReactNode {
  return (
    <span
      data-testid="selo-plano"
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: 4,
        fontSize: 11,
        fontWeight: 600,
        color: 'var(--verde)',
        padding: '2px 8px',
        border: '1px solid rgba(59,240,138,0.4)',
        borderRadius: 999,
        whiteSpace: 'nowrap',
      }}
    >
      {cadeado && (
        <svg
          aria-hidden="true"
          width={12}
          height={12}
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth={2}
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <rect x="5" y="11" width="14" height="10" rx="2" />
          <path d="M8 11V8a4 4 0 0 1 8 0v3" />
        </svg>
      )}
      {seloPlano(plano)}
    </span>
  )
}
