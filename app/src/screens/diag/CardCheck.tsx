// Card de uma checagem: ícone, título, selo, linhas (5-diagnostico.html linhas 93-119).
import type { CSSProperties, ReactNode } from 'react'
import type { Check, Tom } from './diagModel'

export interface CardCheckProps { check: Check; verificando: boolean }

type Estado = 'ok' | 'warn' | 'soon' | 'checking'

const TOM: Record<Tom, string> = { ok: '#3BF08A', warn: '#FFB547', bad: '#FF8592', info: '#7E978B' }
const TEXTO_STATUS: Record<Estado, string> = {
  ok: 'Tudo certo',
  warn: 'Atenção',
  checking: 'Verificando…',
  soon: 'em breve',
}

function bgBadge(tom: Tom): string {
  if (tom === 'ok') return 'rgba(59,240,138,0.12)'
  if (tom === 'warn') return 'rgba(255,181,71,0.12)'
  return 'rgba(255,133,146,0.12)'
}

function Icone({ s }: { s: Estado }): ReactNode {
  const comum = {
    width: 20,
    height: 20,
    viewBox: '0 0 24 24',
    fill: 'none',
    stroke: 'currentColor',
    strokeWidth: 2.4,
    strokeLinecap: 'round' as const,
    strokeLinejoin: 'round' as const,
    'aria-hidden': true,
  }
  if (s === 'ok') return <svg {...comum}><path d="m5 12 5 5 9-10" /></svg>
  if (s === 'warn') return <svg {...comum}><path d="M12 6v8M12 18h.01" /></svg>
  if (s === 'checking') return <svg {...comum} className="zp-spin"><path d="M12 3a9 9 0 1 0 9 9" /></svg>
  // 'soon': traço neutro, sem alarme
  return <svg {...comum}><path d="M5 12h14" /></svg>
}

export function CardCheck({ check, verificando }: CardCheckProps): ReactNode {
  const s: Estado = verificando ? 'checking' : check.status
  const col = s === 'ok' ? '#3BF08A' : s === 'warn' ? '#FFB547' : '#A3BDB1'
  const bg = s === 'ok' ? 'rgba(59,240,138,0.12)' : s === 'warn' ? 'rgba(255,181,71,0.12)' : '#16241F'

  const secao: CSSProperties = {
    flex: '1 1 440px', minWidth: 0, display: 'flex', flexDirection: 'column', gap: 14,
    padding: '20px 22px', borderRadius: 18, background: 'var(--superficie)', border: '1px solid var(--borda)',
  }
  const icone: CSSProperties = {
    flex: '0 0 40px', width: 40, height: 40, borderRadius: 12,
    display: 'flex', alignItems: 'center', justifyContent: 'center', color: col, background: bg,
  }
  const selo: CSSProperties = {
    height: 28, padding: '0 10px', borderRadius: 999, fontSize: 12, fontWeight: 600,
    display: 'flex', alignItems: 'center', color: col, background: bg,
  }

  return (
    <section data-estado={s} style={secao}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
        <span style={icone}><Icone s={s} /></span>
        <h2 style={{ margin: 0, flex: 1, fontSize: 16, fontWeight: 700 }}>{check.title}</h2>
        <span style={selo}>{TEXTO_STATUS[s]}</span>
      </div>
      <ul style={{ listStyle: 'none', margin: 0, padding: 0, display: 'flex', flexDirection: 'column', gap: 10 }}>
        {check.lines.map((l, i) => (
          <li key={i} style={{ display: 'flex', alignItems: 'flex-start', gap: 10 }}>
            <span
              data-testid="dot"
              style={{
                flex: '0 0 8px', width: 8, height: 8, marginTop: 7, borderRadius: '50%',
                background: TOM[l.tom],
                ...(l.tom === 'ok' ? { boxShadow: '0 0 8px #3BF08A' } : {}),
              }}
            />
            <span style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', gap: 2 }}>
              <span style={{ lineHeight: 1.4 }}>{l.text}</span>
              {l.sub && <span style={{ fontSize: 12.5, color: '#8BA498', lineHeight: 1.4 }}>{l.sub}</span>}
            </span>
            {l.badge && (
              <span
                style={{
                  flex: '0 0 auto', height: 26, padding: '0 10px', borderRadius: 999, fontSize: 12, fontWeight: 600,
                  display: 'flex', alignItems: 'center', whiteSpace: 'nowrap',
                  color: TOM[l.tom], background: bgBadge(l.tom),
                }}
              >
                {l.badge}
              </span>
            )}
          </li>
        ))}
      </ul>
    </section>
  )
}
