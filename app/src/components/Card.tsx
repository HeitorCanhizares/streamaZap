// Tarefa ui-02
import type { ReactNode } from 'react'

export interface CardProps { children?: ReactNode; className?: string }

// Superfície básica do tema; `className` extra é anexado após a classe base.
export function Card({ children, className }: CardProps): ReactNode {
  return <div className={'zp-card ' + (className ?? '')}>{children}</div>
}
