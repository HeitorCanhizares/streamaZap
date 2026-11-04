// Tarefa ui-06
import type { ReactNode } from 'react'
import { Slider } from '../components/Slider'

export interface VideoTileProps { label: string; color?: string; volume?: number; muted?: boolean; onVolume?: (v: number) => void; onMute?: () => void; onClose?: () => void; canvasRef?: (c: HTMLCanvasElement | null) => void }

// Moldura de vídeo. Não decodifica nada: o canvas é entregue ao VideoPlayer via canvasRef.
export function VideoTile({ label, color, volume, muted = false, onVolume, onMute, onClose, canvasRef }: VideoTileProps): ReactNode {
  return (
    <div
      className="zp-video-tile"
      style={{
        position: 'relative',
        aspectRatio: '16 / 9',
        background: 'var(--bg)',
        border: '1px solid var(--borda)',
        borderRadius: 10,
        overflow: 'hidden',
      }}
    >
      <canvas
        ref={canvasRef}
        style={{ width: '100%', height: '100%', objectFit: 'contain', display: 'block' }}
      />

      <div
        className="zp-video-label"
        style={{ position: 'absolute', left: 8, bottom: 8, display: 'flex', alignItems: 'center', gap: 6 }}
      >
        <span
          aria-hidden="true"
          style={{ width: 8, height: 8, borderRadius: '50%', background: color ?? 'var(--accent)', display: 'inline-block' }}
        />
        <span>{label}</span>
      </div>

      {onClose && (
        <button
          type="button"
          className="zp-btn zp-btn-ghost zp-video-close"
          aria-label="Fechar"
          onClick={() => onClose()}
          style={{ position: 'absolute', top: 8, right: 8 }}
        >
          Fechar
        </button>
      )}

      <div
        className="zp-video-bar"
        style={{ position: 'absolute', right: 8, bottom: 8, display: 'flex', alignItems: 'center', gap: 8 }}
      >
        <button
          type="button"
          className="zp-btn zp-btn-ghost zp-video-mute"
          aria-label={muted ? 'Reativar som' : 'Mutar'}
          onClick={() => onMute?.()}
        >
          {muted ? 'Som' : 'Mudo'}
        </button>
        {onVolume && (
          <div style={{ width: 120, opacity: muted ? 0.4 : 1, pointerEvents: muted ? 'none' : 'auto' }}>
            <Slider value={volume ?? 1} onChange={onVolume} />
          </div>
        )}
      </div>
    </div>
  )
}
