import { useEffect, useState, type CSSProperties, type ReactNode } from 'react'
import type { RoomView, TileView } from '../../bridge/types'
import { api } from '../../bridge/api'
import { VideoPlayer } from '../../video/player'
import { Avatar } from '../../components/Avatar'
import { ScenePreview, type SceneName } from '../../components/ScenePreview'
import { SalaVoce } from './SalaVoce'
import { kindLabel } from './salaModel'

export interface SalaTilesProps {
  room: RoomView
  tiles: TileView[]
  mainTile: number | null
  onPick: (tile: number) => void
  meId?: string
  camOn?: boolean
  sharing?: boolean
  onToggleCam?: () => void
  onToggleShare?: () => void
}

const SCENA: Record<string, SceneName> = { camera: 'cama', window: 'desktop', screen: 'game' }

// card(on) do script x-dc
const card = (on: boolean): CSSProperties => ({
  display: 'flex', flexDirection: 'column', minWidth: 0, borderRadius: 16, overflow: 'hidden', background: '#0F1916',
  border: '2px solid ' + (on ? '#3BF08A' : '#1F332D'), ...(on ? { boxShadow: '0 0 20px rgba(59,240,138,0.22)' } : {}),
})
const small = (alert: boolean): CSSProperties => ({
  flex: '0 0 18px', width: 18, height: 18, border: 0, padding: 0, background: 'transparent', display: 'flex', alignItems: 'center', justifyContent: 'center',
  color: alert ? '#FF8592' : '#E6F4EC',
})
const BTN_X: CSSProperties = {
  position: 'absolute', right: 6, top: 6, width: 36, height: 36, borderRadius: 10, border: '1px solid #2A443C', background: 'rgba(10,16,14,0.82)',
  color: '#E6F4EC', display: 'flex', alignItems: 'center', justifyContent: 'center',
}
const EQ: CSSProperties = { width: 2, background: '#3BF08A', borderRadius: 1 }
const MONO = "'JetBrains Mono', monospace"
const SR_ONLY: CSSProperties = { position: 'absolute', width: 1, height: 1, overflow: 'hidden', clip: 'rect(0 0 0 0)' }

interface TileProps { tile: TileView; room: RoomView; main: boolean; onPick: (tile: number) => void }

function Tile({ tile, room, main, onPick }: TileProps): ReactNode {
  const track = room.tracks.find((x) => x.id === tile.track_id)
  const dono = track?.owner_id
  const pessoa = room.participants.find((x) => x.id === dono)
  const [canvas, setCanvas] = useState<HTMLCanvasElement | null>(null)
  // Volume e mudo iniciais são os que EU já escolhi para esse participante (vêm do room).
  const [vol, setVol] = useState(() => Math.round((pessoa?.volume ?? 0.8) * 100))
  const [muted, setMuted] = useState(() => pessoa?.muted ?? false)
  const [closed, setClosed] = useState(false)

  const nome = pessoa?.name ?? 'Alguém'
  const tipo = kindLabel(track?.kind ?? 'screen')
  const id = tile.tile
  const trackId = tile.track_id

  // O canvas só existe com o tile aberto: fechar desmonta o canvas, o cleanup faz unwatch;
  // reabrir cria outro canvas e refaz o watch. Miniatura pede só a camada pequena (2).
  useEffect(() => {
    if (!canvas) return
    const player = new VideoPlayer(canvas)
    void api.watch(trackId, id, [320, 180], 2, (buf) => player.push(buf))?.catch?.(() => {})
    return () => {
      void api.unwatch(id)?.catch?.(() => {})
      player.close()
    }
  }, [canvas, trackId, id])

  const aplicar = (v: number, m: boolean) => {
    if (dono) void api.setVolume(dono, v / 100, m)?.catch?.(() => {})
  }
  const audible = !closed && !muted && vol > 0
  const muteLabel = (muted ? 'Ativar som de ' : 'Silenciar ') + nome
  const closeLabel = `Fechar tela de ${nome}`
  const volId = 'vol-' + id

  return (
    <div style={card(main)}>
      <div style={{ position: 'relative' }}>
        {!closed ? (
          <button type="button" onClick={() => onPick(id)} aria-pressed={main} aria-label={`Assistir ${nome}, ${tipo}`} style={{ display: 'block', width: '100%', padding: 0, border: 0, background: 'transparent', position: 'relative' }}>
            <ScenePreview scene={SCENA[track?.kind ?? 'screen'] ?? 'game'} />
            <canvas ref={setCanvas} style={{ position: 'absolute', inset: 0, width: '100%', height: '100%' }} />
          </button>
        ) : (
          <div className="zp-dots" style={{ width: '100%', aspectRatio: '16 / 9', display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: 6, textAlign: 'center', padding: 8 }}>
            <span style={{ fontWeight: 600, fontSize: 13 }}>{`Tela de ${nome} fechada`}</span>
            <span style={{ fontSize: 11.5, color: '#8BA498' }}>sem receber · economiza banda</span>
            <button type="button" className="zp-ghost" onClick={() => setClosed(false)} style={{ marginTop: 2, height: 34, padding: '0 14px', borderRadius: 999, border: '1px solid #3BF08A', background: 'transparent', color: '#3BF08A', fontSize: 12.5, fontWeight: 600 }}>Reabrir</button>
          </div>
        )}
        <span style={{ position: 'absolute', left: 8, bottom: 8, maxWidth: 'calc(100% - 16px)', padding: '3px 8px', borderRadius: 7, background: 'rgba(10,16,14,0.84)', fontSize: 12, display: 'flex', alignItems: 'center', gap: 6, whiteSpace: 'nowrap', overflow: 'hidden', pointerEvents: 'none' }}>
          {audible && (
            <span aria-hidden="true" style={{ display: 'flex', alignItems: 'flex-end', gap: 2, height: 11 }}>
              <span className="zp-eq" style={EQ} /><span className="zp-eq zp-d2" style={EQ} /><span className="zp-eq zp-d3" style={EQ} />
            </span>
          )}
          <b style={{ fontWeight: 600 }}>{nome}</b><span style={{ color: '#A3BDB1', overflow: 'hidden', textOverflow: 'ellipsis' }}>{`· ${tipo}`}</span>
        </span>
        {main && !closed && (
          <span style={{ position: 'absolute', left: 8, top: 8, padding: '2px 7px', borderRadius: 6, background: '#3BF08A', color: '#03140A', fontSize: 10.5, fontWeight: 700, letterSpacing: '0.06em', pointerEvents: 'none' }}>Assistindo</span>
        )}
        {!closed && (
          <button type="button" className="zp-x" onClick={() => setClosed(true)} aria-label={closeLabel} title={closeLabel} style={BTN_X}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true"><path d="M6 6l12 12M18 6 6 18" /></svg>
          </button>
        )}
      </div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 10px 6px 6px' }}>
        <Avatar name={nome} color={pessoa?.color ?? '#5CD6FF'} size={22} />
        <button type="button" className="zp-dock" onClick={() => { const m = !muted; setMuted(m); aplicar(vol, m) }} aria-label={muteLabel} title={muteLabel} disabled={closed} style={small(muted)}>
          {!muted ? (
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M4 9v6h4l5 4V5L8 9z" /><path d="M16.5 8.5a5 5 0 0 1 0 7" /></svg>
          ) : (
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M4 9v6h4l5 4V5L8 9z" /><path d="m17 9 5 6M22 9l-5 6" /></svg>
          )}
        </button>
        <label htmlFor={volId} style={SR_ONLY}>{`Volume de ${nome}`}</label>
        <input id={volId} className="zp-range" type="range" min={0} max={100} value={vol} disabled={closed} style={{ flex: 1, minWidth: 0 }}
          onChange={(e) => { const v = Number(e.target.value); setVol(v); setMuted(false); aplicar(v, false) }} />
        <span style={{ flex: '0 0 38px', textAlign: 'right', fontFamily: MONO, fontSize: 11, color: '#A3BDB1' }}>{closed ? '—' : muted ? 'mudo' : `${vol}%`}</span>
      </div>
    </div>
  )
}

export function SalaTiles({ room, tiles, mainTile, onPick, meId = '', camOn = false, sharing = false, onToggleCam, onToggleShare }: SalaTilesProps): ReactNode {
  return (
    <div role="group" aria-label="Transmissões na sala" style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(200px, 1fr))', gap: 14 }}>
      {tiles.map((t) => (
        <Tile key={t.tile} tile={t} room={room} main={t.tile === mainTile} onPick={onPick} />
      ))}
      <SalaVoce room={room} meId={meId} camOn={camOn} sharing={sharing} onToggleCam={onToggleCam} onToggleShare={onToggleShare} />
    </div>
  )
}
