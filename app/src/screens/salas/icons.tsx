// Tarefa salas-01: ícones SVG da tela Salas, copiados do design (1-salas.html).
// Cada ícone usa os mesmos paths e atributos de stroke do design. `cor` vira o stroke
// (padrão currentColor). Só IconeCadeado é anunciado para leitor de tela ("com senha");
// os demais são decorativos e ficam aria-hidden.
import type { ReactNode } from 'react'

export interface IconeProps { size?: number; cor?: string }

interface BaseProps extends IconeProps {
  defaultSize: number
  strokeWidth: number
  linejoin?: 'round'
  children: ReactNode
  role?: 'img'
  label?: string
}

// Monta o <svg> comum. Atributos fixos ficam aqui para não repetir em cada ícone.
function Base({ size, cor, defaultSize, strokeWidth, linejoin, children, role, label }: BaseProps & { size?: number; cor?: string }): ReactNode {
  const s = size ?? defaultSize
  return (
    <svg
      viewBox="0 0 24 24"
      width={s}
      height={s}
      fill="none"
      stroke={cor ?? 'currentColor'}
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin={linejoin}
      aria-hidden={role === 'img' ? undefined : 'true'}
      role={role}
      aria-label={label}
    >
      {children}
    </svg>
  )
}

// Globo da aba Online (design linha 74).
export function IconeGlobo(props: IconeProps): ReactNode {
  return (
    <Base {...props} defaultSize={20} strokeWidth={1.8}>
      <circle cx="12" cy="12" r="9"></circle>
      <path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18"></path>
    </Base>
  )
}

// Wi-Fi da aba Local (design linha 75).
export function IconeWifi(props: IconeProps): ReactNode {
  return (
    <Base {...props} defaultSize={20} strokeWidth={1.8}>
      <path d="M2 9a15 15 0 0 1 20 0M5 12.5a10 10 0 0 1 14 0M8.5 16a5 5 0 0 1 7 0M12 19.5h.01"></path>
    </Base>
  )
}

// Mais do botão "Nova sala" (design linha 68).
export function IconeMais(props: IconeProps): ReactNode {
  return (
    <Base {...props} defaultSize={18} strokeWidth={2.2}>
      <path d="M12 5v14M5 12h14"></path>
    </Base>
  )
}

// Lupa do campo de busca (design linha 84).
export function IconeLupa(props: IconeProps): ReactNode {
  return (
    <Base {...props} defaultSize={18} strokeWidth={1.8}>
      <circle cx="11" cy="11" r="7"></circle>
      <path d="m20 20-3.5-3.5"></path>
    </Base>
  )
}

// Cadeado da lista de salas com senha (design linha 100). É o único anunciado.
export function IconeCadeado(props: IconeProps): ReactNode {
  return (
    <Base {...props} defaultSize={14} strokeWidth={2} role="img" label="com senha">
      <rect x="5" y="11" width="14" height="10" rx="2"></rect>
      <path d="M8 11V8a4 4 0 0 1 8 0v3"></path>
    </Base>
  )
}

// Cadeado aberto do card "Sala aberta" (design linha 198).
export function IconeCadeadoAberto(props: IconeProps): ReactNode {
  return (
    <Base {...props} defaultSize={18} strokeWidth={1.8}>
      <rect x="5" y="11" width="14" height="10" rx="2"></rect>
      <path d="M8 11V8a4 4 0 0 1 7.5-2"></path>
    </Base>
  )
}

// Olho do contador de espectadores (design linha 110).
export function IconeOlho(props: IconeProps): ReactNode {
  return (
    <Base {...props} defaultSize={13} strokeWidth={2}>
      <path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z"></path>
      <circle cx="12" cy="12" r="3"></circle>
    </Base>
  )
}

// Escudo do aviso de firewall (design linha 146). Pingo interno usa linecap round.
export function IconeEscudo(props: IconeProps): ReactNode {
  return (
    <Base {...props} defaultSize={20} strokeWidth={1.8} linejoin="round">
      <path d="M12 3 5 6v5c0 5 3 8.5 7 10 4-1.5 7-5 7-10V6z"></path>
      <path d="M12 9v4M12 16h.01"></path>
    </Base>
  )
}

// Triângulo de alerta do aviso de versão (design linha 202).
export function IconeAviso(props: IconeProps): ReactNode {
  return (
    <Base {...props} defaultSize={18} strokeWidth={1.8} linejoin="round">
      <path d="M12 3 2 21h20z"></path>
      <path d="M12 10v5M12 18h.01"></path>
    </Base>
  )
}

// Seta do botão "Entrar na sala" (design linha 205).
export function IconeSeta(props: IconeProps): ReactNode {
  return (
    <Base {...props} defaultSize={20} strokeWidth={2.2} linejoin="round">
      <path d="M5 12h14M13 6l6 6-6 6"></path>
    </Base>
  )
}

// Chave do card "Sala com senha" (design linha 190). O desenho é o do cadeado, com
// stroke 1.8 e tamanho 18, como na linha citada.
export function IconeChave(props: IconeProps): ReactNode {
  return (
    <Base {...props} defaultSize={18} strokeWidth={1.8}>
      <rect x="5" y="11" width="14" height="10" rx="2"></rect>
      <path d="M8 11V8a4 4 0 0 1 8 0v3"></path>
    </Base>
  )
}
