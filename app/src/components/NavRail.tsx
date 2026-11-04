// Tarefa ui-03
import type { ReactNode } from 'react'

export interface NavRailProps { active: 'salas' | 'config' | 'diag'; onNavigate: (to: 'salas' | 'config' | 'diag') => void }

// Ícones SVG simples, inline, sem dependência externa.
const ITENS: { to: 'salas' | 'config' | 'diag'; label: string; icone: ReactNode }[] = [
  {
    to: 'salas',
    label: 'Salas',
    icone: (
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
        <rect x="3" y="3" width="8" height="8" rx="2" />
        <rect x="13" y="3" width="8" height="8" rx="2" />
        <rect x="3" y="13" width="8" height="8" rx="2" />
        <rect x="13" y="13" width="8" height="8" rx="2" />
      </svg>
    ),
  },
  {
    to: 'config',
    label: 'Configuracoes',
    icone: (
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
        <circle cx="12" cy="12" r="3" />
        <path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M4.9 19.1L7 17M17 7l2.1-2.1" />
      </svg>
    ),
  },
  {
    to: 'diag',
    label: 'Diagnostico',
    icone: (
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
        <path d="M3 12h4l3-7 4 14 3-7h4" />
      </svg>
    ),
  },
]

// Trilho lateral de navegação. O item ativo recebe aria-current="page".
export function NavRail({ active, onNavigate }: NavRailProps): ReactNode {
  return (
    <nav className="zp-navrail">
      {ITENS.map((item) => {
        const ativo = item.to === active
        return (
          <button
            key={item.to}
            type="button"
            aria-label={item.label}
            aria-current={ativo ? 'page' : undefined}
            className={'zp-nav-item' + (ativo ? ' zp-nav-active' : '')}
            onClick={() => onNavigate(item.to)}
          >
            {item.icone}
          </button>
        )
      })}
    </nav>
  )
}
