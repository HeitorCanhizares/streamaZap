// Cartão de resumo com ícone, veredito e selos (5-diagnostico.html linhas 71-89).
import type { ReactNode } from 'react'
import type { Check, Veredito } from './diagModel'

export interface ResumoProps { checks: Check[]; veredito: Veredito; verificando: boolean }

/** Estado visual do selo: 'checking' enquanto verifica, senão o status da checagem. */
function estadoSelo(check: Check, verificando: boolean): 'checking' | 'ok' | 'warn' | 'soon' {
  if (verificando) return 'checking'
  return check.status === 'ok' ? 'ok' : check.status === 'warn' ? 'warn' : 'soon'
}

export function Resumo({ checks, veredito, verificando }: ResumoProps): ReactNode {
  return (
    <section
      aria-label="Resumo"
      style={{
        display: 'flex',
        flexWrap: 'wrap',
        alignItems: 'center',
        gap: 24,
        padding: '22px 24px',
        borderRadius: 20,
        background: 'var(--superficie)',
        border: '1px solid var(--borda)',
      }}
    >
      <span
        style={{
          position: 'relative',
          width: 104,
          height: 104,
          flex: '0 0 104px',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
        }}
      >
        <span
          className="zp-pulse"
          style={{
            position: 'absolute',
            inset: 0,
            borderRadius: 30,
            border: `2px solid ${veredito.cor}`,
          }}
        />
        <img
          src="/icone.png"
          alt=""
          style={{
            position: 'relative',
            width: 92,
            height: 92,
            borderRadius: 24,
            display: 'block',
            boxShadow: '0 0 0 1px #2A443C, 0 0 30px rgba(59,240,138,0.3)',
          }}
        />
      </span>
      <div
        style={{
          flex: '1 1 320px',
          minWidth: 0,
          display: 'flex',
          flexDirection: 'column',
          gap: 8,
        }}
      >
        <h2
          style={{
            margin: 0,
            fontFamily: "'Unbounded', sans-serif",
            fontWeight: 700,
            fontSize: 24,
            letterSpacing: '-0.01em',
          }}
        >
          {veredito.titulo}
        </h2>
        <span style={{ color: 'var(--texto-2)' }}>{veredito.sub}</span>
        <span
          style={{
            fontFamily: "'JetBrains Mono', monospace",
            fontSize: 12,
            color: 'var(--texto-3)',
          }}
        >
          streamaZap 0.1.0 · protocolo 3 · Windows 11 64 bits
        </span>
      </div>
      <ul
        style={{
          listStyle: 'none',
          margin: 0,
          padding: 0,
          display: 'flex',
          flexWrap: 'wrap',
          gap: 8,
        }}
      >
        {checks.map((check) => {
          const s = estadoSelo(check, verificando)
          const col = s === 'ok' ? '#3BF08A' : s === 'warn' ? '#FFB547' : '#A3BDB1'
          const bg = s === 'ok'
            ? 'rgba(59,240,138,0.12)'
            : s === 'warn'
              ? 'rgba(255,181,71,0.12)'
              : '#16241F'
          return (
            <li
              key={check.id}
              data-estado={s}
              style={{
                height: 32,
                padding: '0 12px',
                borderRadius: 999,
                fontSize: 12.5,
                fontWeight: 600,
                display: 'flex',
                alignItems: 'center',
                color: col,
                border: `1px solid ${col}55`,
                background: bg,
              }}
            >
              {check.short}
            </li>
          )
        })}
      </ul>
    </section>
  )
}
