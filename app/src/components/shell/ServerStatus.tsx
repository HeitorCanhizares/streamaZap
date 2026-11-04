// Tarefa shell-02
import type { ReactNode } from 'react'
import type { ServerStatusProps } from './types'

// Status do servidor (ping) e do IP Radmin, mostrado na barra de título.
// Porte fiel de docs/design/1-salas.html (linhas 38-41).
export function ServerStatus({ serverMs, radminIp }: ServerStatusProps): ReactNode {
  const temMs = typeof serverMs === 'number' && Number.isFinite(serverMs)
  const textoServidor = temMs ? `Servidor online · ${Math.round(serverMs)} ms` : 'Servidor · não configurado'

  const temIp = typeof radminIp === 'string' && radminIp.length > 0
  const textoRadmin = temIp ? `Radmin · ${radminIp}` : 'Radmin · não detectado'

  return (
    <span
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 18,
        fontFamily: 'var(--font-mono)',
        fontSize: 12,
        color: 'var(--texto-2)',
      }}
    >
      <span style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <span
          aria-hidden="true"
          style={{
            width: 8,
            height: 8,
            borderRadius: '50%',
            background: temMs ? 'var(--verde)' : 'var(--texto-3)',
            boxShadow: temMs ? '0 0 8px var(--verde)' : 'none',
          }}
        />
        {textoServidor}
      </span>
      <span style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <span
          aria-hidden="true"
          style={{
            width: 8,
            height: 8,
            borderRadius: '50%',
            background: temIp ? 'var(--azul)' : 'var(--texto-3)',
          }}
        />
        {textoRadmin}
      </span>
    </span>
  )
}
