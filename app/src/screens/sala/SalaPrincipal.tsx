import { useEffect, useRef, useState, type ReactNode } from 'react'
import type { RoomView, TileView, ShareView } from '../../bridge/types'
import { api } from '../../bridge/api'
import { VideoPlayer } from '../../video/player'
import { VideoTile } from '../../components/VideoTile'
import { ScenePreview } from '../../components/ScenePreview'
import { Ping } from '../../components/Ping'
import { Button } from '../../components/Button'
import { Toggle } from '../../components/Toggle'

export interface SalaPrincipalProps { room: RoomView; main: TileView | null; share: ShareView | null; smoothingAuto: boolean; onSmoothingAuto: (v: boolean) => void; onToggleCamera: () => void; onToggleShare: () => void; onLeave: () => void; onFullscreen: () => void }

const MONO = { fontFamily: "'JetBrains Mono', monospace" } as const

export function SalaPrincipal(p: SalaPrincipalProps): ReactNode {
  const { room, main } = p
  const canvasRef = useRef<HTMLCanvasElement | null>(null)
  const [mudo, setMudo] = useState(false)

  // Guarda as últimas props para os atalhos sem reinscrever o listener a cada render.
  const ref = useRef({ p, mudo, dono: undefined as string | undefined })
  ref.current = {
    p,
    mudo,
    dono: main ? room.tracks.find((t) => t.id === main.track_id)?.owner_id : undefined,
  }

  const tile = main?.tile
  const trackId = main?.track_id
  useEffect(() => {
    const canvas = canvasRef.current
    if (tile === undefined || trackId === undefined || !canvas) return
    // Idempotente: cada execução cria seu player e o cleanup desfaz tudo (StrictMode).
    const player = new VideoPlayer(canvas)
    void api
      .watch(trackId, tile, [canvas.clientWidth || 1280, canvas.clientHeight || 720], null, (buf) => player.push(buf))
      .catch(() => {})
    return () => {
      void api.unwatch(tile)?.catch?.(() => {})
      player.close()
    }
  }, [tile, trackId])

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const alvo = e.target as HTMLElement | null
      const tag = alvo?.tagName
      if (tag === 'INPUT' || tag === 'TEXTAREA') return
      const { p: pr, mudo: m, dono } = ref.current
      if (e.key === 'F11') {
        pr.onFullscreen()
      } else if (e.key === 'Escape') {
        if (document.fullscreenElement) pr.onFullscreen()
      } else if (e.key === 'm' || e.key === 'M') {
        const novo = !m
        setMudo(novo)
        // Sem dono do track não há a quem aplicar o mudo.
        if (dono) void api.setVolume(dono, 1, novo)?.catch?.(() => {})
      } else if (e.key === 'c' || e.key === 'C') {
        pr.onToggleCamera()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  const dono = main ? room.participants.find((x) => x.id === ref.current.dono) : undefined
  const compartilhando = p.share != null

  return (
    <main style={{ flex: '999 1 600px', minWidth: 0, display: 'flex', flexDirection: 'column', gap: 16, padding: '20px 28px 28px' }}>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 4, minWidth: 0 }}>
        <h1 style={{ margin: 0, fontFamily: "'Unbounded', sans-serif", fontWeight: 700, fontSize: 22, letterSpacing: '-0.015em' }}>{room.name}</h1>
        {main && (
          <span style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 10, fontSize: 13, color: 'var(--texto-2, #A3BDB1)', ...MONO }}>
            <span>{`via ${room.mode === 'local' ? 'Local' : 'Radmin'}`}</span>
            <Ping ms={main.ping_ms} />
            <span>{`suav. ${Math.round(main.smoothing_ms)} ms`}</span>
            <span>{`camada ${main.layer + 1}`}</span>
            <span>{`${(main.kbps / 1000).toFixed(1)} Mbps`}</span>
            <span>{`${main.width}x${main.height}@${main.fps}`}</span>
          </span>
        )}
      </div>

      {main ? (
        <VideoTile
          label={dono?.name ?? 'Transmissão'}
          color={dono?.color}
          muted={mudo}
          onMute={() => {
            const novo = !mudo
            setMudo(novo)
            if (ref.current.dono) void api.setVolume(ref.current.dono, 1, novo)?.catch?.(() => {})
          }}
          canvasRef={(c) => { canvasRef.current = c }}
        />
      ) : (
        <div style={{ position: 'relative', aspectRatio: '16 / 9', borderRadius: 20, overflow: 'hidden', border: '1px solid var(--borda, #2A443C)' }}>
          <ScenePreview scene="game" />
          <span style={{ position: 'absolute', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center', background: 'rgba(10,16,14,0.6)', fontWeight: 600 }}>
            Ninguém transmitindo ainda
          </span>
        </div>
      )}

      <div role="toolbar" aria-label="Controles da sala" style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', justifyContent: 'center', gap: 8 }}>
        <Button variant="ghost" onClick={p.onToggleCamera}>Câmera</Button>
        <Button variant={compartilhando ? 'danger' : 'ghost'} onClick={p.onToggleShare}>
          {compartilhando ? 'Parar de compartilhar' : 'Compartilhar tela'}
        </Button>
        <Toggle checked={p.smoothingAuto} onChange={p.onSmoothingAuto} label="Suavização auto" />
        <Button variant="ghost" onClick={p.onFullscreen}>Tela cheia</Button>
        <Button variant="danger" onClick={p.onLeave}>Sair</Button>
      </div>
    </main>
  )
}
