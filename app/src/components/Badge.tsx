// Tarefa ui-04: selo com tom; as cores vêm das classes zp-badge-<tom> do theme.css.
import type { ReactNode } from 'react'

export interface BadgeProps { tone?: 'green' | 'yellow' | 'red' | 'blue' | 'neutral'; children?: ReactNode }

export function Badge({ tone = 'neutral', children }: BadgeProps): ReactNode {
  return <span className={`zp-badge zp-badge-${tone}`}>{children}</span>
}
