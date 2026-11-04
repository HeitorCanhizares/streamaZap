import type { ReactNode } from 'react'

// Tarefa shell-04: ícones do trilho (svg 22x22, copiados de docs/design/1-salas.html e 3-sala.html).
// Traço de 1.8 em grade 24x24; `currentColor` deixa a cor vir do botão (ativo/inativo).

export function IconSalas(): ReactNode {
  return (
    <svg width={22} height={22} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} aria-hidden="true" strokeLinejoin="round">
      <path d="M21 12a8 8 0 0 1-11.6 7.1L4 20l1-4.6A8 8 0 1 1 21 12z" />
    </svg>
  )
}

export function IconCriar(): ReactNode {
  return (
    <svg width={22} height={22} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} aria-hidden="true" strokeLinecap="round">
      <path d="M12 5v14M5 12h14" />
    </svg>
  )
}

export function IconDiag(): ReactNode {
  return (
    <svg width={22} height={22} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} aria-hidden="true" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 12h4l3-8 4 16 3-8h4" />
    </svg>
  )
}

export function IconConfig(): ReactNode {
  return (
    <svg width={22} height={22} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} aria-hidden="true" strokeLinecap="round">
      <path d="M4 7h9m4 0h3M4 17h3m4 0h9" />
      <circle cx={15} cy={7} r={2} />
      <circle cx={9} cy={17} r={2} />
    </svg>
  )
}

export function IconSalaAtual(): ReactNode {
  return (
    <svg width={22} height={22} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} role="img" aria-label="Sala atual" strokeLinejoin="round">
      <rect x={3} y={5} width={18} height={12} rx={2} />
      <path d="M8 21h8M12 17v4" strokeLinecap="round" />
      <path d="m10.5 8.5 4 2.5-4 2.5z" fill="currentColor" />
    </svg>
  )
}
