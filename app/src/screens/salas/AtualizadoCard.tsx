// "Atualizado para a vX" com as notas, na tela Salas (plano §13). Tarefa front-14.
import type { ReactNode } from 'react'
import { dispensarAplicada, useUpdateAplicada } from '../../bridge/updateAvisoStore'

/** Remove o "v" inicial para não duplicar ao prefixar. */
function semV(v: string): string {
  return v.replace(/^v/i, '')
}

export function AtualizadoCard(): ReactNode {
  const info = useUpdateAplicada()
  if (!info) return null
  const notas = info.notas && info.notas.trim() !== '' ? info.notas : null

  return (
    <section
      data-testid="atualizado-card"
      aria-label="Atualização aplicada"
      style={{
        margin: '16px 32px 0',
        padding: '14px 18px',
        borderRadius: 14,
        background: 'rgba(59,240,138,0.08)',
        border: '1px solid rgba(59,240,138,0.35)',
        display: 'flex',
        alignItems: 'flex-start',
        gap: 12,
        color: 'var(--texto)',
      }}
    >
      <svg width={18} height={18} viewBox="0 0 24 24" fill="none" aria-hidden="true" style={{ flex: 'none', marginTop: 2 }}>
        <path d="m5 12 5 5 9-10" stroke="var(--verde)" strokeWidth={2.4} strokeLinecap="round" strokeLinejoin="round" />
      </svg>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'baseline', gap: 10 }}>
          <strong>{`Atualizado para a v${semV(info.versao)}`}</strong>
          <span style={{ fontFamily: 'var(--font-mono)', fontSize: 12, color: 'var(--texto-3)' }}>
            {`Antes: v${semV(info.anterior)}`}
          </span>
        </div>
        {notas && (
          <pre
            style={{
              margin: '8px 0 0',
              whiteSpace: 'pre-wrap',
              fontSize: 13,
              color: 'var(--texto-2)',
              maxHeight: 160,
              overflow: 'auto',
              fontFamily: 'inherit',
            }}
          >
            {notas}
          </pre>
        )}
      </div>
      <button
        type="button"
        aria-label="Fechar aviso de atualização"
        onClick={() => dispensarAplicada()}
        style={{
          flex: 'none',
          height: 32,
          padding: '0 12px',
          borderRadius: 8,
          border: '1px solid rgba(59,240,138,0.35)',
          background: 'transparent',
          color: 'var(--texto)',
        }}
      >
        Fechar
      </button>
    </section>
  )
}
