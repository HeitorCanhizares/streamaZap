// Cabeçalho da tela Diagnóstico (5-diagnostico.html linhas 61-69).
import type { ReactNode } from 'react'

export interface CabecalhoProps {
  /** Rótulo do botão primário ("Copiar relatório" ou "Copiado!"). */
  copyLabel: string
  onRecheck: () => void
  onCopy: () => void
}

export function Cabecalho({ copyLabel, onRecheck, onCopy }: CabecalhoProps): ReactNode {
  return (
    <div
      style={{
        display: 'flex',
        flexWrap: 'wrap',
        alignItems: 'flex-end',
        gap: 14,
      }}
    >
      <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
        <span
          style={{
            fontFamily: "'JetBrains Mono', monospace",
            fontSize: 11,
            letterSpacing: '0.14em',
            textTransform: 'uppercase',
            color: 'var(--verde)',
          }}
        >
          Diagnóstico
        </span>
        <h1
          style={{
            margin: 0,
            fontFamily: "'Unbounded', sans-serif",
            fontWeight: 700,
            fontSize: 32,
            letterSpacing: '-0.02em',
          }}
        >
          Tá tudo conectando?
        </h1>
      </div>
      <span style={{ flex: 1 }} />
      <button
        type="button"
        className="zp-ghost"
        onClick={onRecheck}
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
          width={18}
          height={18}
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth={1.8}
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <path d="M20 11a8 8 0 0 0-14.6-4.5M4 13a8 8 0 0 0 14.6 4.5" />
          <path d="M5 3v4h4M19 21v-4h-4" />
        </svg>
        Verificar de novo
      </button>
      <button
        type="button"
        className="zp-cta"
        onClick={onCopy}
        style={{
          height: 44,
          padding: '0 18px',
          borderRadius: 12,
          border: 0,
          background: 'var(--verde)',
          color: 'var(--verde-texto)',
          fontWeight: 700,
          display: 'flex',
          alignItems: 'center',
          gap: 8,
        }}
      >
        <svg
          width={18}
          height={18}
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth={1.8}
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <rect x={8} y={8} width={12} height={12} rx={2} />
          <path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2" />
        </svg>
        {copyLabel}
      </button>
    </div>
  )
}
