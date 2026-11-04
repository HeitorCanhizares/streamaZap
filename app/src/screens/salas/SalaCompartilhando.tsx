// Tarefa salas-10
import type { ReactNode } from 'react'
import { ScenePreview } from '../../components/ScenePreview'
import type { SalaVM } from './model'

export interface SalaCompartilhandoProps { sala: SalaVM }

// Grade de miniaturas de quem compartilha agora (1-salas.html linhas 223-236).
export function SalaCompartilhando({ sala }: SalaCompartilhandoProps): ReactNode {
  const itens = sala.compartilhando
  return (
    <section style={{ flex: '1 1 420px', minWidth: 0, display: 'flex', flexDirection: 'column', gap: 12 }}>
      <h3 style={{ margin: 0, fontFamily: "'JetBrains Mono', monospace", fontWeight: 400, fontSize: 11, letterSpacing: '0.14em', textTransform: 'uppercase', color: '#7E978B' }}>
        Compartilhando agora · {itens.length}
      </h3>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, minmax(0, 1fr))', gap: 12 }}>
        {itens.map((item, i) => (
          <div key={`${item.name}-${i}`} style={{ display: 'flex', flexDirection: 'column', gap: 6, minWidth: 0 }}>
            <div style={{ position: 'relative', borderRadius: 12, overflow: 'hidden', border: '1px solid #1F332D' }}>
              <ScenePreview scene={item.scene} />
            </div>
            <span style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 13, minWidth: 0 }}>
              <b style={{ fontWeight: 600, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{item.name}</b>
              <span style={{ color: '#7E978B' }}>· {item.kind}</span>
            </span>
          </div>
        ))}
      </div>
    </section>
  )
}
