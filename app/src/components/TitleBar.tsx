// Tarefa ui-03
import type { ReactNode } from 'react'

export interface TitleBarProps { title?: string; onMinimize?: () => void; onMaximize?: () => void; onClose?: () => void }

// Barra de título própria. O próprio <header> é a região de arrasto (Tauri);
// os botões não têm o atributo, assim o clique neles não inicia arrasto.
export function TitleBar({ title, onMinimize, onMaximize, onClose }: TitleBarProps): ReactNode {
  return (
    <header className="zp-titlebar" data-tauri-drag-region>
      <span className="zp-titlebar-title" style={{ fontFamily: 'var(--font-titulo)' }}>
        {title ?? 'streamaZap'}
      </span>
      <div className="zp-titlebar-controls">
        <button type="button" aria-label="Minimizar" onClick={() => onMinimize?.()}>
          <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
            <path d="M2 6h8" stroke="currentColor" strokeWidth="1.5" />
          </svg>
        </button>
        <button type="button" aria-label="Maximizar" onClick={() => onMaximize?.()}>
          <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
            <rect x="2" y="2" width="8" height="8" fill="none" stroke="currentColor" strokeWidth="1.5" />
          </svg>
        </button>
        <button type="button" aria-label="Fechar" onClick={() => onClose?.()}>
          <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
            <path d="M2 2l8 8M10 2l-8 8" stroke="currentColor" strokeWidth="1.5" />
          </svg>
        </button>
      </div>
    </header>
  )
}
