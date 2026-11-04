// Tarefa salas-08
import type { CSSProperties, ReactNode } from 'react'
import { VideoFrame } from '../../components/VideoFrame'
import { ScenePreview } from '../../components/ScenePreview'
import type { SalaVM } from './model'

export interface SalaPreviaProps { sala: SalaVM }

const VERDE = '#3BF08A'
const LADO = `2px solid ${VERDE}`

// Cantos de 20px a 12px da borda (1-salas.html 172-175); o VideoFrame usa 24/14, então desenhamos aqui.
const CANTOS: { id: string; estilo: CSSProperties }[] = [
  { id: 'tl', estilo: { left: 12, top: 12, borderLeft: LADO, borderTop: LADO } },
  { id: 'tr', estilo: { right: 12, top: 12, borderRight: LADO, borderTop: LADO } },
  { id: 'bl', estilo: { left: 12, bottom: 12, borderLeft: LADO, borderBottom: LADO } },
  { id: 'br', estilo: { right: 12, bottom: 12, borderRight: LADO, borderBottom: LADO } },
]

const rotulo: CSSProperties = {
  position: 'absolute', top: 22, height: 26, padding: '0 10px', borderRadius: 8,
  background: 'rgba(10,16,14,0.78)', fontFamily: "'JetBrains Mono', monospace", fontSize: 11,
  display: 'flex', alignItems: 'center',
}

export function SalaPrevia({ sala }: SalaPreviaProps): ReactNode {
  const info = sala.bitrate ? `${sala.qualidade} · ${sala.bitrate}` : sala.qualidade
  return (
    <div style={{ flex: '1 1 480px', minWidth: 0, display: 'flex', flexDirection: 'column', gap: 10 }}>
      <VideoFrame radius={18} corners={false}>
        <ScenePreview scene={sala.cena} />
        {CANTOS.map((c) => (
          <span key={c.id} data-corner={c.id} aria-hidden="true" style={{ position: 'absolute', width: 20, height: 20, ...c.estilo }} />
        ))}
        <span style={{ ...rotulo, left: 22, letterSpacing: '0.08em', gap: 7 }}>
          <span className="zp-blink" style={{ width: 6, height: 6, borderRadius: '50%', background: '#FF5D6E' }} />
          AO VIVO
        </span>
        <span style={{ ...rotulo, right: 22, color: '#A3BDB1' }}>{info}</span>
        {sala.camCena != null && (
          <span style={{ position: 'absolute', right: 22, bottom: 22, width: '26%', borderRadius: 12, overflow: 'hidden', border: '2px solid #0A100E', boxShadow: `0 0 0 1px ${VERDE}, 0 8px 24px rgba(0,0,0,0.5)` }}>
            <ScenePreview scene={sala.camCena} />
            <span style={{ position: 'absolute', left: 6, bottom: 6, padding: '2px 7px', borderRadius: 6, background: 'rgba(10,16,14,0.8)', fontSize: 11 }}>
              {`cam · ${sala.host}`}
            </span>
          </span>
        )}
      </VideoFrame>
      <span style={{ fontSize: 12.5, color: '#7E978B' }}>Prévia em baixa resolução, sem som. O áudio começa quando você entra.</span>
    </div>
  )
}
