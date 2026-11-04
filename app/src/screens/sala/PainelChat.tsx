// Tarefa sala-05: aba Chat do painel (3-sala.html 224-260).
import { useEffect, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import type { ChatLine } from '../../bridge/types'

export interface PainelChatProps {
  chat: ChatLine[]
  meId: string
  onSend: (text: string) => void
  typing?: string
  // Avisa a sala que eu comecei/parei de digitar (chat_typing).
  onTyping?: (typing: boolean) => void
}

const MONO = "'JetBrains Mono', monospace"

// Sem digitar por esse tempo, considera que parou.
const PARADO_MS = 3000

// Hora local HH:MM; timestamp inválido vira '--:--'.
function hora(ts: number): string {
  const d = new Date(ts)
  if (!Number.isFinite(ts) || Number.isNaN(d.getTime())) return '--:--'
  const p = (n: number) => String(n).padStart(2, '0')
  return `${p(d.getHours())}:${p(d.getMinutes())}`
}

const ponto = { width: 6, height: 6, borderRadius: '50%', background: '#A3BDB1' } as const

export function PainelChat({ chat, meId, onSend, typing, onTyping }: PainelChatProps): ReactNode {
  const [draft, setDraft] = useState('')
  const fim = useRef<HTMLDivElement>(null)
  // Estado "digitando" fica em ref: o aviso sai uma vez por rodada, não a cada tecla.
  const digitandoRef = useRef(false)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    // jsdom não implementa scrollIntoView
    fim.current?.scrollIntoView?.({ block: 'end' })
  }, [chat.length])

  // Limpa o timer no unmount sem avisar a sala.
  useEffect(() => {
    return () => {
      if (timerRef.current !== null) clearTimeout(timerRef.current)
    }
  }, [])

  // Encerra a rodada de digitação: cancela o timer e avisa só se estava digitando.
  function parar(): void {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current)
      timerRef.current = null
    }
    if (digitandoRef.current) {
      digitandoRef.current = false
      onTyping?.(false)
    }
  }

  function aoMudar(valor: string): void {
    setDraft(valor)
    if (!valor) {
      parar()
      return
    }
    if (!digitandoRef.current) {
      digitandoRef.current = true
      onTyping?.(true)
    }
    if (timerRef.current !== null) clearTimeout(timerRef.current)
    timerRef.current = setTimeout(() => {
      timerRef.current = null
      parar()
    }, PARADO_MS)
  }

  function enviar(): void {
    const t = draft.trim()
    if (!t) return
    parar()
    onSend(t)
    setDraft('')
  }

  return (
    <>
      <div
        className="zp-dots"
        role="log"
        aria-label="Mensagens"
        style={{ flex: 1, minHeight: 0, overflowY: 'auto', display: 'flex', flexDirection: 'column', gap: 8, padding: '16px 14px' }}
      >
        <span style={{ marginTop: 'auto', alignSelf: 'center', padding: '4px 12px', borderRadius: 8, background: '#16241F', fontFamily: MONO, fontSize: 11, letterSpacing: '0.12em', color: '#A3BDB1' }}>HOJE</span>
        {chat.map((m) => {
          if (m.author_id === 'system' || !m.author) {
            return (
              <span key={m.id} style={{ alignSelf: 'center', maxWidth: '90%', padding: '5px 12px', borderRadius: 8, background: 'rgba(22,36,31,0.9)', color: '#A3BDB1', fontSize: 12, textAlign: 'center' }}>{m.text}</span>
            )
          }
          if (m.author_id === meId) {
            return (
              <div key={m.id} style={{ alignSelf: 'flex-end', maxWidth: '82%', display: 'flex', flexDirection: 'column', gap: 2, padding: '7px 10px 6px 12px', borderRadius: '16px 4px 16px 16px', background: '#114A30', boxShadow: '0 1px 0 rgba(0,0,0,0.25)' }}>
                <span style={{ lineHeight: 1.4, fontSize: 14 }}>{m.text}</span>
                <span style={{ alignSelf: 'flex-end', display: 'flex', alignItems: 'center', gap: 4, fontFamily: MONO, fontSize: 10.5, color: '#A8D9BF' }}>
                  {hora(m.ts)}
                  <span aria-hidden="true">✓✓</span>
                </span>
              </div>
            )
          }
          return (
            <div key={m.id} style={{ alignSelf: 'flex-start', maxWidth: '82%', display: 'flex', flexDirection: 'column', gap: 2, padding: '7px 10px 6px 12px', borderRadius: '4px 16px 16px 16px', background: '#16241F', boxShadow: '0 1px 0 rgba(0,0,0,0.25)' }}>
              <span style={{ fontWeight: 700, fontSize: 12.5, color: m.color }}>{m.author}</span>
              <span style={{ lineHeight: 1.4, fontSize: 14 }}>{m.text}</span>
              <span style={{ alignSelf: 'flex-end', fontFamily: MONO, fontSize: 10.5, color: '#8BA498' }}>{hora(m.ts)}</span>
            </div>
          )
        })}
        {typing ? (
          <div style={{ alignSelf: 'flex-start', display: 'flex', alignItems: 'center', gap: 10 }}>
            <span data-testid="digitando" style={{ display: 'flex', gap: 4, padding: '11px 12px', borderRadius: '4px 16px 16px 16px', background: '#16241F' }}>
              <span className="zp-d" style={ponto} />
              <span className="zp-d zp-d2" style={ponto} />
              <span className="zp-d zp-d3" style={ponto} />
            </span>
            <span style={{ fontSize: 12, color: '#7CF2C0', fontStyle: 'italic' }}>{typing} está digitando…</span>
          </div>
        ) : null}
        <div ref={fim} />
      </div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: 12, borderTop: '1px solid #1F332D' }}>
        <input
          aria-label="Mensagem"
          placeholder="Mensagem"
          value={draft}
          onChange={(e) => aoMudar(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') enviar()
          }}
          style={{ flex: 1, minWidth: 0, height: 46, padding: '0 16px', borderRadius: 999, border: '1px solid #1F332D', background: '#0A100E', color: '#E6F4EC', fontSize: 14 }}
        />
        <button
          type="button"
          className="zp-send"
          onClick={enviar}
          aria-label="Enviar mensagem"
          style={{ flex: '0 0 46px', width: 46, height: 46, borderRadius: '50%', border: 0, background: '#3BF08A', color: '#03140A', display: 'flex', alignItems: 'center', justifyContent: 'center' }}
        >
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <path d="M4 12 20 4l-4 16-4-7z" />
            <path d="m12 13 8-9" />
          </svg>
        </button>
      </div>
    </>
  )
}
