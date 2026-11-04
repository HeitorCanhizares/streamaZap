// Tarefa ui-21: painel lateral da sala (chat e pessoas). Porte de docs/design/3-sala.html.
import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from 'react'
import type { RoomView } from '../../bridge/types'
import { Avatar } from '../../components/Avatar'
import { Badge } from '../../components/Badge'
import { Button } from '../../components/Button'
import { Ping } from '../../components/Ping'
import { TextField } from '../../components/TextField'

export interface SalaPainelProps { room: RoomView; onSend: (text: string) => void }

type Aba = 'chat' | 'pessoas'

const MONO = { fontFamily: "'JetBrains Mono', monospace" } as const

// Hora HH:MM a partir de ts em segundos epoch. Data inválida vira '--:--' em vez de quebrar a tela.
function hora(ts: number): string {
  const d = new Date(ts * 1000)
  if (Number.isNaN(d.getTime())) return '--:--'
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
}

export function SalaPainel({ room, onSend }: SalaPainelProps): ReactNode {
  const [aba, setAba] = useState<Aba>('chat')
  const [texto, setTexto] = useState('')
  const listaRef = useRef<HTMLDivElement | null>(null)

  // Rola para o fim quando chegam mensagens (ou ao voltar para a aba do chat).
  useEffect(() => {
    const lista = listaRef.current
    if (aba === 'chat' && lista) lista.scrollTop = lista.scrollHeight
  }, [aba, room.chat.length])

  const enviar = () => {
    const limpo = texto.trim()
    if (limpo === '') return
    onSend(limpo)
    setTexto('')
  }

  const aoTeclar = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      enviar()
    }
  }

  const abas: { value: Aba; label: string; badge: number }[] = [
    { value: 'chat', label: 'Chat', badge: room.unread },
    { value: 'pessoas', label: 'Pessoas', badge: 0 },
  ]

  return (
    <aside aria-label="Chat e pessoas" style={{ display: 'flex', flexDirection: 'column', minHeight: 0, height: '100%', background: '#0F1916', borderLeft: '1px solid #1F332D' }}>
      <div style={{ padding: '12px 16px', borderBottom: '1px solid #1F332D' }}>
        {/* Mesmo visual do Segmented (zp-segmented), com selo de não lidas na aba Chat. */}
        <div className="zp-segmented" role="tablist" aria-label="Painel">
          {abas.map((a) => {
            const ativo = a.value === aba
            return (
              <button
                key={a.value}
                type="button"
                role="tab"
                aria-selected={ativo}
                className={'zp-seg-item' + (ativo ? ' zp-seg-active' : '')}
                onClick={() => setAba(a.value)}
              >
                {a.label}
                {a.badge > 0 && <Badge tone="blue">{a.badge}</Badge>}
              </button>
            )
          })}
        </div>
      </div>

      {aba === 'chat' ? (
        <>
          <div
            ref={listaRef}
            className="zp-dots"
            role="log"
            aria-label="Mensagens"
            style={{ flex: 1, minHeight: 0, overflowY: 'auto', display: 'flex', flexDirection: 'column', gap: 8, padding: '16px 14px' }}
          >
            {room.chat.map((m) => (
              <div
                key={m.id}
                style={{ alignSelf: 'flex-start', maxWidth: '82%', display: 'flex', flexDirection: 'column', gap: 2, padding: '7px 10px 6px 12px', borderRadius: '4px 16px 16px 16px', background: '#16241F' }}
              >
                <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                  <Avatar name={m.author} color={m.color} size={20} />
                  <b style={{ fontWeight: 600, color: m.color }}>{m.author}</b>
                  <span style={{ ...MONO, fontSize: 10.5, color: '#8BA498' }}>{hora(m.ts)}</span>
                </div>
                <span style={{ lineHeight: 1.4, fontSize: 14, whiteSpace: 'pre-wrap' }}>{m.text}</span>
              </div>
            ))}
          </div>
          <div
            onKeyDown={aoTeclar}
            style={{ display: 'flex', alignItems: 'center', gap: 8, padding: 12, borderTop: '1px solid #1F332D' }}
          >
            <div style={{ flex: 1, minWidth: 0 }}>
              <TextField value={texto} onChange={setTexto} placeholder="Mensagem" />
            </div>
            <Button onClick={enviar}>Enviar</Button>
          </div>
        </>
      ) : (
        <ul style={{ listStyle: 'none', margin: 0, padding: '8px 0', flex: 1, minHeight: 0, overflowY: 'auto' }}>
          {room.participants.map((p) => (
            <li
              key={p.id}
              style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '12px 16px', borderBottom: '1px solid #18292A' }}
            >
              <Avatar name={p.name} color={p.color} />
              <span style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', gap: 4 }}>
                <span style={{ display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
                  <b style={{ fontWeight: 600 }}>{p.name}</b>
                  {p.sharing && <Badge tone="green">transmitindo</Badge>}
                  {p.camera && <Badge tone="blue">câmera</Badge>}
                </span>
              </span>
              <Ping ms={p.ping_ms} />
            </li>
          ))}
        </ul>
      )}
    </aside>
  )
}
