// Tipos compartilhados da frente shell (barra de título e trilho lateral).
import type { ReactNode } from 'react'
import type { ShareView, TileView } from '../../bridge/types'

// Telas navegáveis. 'criar' e 'sala' têm item próprio no trilho.
export type NavTarget = 'salas' | 'criar' | 'sala' | 'diag' | 'config'

export interface WindowControlsProps { onMinimize?: () => void; onMaximize?: () => void; onClose?: () => void }

export interface ServerStatusProps {
  /** Ping até o servidor em ms; undefined/null = mostra só "Servidor online". */
  serverMs?: number | null
  /** IP do Radmin; null/undefined = "não detectado". */
  radminIp?: string | null
}

export interface SalaMetricsProps { tile: TileView | null; share: ShareView | null }

export interface TitleBarProps extends WindowControlsProps, ServerStatusProps {
  /** Texto de versão ao lado do logo (ex.: "v3.0.0-alpha.0"). Ignorado se `crumb` existir. */
  version?: string
  /** Trilha da tela atual; ausente = mostra a versão. */
  crumb?: { atual: string; pai?: string }
  /** Se presente, substitui o bloco de status do servidor/Radmin (usado na tela da sala). */
  right?: ReactNode
}

export interface NavRailProps {
  active: NavTarget
  onNavigate: (to: NavTarget) => void
  /** Há sala ativa: o item "Criar sala" dá lugar ao indicador "Sala atual". */
  inRoom?: boolean
  /** Nome do perfil (inicial no botão do rodapé). */
  profileName?: string
}
