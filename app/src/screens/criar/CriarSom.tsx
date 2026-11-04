import type { ReactNode } from 'react'
import type { SourceItem } from '../../bridge/types'
import type { SomModo } from './criarModel'

export interface CriarSomProps { apps: SourceItem[]; sound: SomModo; onSound: (v: SomModo) => void; marcados: string[]; onMarcados: (v: string[]) => void }

// Seção 05 "Som" (2-criar-sala.html 147-166).

const MONO = "'JetBrains Mono', monospace"

const MODOS: { id: SomModo; label: string }[] = [
  { id: 'none', label: 'Sem som' },
  { id: 'all', label: 'Todo o PC' },
  { id: 'apps', label: 'Só alguns apps' },
]

export function CriarSom({ apps, sound, onSound, marcados, onMarcados }: CriarSomProps): ReactNode {
  const bloco = { display: 'flex', flexDirection: 'column' as const, gap: 12, padding: 20, borderRadius: 18, background: 'var(--superficie)', border: '1px solid var(--borda)' }

  // Alterna o id sem mutar `marcados`: sempre entrega um array novo.
  function alternar(id: string): void {
    onMarcados(marcados.includes(id) ? marcados.filter((m) => m !== id) : [...marcados, id])
  }

  return (
    <section style={{ ...bloco, flex: '1 1 320px', minWidth: 0 }}>
      <h2 style={{ margin: 0, display: 'flex', alignItems: 'baseline', gap: 10, fontSize: 14, fontWeight: 600 }}>
        <span style={{ fontFamily: MONO, fontSize: 11, color: 'var(--verde)' }}>05</span>
        Som
      </h2>
      <div role="radiogroup" aria-label="Som" style={{ display: 'flex', gap: 4, padding: 4, borderRadius: 12, background: 'var(--bg)', border: '1px solid var(--borda)' }}>
        {MODOS.map((m) => {
          const ativo = sound === m.id
          return (
            <button
              key={m.id}
              type="button"
              role="radio"
              aria-checked={ativo}
              onClick={() => onSound(m.id)}
              style={{
                flex: 1, height: 38, padding: '0 12px', borderRadius: 9, border: 0,
                fontSize: 13, fontWeight: 600, whiteSpace: 'nowrap', cursor: 'pointer',
                background: ativo ? '#1A3A2C' : 'transparent',
                color: ativo ? 'var(--verde)' : 'var(--texto-2)',
              }}
            >
              {m.label}
            </button>
          )
        })}
      </div>
      {sound === 'apps' && (
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
          {apps.map((app) => {
            const marcado = marcados.includes(app.id)
            return (
              <button
                key={app.id}
                type="button"
                aria-pressed={marcado}
                onClick={() => alternar(app.id)}
                style={{
                  height: 36, padding: '0 12px', borderRadius: 999,
                  display: 'flex', alignItems: 'center', gap: 6,
                  fontFamily: MONO, fontSize: 12, cursor: 'pointer',
                  border: `1px solid ${marcado ? 'var(--verde)' : 'var(--borda-forte)'}`,
                  background: marcado ? 'rgba(59,240,138,0.12)' : 'transparent',
                  color: marcado ? 'var(--verde)' : 'var(--texto-2)',
                }}
              >
                {marcado && (
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={3} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                    <path d="m5 12 5 5 9-10" />
                  </svg>
                )}
                {app.name}
              </button>
            )
          })}
        </div>
      )}
      <span style={{ fontSize: 12, color: 'var(--texto-3)' }}>O som do próprio streamaZap nunca vai junto.</span>
    </section>
  )
}
