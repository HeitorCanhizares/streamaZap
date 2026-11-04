import type { ReactNode } from 'react'
import type { WindowControlsProps } from './types'

// Botões de janela (minimizar, maximizar, fechar). Porte fiel de docs/design/1-salas.html.
// Sem data-tauri-drag-region: clicar aqui não deve arrastar a janela.
export function WindowControls({ onMinimize, onMaximize, onClose }: WindowControlsProps): ReactNode {
  return (
    <span style={{ display: 'flex', marginLeft: 12 }}>
      <button type="button" className="zp-win" aria-label="Minimizar" onClick={() => onMinimize?.()}>
        <svg width={16} height={16} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" aria-hidden="true">
          <path d="M6 12h12" />
        </svg>
      </button>
      <button type="button" className="zp-win" aria-label="Maximizar" onClick={() => onMaximize?.()}>
        <svg width={14} height={14} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} aria-hidden="true">
          <rect x={5} y={5} width={14} height={14} rx={2} />
        </svg>
      </button>
      <button type="button" className="zp-close" aria-label="Fechar" onClick={() => onClose?.()}>
        <svg width={16} height={16} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" aria-hidden="true">
          <path d="M6 6l12 12M18 6 6 18" />
        </svg>
      </button>
    </span>
  )
}
