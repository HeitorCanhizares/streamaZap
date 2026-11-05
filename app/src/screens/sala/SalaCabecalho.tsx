// Tarefa sala-01: cabeçalho da sala (3-sala.html 72-84).
import type { ReactNode } from 'react'
import type { RoomView, TileView } from '../../bridge/types'
import { useEffect, useRef, useState } from 'react'
import { roomChip, watching } from './salaModel'

export interface SalaCabecalhoProps {
  room: RoomView
  /** O que está na tela principal (só a trilha importa). */
  main: Pick<TileView, 'track_id'> | null
  sharing: boolean
  panelOpen: boolean
  onOpenPanel: () => void
  onStop?: () => void
  invite?: string
  onCopied?: () => void
}

const MONO = { fontFamily: 'var(--font-mono)' } as const

export function SalaCabecalho(props: SalaCabecalhoProps): ReactNode {
  const { room, main, sharing, panelOpen, onOpenPanel, onStop, invite, onCopied } = props
  const assistindo = watching(room, main)
  const [copiado, setCopiado] = useState(false)
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)

  // Limpa o timer ao desmontar para não atualizar estado de componente morto.
  useEffect(
    () => () => {
      if (timer.current !== null) clearTimeout(timer.current)
    },
    [],
  )

  function copiar(): void {
    try {
      void navigator.clipboard?.writeText(invite ?? room.name)?.catch(() => {})
    } catch {
      // falha ao copiar é ignorada
    }
    onCopied?.()
    setCopiado(true)
    if (timer.current !== null) clearTimeout(timer.current)
    timer.current = setTimeout(() => setCopiado(false), 2000)
  }

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
          <span
            style={{
              ...MONO,
              height: 22,
              padding: '0 8px',
              borderRadius: 6,
              background: 'rgba(59,240,138,0.12)',
              color: 'var(--verde)',
              fontSize: 11,
              fontWeight: 600,
              letterSpacing: '0.06em',
              display: 'inline-flex',
              alignItems: 'center',
            }}
          >
            {roomChip(room)}
          </span>
          {assistindo !== null && (
            <span>
              Assistindo <b style={{ color: 'var(--texto)', fontWeight: 600 }}>{assistindo.name}</b>
              {` · ${assistindo.kind}`}
            </span>
          )}
        </span>
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

      <button
        type="button"
        className="zp-ghost"
        onClick={copiar}
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
          <rect x="8" y="8" width="12" height="12" rx="2" />
          <path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2" />
        </svg>
        {copiado ? 'Convite copiado' : 'Copiar convite'}
      </button>

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
