// Tarefa ui-02
import type { ReactNode } from 'react'

export interface ButtonProps { variant?: 'primary' | 'ghost' | 'danger'; children?: ReactNode; onClick?: () => void; disabled?: boolean }

// Botão padrão do tema. `type="button"` evita submit acidental dentro de formulários.
export function Button({ variant = 'primary', children, onClick, disabled }: ButtonProps): ReactNode {
  return (
    <button
      type="button"
      className={`zp-btn zp-btn-${variant}`}
      disabled={disabled}
      onClick={() => {
        // Guarda extra: o disabled nativo já bloqueia o clique, mas não custa garantir.
        if (!disabled) onClick?.()
      }}
    >
      {children}
    </button>
  )
}
