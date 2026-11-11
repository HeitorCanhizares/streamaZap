// Tarefa sala-02: barra de controles em pílula (3-sala.html 123-142).
import type { CSSProperties, ReactNode } from 'react'

export interface SalaControlesProps {
  muted: boolean
  volume: number
  camOn: boolean
  sharing: boolean
  smoothingMs: number
  onFullscreen: () => void
  onToggleMute: () => void
  onVolume: (v: number) => void
  onToggleCam: () => void
  onToggleShare: () => void
  onLeave: () => void
  /** Some com o mouse parado (useOcioso); padrão: sempre visível. */
  visivel?: boolean
  /** Mouse em cima ou foco nos controles: o pai não deixa sumir. */
  onPreso?: (preso: boolean) => void
  /** Plano sem transmissão: 'Vire Transmissor' no lugar de compartilhar (padrão true). */
  podeTransmitir?: boolean
}

const redondo: CSSProperties = {
  width: 48,
  height: 48,
  borderRadius: '50%',
  border: 0,
  background: '#16241F',
  color: '#E6F4EC',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
}

const separador: CSSProperties = { width: 1, height: 28, background: '#2A443C', margin: '0 4px' }

export function SalaControles(props: SalaControlesProps): ReactNode {
  const { muted, volume, camOn, sharing, smoothingMs } = props
  // Plano sem transmissão: só dá para "virar Transmissor" (abre o painel) e a câmera fica travada.
  const travado = props.podeTransmitir === false && !sharing
  const shareTxt = sharing ? 'Transmissão' : travado ? 'Vire Transmissor' : 'Compartilhar'
  const muteLabel = muted ? 'Ativar som da sala' : 'Silenciar a sala toda'
  const camLabel = camOn ? 'Desligar minha câmera' : 'Ligar minha câmera'
  const camTitle = travado ? 'Vire Transmissor para ligar a câmera' : camLabel
  const camStyle: CSSProperties = camOn ? redondo : { ...redondo, background: '#3A1C21', color: '#FF8592' }
  const shareStyle: CSSProperties = {
    height: 48,
    padding: '0 18px',
    borderRadius: 999,
    border: 0,
    background: sharing ? '#3BF08A' : '#16241F',
    color: sharing ? '#03140A' : '#E6F4EC',
    fontWeight: 700,
    display: 'flex',
    alignItems: 'center',
    gap: 8,
    whiteSpace: 'nowrap',
  }
  const svgBase = { fill: 'none', stroke: 'currentColor', strokeWidth: 1.8, 'aria-hidden': true } as const
  const visivel = props.visivel ?? true
  const preso = props.onPreso

  return (
    <div
      role="toolbar"
      aria-label="Controles da sala"
      data-visivel={visivel}
      onPointerEnter={() => preso?.(true)}
      onPointerLeave={() => preso?.(false)}
      onFocus={() => preso?.(true)}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) preso?.(false)
      }}
      style={{
        opacity: visivel ? 1 : 0,
        pointerEvents: visivel ? 'auto' : 'none',
        transition: 'opacity 0.25s ease',
        position: 'absolute',
        left: '50%',
        bottom: 22,
        transform: 'translateX(-50%)',
        width: 'max-content',
        maxWidth: 'calc(100% - 24px)',
        display: 'flex',
        flexWrap: 'wrap',
        justifyContent: 'center',
        alignItems: 'center',
        gap: 8,
        padding: 8,
        borderRadius: 30,
        background: 'rgba(10,16,14,0.86)',
        border: '1px solid #2A443C',
        boxShadow: '0 12px 40px rgba(0,0,0,0.45)',
      }}
    >
      <button type="button" className="zp-dock" onClick={props.onToggleMute} aria-label={muteLabel} title={muteLabel} style={redondo}>
        <svg width="20" height="20" viewBox="0 0 24 24" strokeLinecap="round" strokeLinejoin="round" {...svgBase}>
          <path d="M4 9v6h4l5 4V5L8 9z" />
          {muted ? <path d="m17 9 5 6M22 9l-5 6" /> : <path d="M16.5 8.5a5 5 0 0 1 0 7M19 6a8.5 8.5 0 0 1 0 12" />}
        </svg>
      </button>
      <label
        htmlFor="zp-sala-vol"
        style={{ position: 'absolute', width: 1, height: 1, overflow: 'hidden', clip: 'rect(0 0 0 0)' }}
      >
        Volume
      </label>
      <input
        id="zp-sala-vol"
        type="range"
        min={0}
        max={100}
        value={volume}
        onChange={(e) => props.onVolume(Number(e.currentTarget.value))}
        title="Volume geral"
        className="zp-range zp-ctl-opc"
        style={{ width: 90 }}
      />
      <span aria-hidden="true" className="zp-ctl-opc" style={separador} />
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
        <span className="zp-ctl-txt">{shareTxt}</span>
      </button>
      <button
        type="button"
        className="zp-dock zp-ctl-opc"
        aria-label={`Suavização automática, ${smoothingMs} milissegundos`}
        title="Suavização"
        style={{
          height: 48,
          padding: '0 14px',
          borderRadius: 999,
          border: 0,
          background: '#16241F',
          color: '#A3BDB1',
          fontFamily: "'JetBrains Mono', monospace",
          fontSize: 12,
          display: 'flex',
          alignItems: 'center',
          gap: 8,
        }}
      >
        <svg width="18" height="18" viewBox="0 0 24 24" strokeLinecap="round" {...svgBase}>
          <path d="M3 12c2.5-5 5.5-5 8 0s5.5 5 8 0" />
        </svg>
        auto
      </button>
      <button type="button" className="zp-dock" onClick={props.onFullscreen} aria-label="Tela cheia (F11)" title="Tela cheia (F11)" style={redondo}>
        <svg width="20" height="20" viewBox="0 0 24 24" strokeLinecap="round" strokeLinejoin="round" {...svgBase}>
          <path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" />
        </svg>
      </button>
      <span aria-hidden="true" style={separador} />
      <button
        type="button"
        className="zp-leave"
        onClick={props.onLeave}
        style={{
          height: 48,
          padding: '0 18px',
          borderRadius: 999,
          border: 0,
          background: '#FF5D6E',
          color: '#1A0508',
          fontWeight: 700,
          whiteSpace: 'nowrap',
          display: 'flex',
          alignItems: 'center',
          gap: 8,
        }}
      >
        <svg width="18" height="18" viewBox="0 0 24 24" strokeLinecap="round" strokeLinejoin="round" fill="none" stroke="currentColor" strokeWidth={2} aria-hidden="true">
          <path d="M15 4h3a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-3M10 8l-4 4 4 4M6 12h10" />
        </svg>
        Sair
      </button>
    </div>
  )
}
