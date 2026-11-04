// Tarefa ui-04: avatar circular com a inicial do nome.
import type { ReactNode } from 'react'

export interface AvatarProps { name: string; color: string; size?: number }

// Primeiro caractere não-espaço, em maiúscula; '?' quando o nome não tem nenhum.
// Usa código de ponto (não índice) para não quebrar emoji em dois.
function inicial(name: string): string {
  const primeiro = [...name.trim()][0]
  return primeiro ? primeiro.toUpperCase() : '?'
}

export function Avatar({ name, color, size = 32 }: AvatarProps): ReactNode {
  return (
    <div
      className="zp-avatar"
      style={{
        width: size,
        height: size,
        background: color,
        color: 'var(--verde-texto)',
      }}
    >
      {inicial(name)}
    </div>
  )
}
