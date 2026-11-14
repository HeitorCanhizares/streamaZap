// Barra da sala: o dock no pé da coluna, fora das transmissões e sempre visível. Só tem o que é da
// sala (minha câmera, minha transmissão, tela cheia do app e sair). O som, a suavização e a tela
// cheia de cada transmissão ficam na própria transmissão (SalaPrincipal).
import type { CSSProperties, ReactNode } from 'react'

export interface SalaControlesProps {
  camOn: boolean
  sharing: boolean
  onFullscreen: () => void
  onToggleCam: () => void
  onToggleShare: () => void
  onLeave: () => void
  /** Plano sem transmissão: 'Vire Transmissor' no lugar de compartilhar (padrão true). */
  podeTransmitir?: boolean
}

const redondo: CSSProperties = {
  width: 44,
  height: 44,
  borderRadius: '50%',
  border: 0,
  background: '#16241F',
  color: '#E6F4EC',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  padding: 0,
  flex: '0 0 auto',
}

const pilula: CSSProperties = {
  height: 44,
  padding: '0 18px',
  borderRadius: 999,
  border: 0,
  fontWeight: 700,
  display: 'flex',
  alignItems: 'center',
  gap: 8,
  whiteSpace: 'nowrap',
  flex: '0 0 auto',
}

const separador: CSSProperties = { width: 1, height: 28, background: '#2A443C', margin: '0 4px', flex: '0 0 1px' }

export function SalaControles(props: SalaControlesProps): ReactNode {
  const { camOn, sharing } = props
  // Plano sem transmissão: só dá para "virar Transmissor" (abre o painel) e a câmera fica travada.
  const travado = props.podeTransmitir === false && !sharing
  const shareTxt = sharing ? 'Transmissão' : travado ? 'Vire Transmissor' : 'Compartilhar'
  const camLabel = camOn ? 'Desligar minha câmera' : 'Ligar minha câmera'
  const camTitle = travado ? 'Vire Transmissor para ligar a câmera' : `${camLabel} (C)`
  const camStyle: CSSProperties = camOn ? redondo : { ...redondo, background: '#3A1C21', color: '#FF8592' }
  const shareStyle: CSSProperties = {
    ...pilula,
    background: sharing ? '#3BF08A' : '#16241F',
    color: sharing ? '#03140A' : '#E6F4EC',
  }
  const svgBase = { fill: 'none', stroke: 'currentColor', strokeWidth: 1.8, 'aria-hidden': true } as const

  return (
    <div
      role="toolbar"
      aria-label="Controles da sala"
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 8,
        padding: 8,
        maxWidth: '100%',
        borderRadius: 30,
        background: '#0F1916',
        border: '1px solid #2A443C',
        boxShadow: '0 12px 40px rgba(0,0,0,0.45)',
      }}
    >
      <button type="button" className="zp-dock" onClick={travado ? undefined : props.onToggleCam} aria-label={camLabel} aria-disabled={travado ? true : undefined} title={camTitle} style={travado ? { ...camStyle, opacity: 0.5, cursor: 'not-allowed' } : camStyle}>
        <svg width="20" height="20" viewBox="0 0 24 24" strokeLinejoin="round" strokeLinecap={camOn ? undefined : 'round'} {...svgBase}>
          <rect x="3" y="7" width="13" height="10" rx="2" />
          {camOn ? <path d="m16 11 5-3v8l-5-3z" /> : <path d="m16 11 5-3v8l-5-3zM3 3l18 18" />}
        </svg>
      </button>
      <button type="button" className="zp-dock" onClick={props.onToggleShare} aria-pressed={sharing} aria-label={shareTxt} title={sharing ? 'Ajustar a transmissão' : shareTxt} style={shareStyle}>
        <svg width="20" height="20" viewBox="0 0 24 24" strokeLinecap="round" strokeLinejoin="round" {...svgBase}>
          <rect x="3" y="4" width="18" height="12" rx="2" />
          <path d="M8 20h8M12 16v4M12 13V7M9 10l3-3 3 3" />
        </svg>
        <span>{shareTxt}</span>
      </button>
      <button type="button" className="zp-dock" onClick={props.onFullscreen} aria-label="Tela cheia do app (F11)" title="Tela cheia do app (F11)" style={redondo}>
        <svg width="20" height="20" viewBox="0 0 24 24" strokeLinecap="round" strokeLinejoin="round" {...svgBase}>
          <path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" />
        </svg>
      </button>
      <span aria-hidden="true" style={separador} />
      <button type="button" className="zp-leave" onClick={props.onLeave} style={{ ...pilula, background: '#FF5D6E', color: '#1A0508' }}>
        <svg width="18" height="18" viewBox="0 0 24 24" strokeLinecap="round" strokeLinejoin="round" fill="none" stroke="currentColor" strokeWidth={2} aria-hidden="true">
          <path d="M15 4h3a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-3M10 8l-4 4 4 4M6 12h10" />
        </svg>
        Sair
      </button>
    </div>
  )
}
