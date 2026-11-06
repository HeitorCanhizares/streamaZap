// Bloco "Como ler isso" (5-diagnostico.html linhas 121-127).
import type { ReactNode } from 'react'

const pStyle = { margin: 0, color: '#C6D9CF', lineHeight: 1.55 } as const
const bStyle = { color: 'var(--texto)' } as const

// Decisão: com Online desligado o parágrafo sobre o servidor some (a tela não cita o modo Online).
export function ComoLer({ online = true }: { online?: boolean }): ReactNode {
  return (
    <section
      aria-labelledby="h-ler"
      style={{
        flex: '1 1 440px',
        minWidth: 0,
        display: 'flex',
        flexDirection: 'column',
        gap: 12,
        padding: '20px 22px',
        borderRadius: 18,
        background: 'rgba(92,214,255,0.06)',
        border: '1px solid rgba(92,214,255,0.25)',
      }}
    >
      <h2
        id="h-ler"
        style={{
          margin: 0,
          display: 'flex',
          alignItems: 'center',
          gap: 10,
          fontSize: 16,
          fontWeight: 700,
        }}
      >
        <svg
          width="20"
          height="20"
          viewBox="0 0 24 24"
          fill="none"
          stroke="var(--azul)"
          strokeWidth={1.8}
          strokeLinecap="round"
          aria-hidden="true"
        >
          <circle cx={12} cy={12} r={9} />
          <path d="M12 11v6M12 7.5h.01" />
        </svg>
        Como ler isso
      </h2>
      <p style={pStyle}>
        Se a sala de alguém aparece mas dá <b style={bStyle}>tempo esgotado</b>, e do lado do nome dele
        está “não recebe nada do seu PC”, o bloqueio está no PC <b style={bStyle}>dele</b>.
      </p>
      <p style={pStyle}>
        Teste rápido no PC bloqueado: desligue o Firewall do Windows por 1 minuto. Funcionou? É o
        firewall, use <b style={bStyle}>Corrigir firewall</b>. Não funcionou? É antivírus ou o Radmin —
        veja se a conexão no Radmin está “direta”.
      </p>
      {online && (
        <p style={pStyle}>
          No modo <b style={bStyle}>Online</b> nada disso importa tanto: o servidor faz a ponte quando a
          conexão direta falha.
        </p>
      )}
    </section>
  )
}
