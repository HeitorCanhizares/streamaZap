// Tarefa previa-07
import type { CSSProperties, ReactNode } from 'react'

export interface VideoFrameProps {
  /** Conteúdo do vídeo (ScenePreview ou <canvas>) */
  children?: ReactNode
  /** Mostra os 4 cantos verdes (3-sala.html 97-101). Padrão: true */
  corners?: boolean
  /** Raio da moldura em px. Padrão: 20 */
  radius?: number
  style?: CSSProperties
}

const COR_CANTO = '#3BF08A'

// Cada canto: 24x24, a 14px das bordas, com dois lados de 2px.
const CANTOS: { id: 'tl' | 'tr' | 'bl' | 'br'; posicao: CSSProperties; lados: CSSProperties }[] = [
  { id: 'tl', posicao: { left: 14, top: 14 }, lados: { borderLeft: `2px solid ${COR_CANTO}`, borderTop: `2px solid ${COR_CANTO}` } },
  { id: 'tr', posicao: { right: 14, top: 14 }, lados: { borderRight: `2px solid ${COR_CANTO}`, borderTop: `2px solid ${COR_CANTO}` } },
  { id: 'bl', posicao: { left: 14, bottom: 14 }, lados: { borderLeft: `2px solid ${COR_CANTO}`, borderBottom: `2px solid ${COR_CANTO}` } },
  { id: 'br', posicao: { right: 14, bottom: 14 }, lados: { borderRight: `2px solid ${COR_CANTO}`, borderBottom: `2px solid ${COR_CANTO}` } },
]

// Moldura 16:9 com borda #2A443C, fundo #0C1412 e cantos verdes opcionais.
export function VideoFrame({ children, corners = true, radius = 20, style }: VideoFrameProps): ReactNode {
  const base: CSSProperties = {
    position: 'relative',
    width: '100%',
    aspectRatio: '16 / 9',
    borderRadius: radius,
    overflow: 'hidden',
    border: '1px solid #2A443C',
    background: '#0C1412',
  }
  return (
    <div data-testid="video-frame" style={{ ...base, ...style }}>
      {children}
      {corners &&
        CANTOS.map((c) => (
          <span
            key={c.id}
            data-corner={c.id}
            aria-hidden="true"
            style={{ position: 'absolute', width: 24, height: 24, ...c.posicao, ...c.lados }}
          />
        ))}
    </div>
  )
}
