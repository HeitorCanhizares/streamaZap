// Tarefa salas-04: item da lista de salas (design 1-salas.html linhas 96-125).
import type { ReactNode } from 'react'
import { IconeCadeado, IconeOlho } from './icons'
import { corPing, estiloAvatar } from './model'
import type { SalaVM } from './model'

export interface SalaItemProps { sala: SalaVM; selecionada: boolean; onSelect: (id: string) => void }

const elipse = { whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' } as const

export function SalaItem({ sala, selecionada, onSelect }: SalaItemProps): ReactNode {
  const temNaoLidas = sala.naoLidas > 0
  return (
    <button
      type="button"
      className="zp-row"
      aria-pressed={selecionada}
      onClick={() => onSelect(sala.id)}
      style={{
        width: '100%',
        display: 'flex',
        gap: 14,
        alignItems: 'flex-start',
        padding: '14px 20px',
        border: 0,
        borderBottom: '1px solid #18292A',
        textAlign: 'left',
        color: 'inherit',
        ...(selecionada ? { background: '#12291F' } : {}),
      }}
    >
      <span style={estiloAvatar(sala.host, 48, true)}>{sala.host.slice(0, 1).toUpperCase()}</span>
      <span style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', gap: 4 }}>
        <span style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          <span style={{ fontWeight: 600, fontSize: 15, ...elipse }}>{sala.nome}</span>
          {sala.trancada ? <IconeCadeado size={14} cor="#A3BDB1" /> : null}
          <span style={{ flex: 1 }}></span>
          <span style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: 11, color: temNaoLidas ? '#3BF08A' : '#7E978B' }}>{sala.hora}</span>
        </span>
        <span style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          {sala.digitando ? (
            <span style={{ color: '#3BF08A', fontStyle: 'italic', fontSize: 13 }}>{sala.host} está digitando…</span>
          ) : (
            <span style={{ color: '#A3BDB1', fontSize: 13, ...elipse }}>{sala.ultima}</span>
          )}
          <span style={{ flex: 1 }}></span>
          {temNaoLidas ? (
            <span
              style={{
                minWidth: 22, height: 22, padding: '0 7px', borderRadius: 999, background: '#3BF08A', color: '#03140A',
                fontSize: 12, fontWeight: 700, display: 'flex', alignItems: 'center', justifyContent: 'center',
              }}
            >{sala.naoLidas}</span>
          ) : null}
        </span>
        <span style={{ display: 'flex', alignItems: 'center', gap: 10, fontFamily: "'JetBrains Mono', monospace", fontSize: 11, color: '#7E978B' }}>
          <span style={{ display: 'flex', alignItems: 'center', gap: 4 }}><IconeOlho size={13} />{sala.assistindo}</span>
          <span>{sala.qualidade}</span>
          <span style={{ color: corPing(sala.ping) }}>{sala.ping} ms</span>
          {sala.versaoAviso ? <span style={{ color: '#FFB547' }}>v{sala.versaoAviso}</span> : null}
        </span>
      </span>
    </button>
  )
}
