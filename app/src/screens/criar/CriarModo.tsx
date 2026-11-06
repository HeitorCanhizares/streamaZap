import type { ReactNode } from 'react'
import type { RoomMode } from '../../bridge/types'
import { useOnlineEnabled } from '../../bridge/onlineStore'
import { MODOS, type ModoInfo } from './criarModel'

export interface CriarModoProps { modo: RoomMode; onModo: (m: RoomMode) => void }

// Seção 01 "Onde a sala vai rodar" (2-criar-sala.html 68-85).

const MONO = "'JetBrains Mono', monospace"

// Com o Online desligado a dica do Local não cita o servidor (a seção some o Online por completo).
const HINT_LOCAL_SEM_ONLINE = 'Direto entre os PCs, pelo Radmin VPN ou pela rede de casa.'

function Icone({ info }: { info: ModoInfo }): ReactNode {
  // Ícone 44x44 com fundo translúcido da cor do modo (o tom fica igual ao do protótipo).
  const fundo = info.value === 'online' ? 'rgba(59,240,138,0.12)' : 'rgba(92,214,255,0.12)'
  return (
    <span
      style={{
        flex: '0 0 44px', width: 44, height: 44, borderRadius: 12,
        display: 'flex', alignItems: 'center', justifyContent: 'center',
        color: info.cor, background: fundo,
      }}
    >
      {info.value === 'online' ? (
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" aria-hidden="true">
          <circle cx="12" cy="12" r="9" />
          <path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18" />
        </svg>
      ) : (
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" aria-hidden="true">
          <path d="M2 9a15 15 0 0 1 20 0M5 12.5a10 10 0 0 1 14 0M8.5 16a5 5 0 0 1 7 0M12 19.5h.01" />
        </svg>
      )}
    </span>
  )
}

export function CriarModo({ modo, onModo }: CriarModoProps): ReactNode {
  const onlineLigado = useOnlineEnabled()
  // Online desligado: só o cartão Local, em uma coluna.
  const modos = onlineLigado ? MODOS : MODOS.filter((m) => m.value === 'local')
  return (
    <section
      style={{
        display: 'flex', flexDirection: 'column', gap: 12, padding: 20,
        borderRadius: 18, background: 'var(--superficie)', border: '1px solid var(--borda)',
      }}
    >
      <h2 style={{ margin: 0, display: 'flex', alignItems: 'baseline', gap: 10, fontSize: 14, fontWeight: 600 }}>
        <span style={{ fontFamily: MONO, fontSize: 11, color: 'var(--verde)' }}>01</span>
        Onde a sala vai rodar
      </h2>
      <div
        role="radiogroup"
        aria-label="Modo da sala"
        style={{
          display: 'grid',
          gridTemplateColumns: onlineLigado ? 'repeat(2, minmax(0, 1fr))' : 'minmax(0, 1fr)',
          gap: 12,
        }}
      >
        {modos.map((info) => {
          const ativo = modo === info.value
          const sombra = info.value === 'online' ? 'rgba(59,240,138,0.15)' : 'rgba(92,214,255,0.15)'
          return (
            <button
              key={info.value}
              type="button"
              className="zp-card"
              role="radio"
              aria-checked={ativo}
              onClick={() => onModo(info.value)}
              style={{
                display: 'flex', alignItems: 'flex-start', gap: 14, padding: 16, borderRadius: 16,
                textAlign: 'left',
                background: ativo ? 'rgba(59,240,138,0.07)' : 'var(--bg)',
                border: `1px solid ${ativo ? info.cor : 'var(--borda)'}`,
                boxShadow: ativo ? `0 0 18px ${sombra}` : 'none',
                color: 'inherit', font: 'inherit', cursor: 'pointer',
              }}
            >
              <Icone info={info} />
              <span style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', gap: 4 }}>
                <span style={{ fontWeight: 700, fontSize: 15 }}>{info.label}</span>
                <span style={{ fontSize: 12.5, color: 'var(--texto-2)', lineHeight: 1.4 }}>
                  {onlineLigado ? info.hint : HINT_LOCAL_SEM_ONLINE}
                </span>
                <span style={{ fontFamily: MONO, fontSize: 11, color: ativo ? info.cor : 'var(--texto-3)' }}>{info.tech}</span>
              </span>
            </button>
          )
        })}
      </div>
    </section>
  )
}
