// Cartao "Voce" da faixa de tiles: previa da propria tela/camera via watch.
import { useEffect, useState, type CSSProperties, type ReactNode } from 'react'
import type { RoomView } from '../../bridge/types'
import { api } from '../../bridge/api'
import { VideoPlayer } from '../../video/player'
import { ScenePreview, type SceneName } from '../../components/ScenePreview'
import { LOCAL_CAMERA_TILE, LOCAL_SCREEN_TILE, ownTracks } from './salaModel'

export interface SalaVoceProps {
  room: RoomView
  meId: string
  camOn: boolean
  sharing: boolean
  onToggleCam?: () => void
  onToggleShare?: () => void
}

// card(on) do script x-dc
const card = (on: boolean): CSSProperties => ({
  display: 'flex', flexDirection: 'column', minWidth: 0, borderRadius: 16, overflow: 'hidden', background: '#0F1916',
  border: '2px solid ' + (on ? '#3BF08A' : '#1F332D'), ...(on ? { boxShadow: '0 0 20px rgba(59,240,138,0.22)' } : {}),
})
const meBtn = (on: boolean): CSSProperties => ({
  flex: 1, minWidth: 0, height: 44, borderRadius: 12, display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 6, fontSize: 13, fontWeight: 600,
  border: '1px solid ' + (on ? '#3BF08A' : '#2A443C'), background: on ? 'rgba(59,240,138,0.12)' : 'transparent', color: on ? '#3BF08A' : '#A3BDB1',
})
interface PreviaProps { trilhaId: string; tile: number; scene: SceneName; rotulo: string }

function Previa({ trilhaId, tile, scene, rotulo }: PreviaProps): ReactNode {
  const [canvas, setCanvas] = useState<HTMLCanvasElement | null>(null)
  // Mesmo padrao do Tile: camada pequena (2), unwatch + close na limpeza.
  useEffect(() => {
    if (!canvas) return
    const player = new VideoPlayer(canvas)
    void api.watch(trilhaId, tile, [320, 180], 2, (buf) => player.push(buf))?.catch?.(() => {})
    return () => {
      void api.unwatch(tile)?.catch?.(() => {})
      player.close()
    }
  }, [canvas, trilhaId, tile])
  return (
    <div style={{ position: 'relative', width: '100%', aspectRatio: '16 / 9' }}>
      <ScenePreview scene={scene} />
      <canvas ref={setCanvas} style={{ position: 'absolute', inset: 0, width: '100%', height: '100%' }} />
      <span style={{ position: 'absolute', left: 8, bottom: 8, padding: '3px 8px', borderRadius: 7, background: 'rgba(10,16,14,0.84)', fontSize: 12, pointerEvents: 'none' }}>{rotulo}</span>
    </div>
  )
}

export function SalaVoce({ room, meId, camOn, sharing, onToggleCam, onToggleShare }: SalaVoceProps): ReactNode {
  const { screen, camera } = ownTracks(room, meId)
  // A tela tem prioridade sobre a camera; so um canvas por vez.
  const previa = sharing && screen
    ? { id: screen.id, tile: LOCAL_SCREEN_TILE, scene: 'game' as SceneName, rotulo: 'sua tela' }
    : camOn && camera
      ? { id: camera.id, tile: LOCAL_CAMERA_TILE, scene: 'cama' as SceneName, rotulo: 'sua câmera' }
      : null
  return (
      <div style={card(false)}>
        {previa && <Previa trilhaId={previa.id} tile={previa.tile} scene={previa.scene} rotulo={previa.rotulo} />}
        <div style={{ padding: '10px 12px 0', fontWeight: 700 }}>Você</div>
        <div style={{ display: 'flex', gap: 8, padding: 6 }}>
          <button type="button" onClick={onToggleCam} aria-pressed={camOn} style={meBtn(camOn)}>
            {camOn ? (
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" aria-hidden="true"><rect x="3" y="7" width="13" height="10" rx="2" /><path d="m16 11 5-3v8l-5-3z" /></svg>
            ) : (
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><rect x="3" y="7" width="13" height="10" rx="2" /><path d="m16 11 5-3v8l-5-3zM3 3l18 18" /></svg>
            )}
            Câmera
          </button>
          <button type="button" onClick={onToggleShare} aria-pressed={sharing} style={meBtn(sharing)}>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><rect x="3" y="4" width="18" height="12" rx="2" /><path d="M8 20h8M12 16v4" /></svg>
            Tela
          </button>
        </div>
      </div>
  )
}
