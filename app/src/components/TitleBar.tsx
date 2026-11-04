// Tarefa shell-05
import type { ReactNode } from 'react'
import type { TitleBarProps } from './shell/types'
import { ServerStatus } from './shell/ServerStatus'
import { WindowControls } from './shell/WindowControls'

export type { TitleBarProps } from './shell/types'

// Barra de título fiel ao design (docs/design/1-salas.html, linhas 35-52).
// O próprio <header> e os textos são região de arrasto (Tauri); os botões
// não têm o atributo, assim o clique neles não inicia arrasto.
export function TitleBar(props: TitleBarProps): ReactNode {
  const { version, crumb, right, onMinimize, onMaximize, onClose, serverMs, server, radminIp } = props

  return (
    <header className="zp-titlebar" data-tauri-drag-region>
      <img
        src="/icone.png"
        alt=""
        style={{ width: 24, height: 24, borderRadius: 7, display: 'block', marginRight: -4 }}
        data-tauri-drag-region
      />
      <span
        data-tauri-drag-region
        style={{ fontFamily: 'var(--font-titulo)', fontWeight: 700, fontSize: 13, letterSpacing: '0.02em' }}
      >
        streama<span style={{ color: 'var(--verde)' }}>Zap</span>
      </span>
      {crumb ? (
        <span data-tauri-drag-region style={{ fontSize: 13, color: 'var(--texto-3)' }}>
          {crumb.pai ? (
            <>
              {crumb.pai} <span aria-hidden="true">/</span>{' '}
              <span style={{ color: 'var(--texto)' }}>{crumb.atual}</span>
            </>
          ) : (
            crumb.atual
          )}
        </span>
      ) : version ? (
        <span
          data-tauri-drag-region
          style={{ fontFamily: 'var(--font-mono)', fontSize: 11, color: 'var(--texto-3)' }}
        >
          {version}
        </span>
      ) : null}
      <span style={{ flex: 1 }} data-tauri-drag-region />
      {right ?? <ServerStatus serverMs={serverMs} server={server} radminIp={radminIp} />}
      <WindowControls onMinimize={onMinimize} onMaximize={onMaximize} onClose={onClose} />
    </header>
  )
}
