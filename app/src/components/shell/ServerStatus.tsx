// Tarefa shell-02
import type { CSSProperties, ReactNode } from 'react'
import { barraServidor, type TomServidor } from '../../bridge/serverModel'
import type { ServerStatusProps } from './types'

// Porte fiel de docs/design/1-salas.html (linhas 38-41).
// Cor da bolinha do servidor por tom: só o "ok" (online) brilha.
const BOLINHA_POR_TOM: Record<TomServidor, CSSProperties> = {
  ok: { background: 'var(--verde)', boxShadow: '0 0 8px var(--verde)' },
  warn: { background: 'var(--amarelo)', boxShadow: 'none' },
  bad: { background: 'var(--vermelho)', boxShadow: 'none' },
  off: { background: 'var(--texto-3)', boxShadow: 'none' },
}

// Status do servidor (ping) e do IP Radmin, mostrado na barra de título.
// Com `server` (Online) usa o modelo de barraServidor; sem ele, mantém serverMs.
export function ServerStatus({ serverMs, radminIp, server }: ServerStatusProps): ReactNode {
  const temMs = typeof serverMs === 'number' && Number.isFinite(serverMs)

  let textoServidor: string
  let bolinhaServidor: CSSProperties
  let dicaServidor: string | undefined
  if (server !== undefined) {
    const barra = barraServidor(server)
    textoServidor = barra.texto
    bolinhaServidor = BOLINHA_POR_TOM[barra.tom]
    // Motivo da falha fica no title, para não poluir a barra.
    dicaServidor = barra.tom === 'bad' && server.message ? server.message : undefined
  } else {
    textoServidor = temMs ? `Servidor online · ${Math.round(serverMs)} ms` : 'Servidor · não configurado'
    bolinhaServidor = temMs ? BOLINHA_POR_TOM.ok : BOLINHA_POR_TOM.off
  }

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
      <span title={dicaServidor} style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <span
          aria-hidden="true"
          style={{
            width: 8,
            height: 8,
            borderRadius: '50%',
            ...bolinhaServidor,
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
