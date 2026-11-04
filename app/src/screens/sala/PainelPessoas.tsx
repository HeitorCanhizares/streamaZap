// Tarefa sala-06: aba Pessoas do painel (design: lista `people`, 3-sala.html 196-260 e script).
import type { ReactNode } from 'react'
import type { Participant } from '../../bridge/types'
import { Avatar } from '../../components/Avatar'
import { Ping } from '../../components/Ping'

export interface PainelPessoasProps {
  participants: Participant[]
  meId: string
  hostId?: string
}

const MONO = "'JetBrains Mono', monospace"

// Status em texto: a tela tem prioridade sobre a webcam; sem nenhum dos dois, só assiste.
function statusDe(p: Participant): string {
  if (p.sharing) return 'compartilhando a tela'
  if (p.camera) return 'webcam ligada'
  return 'assistindo'
}

export function PainelPessoas({ participants, meId, hostId }: PainelPessoasProps): ReactNode {
  return (
    <ul style={{ listStyle: 'none', margin: 0, padding: '8px 0', flex: 1, overflowY: 'auto' }}>
      {participants.map((p) => {
        const eu = p.id === meId
        // Sem hostId não há host marcado: o selo só aparece quando o id bate.
        const host = hostId !== undefined && p.id === hostId
        return (
          <li
            key={p.id}
            style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '12px 16px', borderBottom: '1px solid #18292A' }}
          >
            <Avatar name={p.name} color={p.color} size={40} />
            <span style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', gap: 3 }}>
              <span style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <b style={{ fontWeight: 600 }}>{eu ? `${p.name} (você)` : p.name}</b>
                {host && (
                  <span
                    style={{
                      padding: '1px 7px',
                      borderRadius: 6,
                      background: 'rgba(255,181,71,0.15)',
                      color: '#FFB547',
                      fontSize: 10.5,
                      fontWeight: 700,
                      letterSpacing: '0.06em',
                    }}
                  >
                    host
                  </span>
                )}
              </span>
              <span style={{ fontSize: 12, color: '#A3BDB1' }}>{statusDe(p)}</span>
            </span>
            <span style={{ display: 'flex', flexDirection: 'column', alignItems: 'flex-end', gap: 2, fontFamily: MONO, fontSize: 11.5 }}>
              <Ping ms={p.ping_ms} />
            </span>
          </li>
        )
      })}
    </ul>
  )
}
