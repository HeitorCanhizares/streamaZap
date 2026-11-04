// Tarefa sala-01: cabeçalho da sala (3-sala.html 72-84).
import type { ReactNode } from 'react'
import type { RoomView, TileView } from '../../bridge/types'
import { Ping } from '../../components/Ping'

export interface SalaCabecalhoProps {
  room: RoomView
  main: TileView | null
  sharing: boolean
  panelOpen: boolean
  onOpenPanel: () => void
  onStop?: () => void
}

const MONO = { fontFamily: 'var(--font-mono)' } as const

// kbps para Mbps com vírgula decimal, como no design (6,2 Mbps).
function mbps(kbps: number): string {
  return (kbps / 1000).toFixed(1).replace('.', ',')
}

export function SalaCabecalho(props: SalaCabecalhoProps): ReactNode {
  const { room, main, sharing, panelOpen, onOpenPanel, onStop } = props

  return (
    <div style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 12, minHeight: 48 }}>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 4, minWidth: 0 }}>
        <h1
          style={{
            margin: 0,
            fontFamily: 'var(--font-titulo)',
            fontWeight: 700,
            fontSize: 22,
            letterSpacing: '-0.015em',
          }}
        >
          {room.name}
        </h1>
        {main !== null && (
          <span
            style={{
              display: 'flex',
              flexWrap: 'wrap',
              alignItems: 'center',
              gap: 10,
              fontSize: 13,
              color: 'var(--texto-2)',
            }}
          >
            <span>{`via ${room.mode === 'local' ? 'Local' : 'Radmin'}`}</span>
            <span>
              Ping <Ping ms={main.ping_ms} />
            </span>
            <span style={MONO}>{`suav. ${Math.round(main.smoothing_ms)} ms`}</span>
            <span style={MONO}>{`${mbps(main.kbps)} Mbps`}</span>
            <span style={MONO}>{`${main.width}x${main.height}@${main.fps}`}</span>
          </span>
        )}
      </div>

      <span style={{ flex: 1 }} />

      {sharing && (
        <span
          style={{
            height: 40,
            padding: '0 6px 0 14px',
            borderRadius: 999,
            background: 'rgba(59,240,138,0.12)',
            border: '1px solid rgba(59,240,138,0.4)',
            color: 'var(--verde)',
            fontWeight: 600,
            fontSize: 13,
            display: 'flex',
            alignItems: 'center',
            gap: 10,
          }}
        >
          <span
            className="zp-blink"
            aria-hidden="true"
            style={{ width: 7, height: 7, borderRadius: '50%', background: 'var(--verde)' }}
          />
          <span>Compartilhando</span>
          <button
            type="button"
            className="zp-ghost"
            onClick={onStop}
            style={{
              height: 30,
              padding: '0 12px',
              borderRadius: 999,
              border: 0,
              background: 'rgba(59,240,138,0.16)',
              color: 'var(--texto)',
              fontSize: 12,
              fontWeight: 600,
            }}
          >
            Parar
          </button>
        </span>
      )}

      {!panelOpen && (
        <button
          type="button"
          className="zp-ghost"
          onClick={onOpenPanel}
          style={{
            height: 44,
            padding: '0 16px',
            borderRadius: 12,
            border: '1px solid var(--borda-forte)',
            background: 'transparent',
            color: 'var(--texto)',
            fontWeight: 600,
            display: 'flex',
            alignItems: 'center',
            gap: 8,
          }}
        >
          <svg
            width="18"
            height="18"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth={1.8}
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <path d="M21 12a8 8 0 0 1-11.6 7.1L4 20l1-4.6A8 8 0 1 1 21 12z" />
          </svg>
          Painel
        </button>
      )}
    </div>
  )
}
