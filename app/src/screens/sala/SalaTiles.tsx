import { useEffect, useRef, useState, type ReactNode } from 'react'
import type { RoomView, TileView } from '../../bridge/types'
import { api } from '../../bridge/api'
import { VideoPlayer } from '../../video/player'
import { VideoTile } from '../../components/VideoTile'

export interface SalaTilesProps { room: RoomView; tiles: TileView[]; mainTile: number | null; onPick: (tile: number) => void }

interface TileVideoProps { tile: TileView; label: string; color?: string; ownerId?: string; onPick: (tile: number) => void }

// Miniatura com player próprio; pede só a camada pequena (max_layer 2) para poupar banda.
function TileVideo({ tile, label, color, ownerId, onPick }: TileVideoProps): ReactNode {
  const [canvas, setCanvas] = useState<HTMLCanvasElement | null>(null)
  const [volume, setVolume] = useState(0.8)
  const [muted, setMuted] = useState(false)
  const ownerRef = useRef(ownerId)
  ownerRef.current = ownerId

  const id = tile.tile
  const trackId = tile.track_id
  useEffect(() => {
    if (!canvas) return
    // Idempotente: o cleanup desfaz tudo (StrictMode).
    const player = new VideoPlayer(canvas)
    void api
      .watch(trackId, id, [320, 180], 2, (buf) => player.push(buf))
      ?.catch?.(() => {})
    return () => {
      void api.unwatch(id)?.catch?.(() => {})
      player.close()
    }
  }, [canvas, trackId, id])

  const aplicar = (v: number, m: boolean) => {
    if (ownerRef.current) void api.setVolume(ownerRef.current, v, m)?.catch?.(() => {})
  }

  return (
    <div onClick={(e) => { if (!(e.target as HTMLElement).closest('button, input')) onPick(id) }} style={{ flex: '0 0 220px', minWidth: 0, cursor: 'pointer' }}>
      <VideoTile
        label={label}
        color={color}
        volume={volume}
        muted={muted}
        onVolume={(v) => { setVolume(v); aplicar(v, muted) }}
        onMute={() => { const m = !muted; setMuted(m); aplicar(volume, m) }}
        onClose={() => { void api.unwatch(id)?.catch?.(() => {}) }}
        canvasRef={setCanvas}
      />
    </div>
  )
}

export function SalaTiles({ room, tiles, mainTile, onPick }: SalaTilesProps): ReactNode {
  return (
    <div style={{ display: 'flex', gap: 12, overflowX: 'auto', paddingBottom: 4 }}>
      <div
        className="zp-tile-voce"
        style={{ flex: '0 0 160px', display: 'flex', flexDirection: 'column', justifyContent: 'space-between', gap: 8, padding: 12, borderRadius: 16, background: 'var(--card, #0F1916)', border: '2px solid var(--borda, #1F332D)' }}
      >
        <span style={{ fontWeight: 700 }}>Você</span>
        <div style={{ display: 'flex', gap: 6 }}>
          <button type="button" className="zp-btn zp-btn-ghost">Câmera</button>
          <button type="button" className="zp-btn zp-btn-ghost">Tela</button>
        </div>
      </div>
      {tiles.filter((t) => t.tile !== mainTile).map((t) => {
        const dono = room.tracks.find((x) => x.id === t.track_id)?.owner_id
        const pessoa = room.participants.find((x) => x.id === dono)
        return (
          <TileVideo key={t.tile} tile={t} label={pessoa?.name ?? 'Alguém'} color={pessoa?.color} ownerId={dono} onPick={onPick} />
        )
      })}
    </div>
  )
}
