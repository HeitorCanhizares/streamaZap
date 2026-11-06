// Tarefa sala-07: painel lateral da sala com cabeçalho de abas (Chat / Pessoas) e botão fechar.
// Porte de docs/design/3-sala.html 196-223. O corpo é PainelChat (sala-05) ou PainelPessoas (sala-06).
import { useState, type ReactNode } from 'react'
import type { RoomView } from '../../bridge/types'
import { api } from '../../bridge/api'
import { Badge } from '../../components/Badge'
import { PainelChat } from './PainelChat'
import { PainelPessoas } from './PainelPessoas'
import { PainelMusica } from './musica/PainelMusica'

export interface SalaPainelProps {
  room: RoomView
  onSend: (text: string) => void
  // install_id do próprio usuário. Sem ele (vazio), nenhuma mensagem vira "minha": tudo fica como entrada.
  meId?: string
  onClose?: () => void
  // Avisa a sala que eu comecei/parei de digitar. Padrão: chama api.chatTyping.
  onTyping?: (typing: boolean) => void
}

// Padrão do aviso de digitação: falha de rede não deve quebrar a digitação, então engole o erro.
const digitarPadrao = (typing: boolean): void => {
  void api.chatTyping(typing).catch(() => {})
}

type Aba = 'chat' | 'pessoas' | 'musica'

// Abas no estilo do design: 36px, raio 9; a ativa ganha fundo escuro e texto verde.
const ABA_BASE = {
  flex: 1,
  height: 36,
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  gap: 6,
  borderRadius: 9,
  border: 0,
  background: 'transparent',
  color: '#A3BDB1',
  fontSize: 13.5,
  fontWeight: 600,
  cursor: 'pointer',
} as const
const ABA_ATIVA = { background: '#1A3A2C', color: '#3BF08A' } as const

export function SalaPainel({ room, onSend, meId = '', onClose, onTyping = digitarPadrao }: SalaPainelProps): ReactNode {
  const [aba, setAba] = useState<Aba>('chat')

  const abas: { value: Aba; label: string; badge: number }[] = [
    { value: 'chat', label: 'Chat', badge: room.unread },
    { value: 'pessoas', label: `Pessoas · ${room.participants.length}`, badge: 0 },
    { value: 'musica', label: 'Música', badge: room.music?.queue.length ?? 0 },
  ]

  return (
    <aside
      aria-label="Chat, pessoas e música"
      style={{
        flex: '1 1 340px',
        maxWidth: 400,
        minWidth: 0,
        minHeight: 0,
        alignSelf: 'stretch',
        display: 'flex',
        flexDirection: 'column',
        background: '#0F1916',
        borderLeft: '1px solid #1F332D',
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '12px 12px 12px 16px', borderBottom: '1px solid #1F332D' }}>
        <div
          role="tablist"
          aria-label="Painel"
          style={{ flex: 1, display: 'flex', gap: 4, padding: 4, borderRadius: 12, background: '#0A100E', border: '1px solid #1F332D' }}
        >
          {abas.map((a) => {
            const ativo = a.value === aba
            return (
              <button
                key={a.value}
                type="button"
                role="tab"
                aria-selected={ativo}
                onClick={() => setAba(a.value)}
                style={ativo ? { ...ABA_BASE, ...ABA_ATIVA } : ABA_BASE}
              >
                {a.label}
                {a.badge > 0 && <Badge tone="blue">{a.badge}</Badge>}
              </button>
            )
          })}
        </div>
        <button
          type="button"
          className="zp-ghost"
          onClick={onClose}
          aria-label="Fechar painel"
          title="Fechar painel"
          style={{ width: 44, height: 44, borderRadius: 12, border: 0, background: 'transparent', color: '#A3BDB1', display: 'flex', alignItems: 'center', justifyContent: 'center', cursor: 'pointer' }}
        >
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden="true">
            <path d="M6 6l12 12M18 6 6 18" />
          </svg>
        </button>
      </div>

      {aba === 'chat' ? (
        <PainelChat chat={room.chat} meId={meId} onSend={onSend} onTyping={onTyping} />
      ) : aba === 'pessoas' ? (
        <PainelPessoas participants={room.participants} meId={meId} />
      ) : (
        <PainelMusica room={room} meId={meId} />
      )}
    </aside>
  )
}
