// Tarefa salas-11
import type { ReactNode } from 'react'
import { corPing, estiloAvatar } from './model'
import type { SalaVM } from './model'

export interface SalaNaSalaProps { sala: SalaVM }

// Lista de pessoas na sala com o ping de cada uma (1-salas.html linhas 237-252).
export function SalaNaSala({ sala }: SalaNaSalaProps): ReactNode {
  const pessoas = sala.pessoas
  return (
    <section style={{ flex: '1 1 280px', minWidth: 0, display: 'flex', flexDirection: 'column', gap: 12 }}>
      <h3 style={{ margin: 0, fontFamily: "'JetBrains Mono', monospace", fontWeight: 400, fontSize: 11, letterSpacing: '0.14em', textTransform: 'uppercase', color: '#7E978B' }}>
        Na sala · {pessoas.length}
      </h3>
      <ul style={{ listStyle: 'none', margin: 0, padding: 0, display: 'flex', flexDirection: 'column', gap: 8 }}>
        {pessoas.map((p, i) => (
          <li key={`${p.name}-${i}`} style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
            <span style={estiloAvatar(p.name, 34, false)}>{p.name.slice(0, 1).toUpperCase()}</span>
            <span style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column' }}>
              <span style={{ fontWeight: 600 }}>{p.name}</span>
              <span style={{ fontSize: 12, color: '#7E978B' }}>{p.role}</span>
            </span>
            <span style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: 12, color: corPing(p.ping_ms) }}>{p.ping_ms} ms</span>
          </li>
        ))}
      </ul>
    </section>
  )
}
