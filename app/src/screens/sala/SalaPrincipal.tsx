import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react'
import type { RoomView, TileView, ShareView } from '../../bridge/types'
import { api } from '../../bridge/api'
import { VideoPlayer } from '../../video/player'
import { VideoFrame } from '../../components/VideoFrame'
import { ScenePreview, type SceneName } from '../../components/ScenePreview'
import { SalaCabecalho } from './SalaCabecalho'
import { SalaControles } from './SalaControles'

export interface SalaPrincipalProps {
  room: RoomView
  main: TileView | null
  share: ShareView | null
  smoothingAuto: boolean
  onSmoothingAuto: (v: boolean) => void
  onToggleCamera: () => void
  onToggleShare: () => void
  /** Parar a transmissão (botão do cabeçalho); sem ele, usa onToggleShare. */
  onStopShare?: () => void
  onLeave: () => void
  onFullscreen: () => void
  // Props abaixo vêm da SalaScreen (sala-07); até lá têm padrão.
  muted?: boolean
  volume?: number
  camOn?: boolean
  pipHidden?: boolean
  onTogglePip?: () => void
  panelOpen?: boolean
  onOpenPanel?: () => void
  onToggleMute?: () => void
  onVolume?: (v: number) => void
  onCloseMain?: () => void
  reconnecting?: boolean
  // Texto copiado pelo botão Copiar convite (vem de inviteText na SalaScreen).
  invite?: string
}

const noop = () => {}
const SCENA: Record<string, SceneName> = { camera: 'cama', window: 'desktop', screen: 'game' }
const PILULA: CSSProperties = { background: 'rgba(10,16,14,0.8)', borderRadius: 9, display: 'flex', alignItems: 'center' }
const BTN_X: CSSProperties = { border: '1px solid #2A443C', background: 'rgba(10,16,14,0.8)', color: '#E6F4EC', display: 'flex', alignItems: 'center', justifyContent: 'center' }

function IconeX({ size }: { size: number }): ReactNode {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" aria-hidden="true">
      <path d="M6 6l12 12M18 6 6 18" />
    </svg>
  )
}

export function SalaPrincipal(p: SalaPrincipalProps): ReactNode {
  const { room, main } = p
  const canvasRef = useRef<HTMLCanvasElement | null>(null)
  const [mudoLocal, setMudo] = useState(false)
  const mudo = p.muted ?? mudoLocal

  const trackMain = main ? room.tracks.find((t) => t.id === main.track_id) : undefined

  // Guarda as últimas props para os atalhos sem reinscrever o listener a cada render.
  const ref = useRef({ p, mudo, dono: undefined as string | undefined })
  ref.current = { p, mudo, dono: trackMain?.owner_id }

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

  const alternarMudo = () => {
    const { p: pr, mudo: m, dono } = ref.current
    const novo = !m
    setMudo(novo)
    pr.onToggleMute?.()
    // Sem dono do track não há a quem aplicar o mudo.
    if (dono) void api.setVolume(dono, 1, novo)?.catch?.(() => {})
  }

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement | null)?.tagName
      if (tag === 'INPUT' || tag === 'TEXTAREA') return
      const pr = ref.current.p
      if (e.key === 'F11') {
        pr.onFullscreen()
      } else if (e.key === 'Escape') {
        if (document.fullscreenElement) pr.onFullscreen()
      } else if (e.key === 'm' || e.key === 'M') {
        alternarMudo()
      } else if (e.key === 'c' || e.key === 'C') {
        pr.onToggleCamera()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const dono = trackMain ? room.participants.find((x) => x.id === trackMain.owner_id) : undefined
  const nome = dono?.name ?? 'Transmissão'
  const temCamera = !!trackMain && trackMain.kind !== 'camera' && room.tracks.some((t) => t.owner_id === trackMain.owner_id && t.kind === 'camera')
  const mostrarPip = temCamera && !p.pipHidden
  const compartilhando = p.share != null
  const aoVivo = !!main
  const rotulo = main ? `camada ${main.layer + 1} · ${Math.round(main.ping_ms)} ms ${main.height}p${main.fps}` : ''

  return (
    <main style={{ flex: '999 1 600px', minWidth: 0, display: 'flex', flexDirection: 'column', gap: 16, padding: '20px 28px 28px' }}>
      <SalaCabecalho
        room={room}
        main={main}
        sharing={compartilhando}
        panelOpen={p.panelOpen ?? true}
        onOpenPanel={p.onOpenPanel ?? noop}
        onStop={p.onStopShare ?? p.onToggleShare}
        invite={p.invite}
      />

      <VideoFrame>
        {main ? (
          <>
            <ScenePreview scene={SCENA[trackMain?.kind ?? 'screen'] ?? 'game'} />
            <canvas ref={(c) => { canvasRef.current = c }} style={{ position: 'absolute', inset: 0, width: '100%', height: '100%' }} />
          </>
        ) : (
          <div className="zp-dots" style={{ position: 'absolute', inset: 0, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: 14, textAlign: 'center', padding: 24 }}>
            <img src="/icone.png" alt="" style={{ width: 104, height: 104, borderRadius: 26, display: 'block', boxShadow: '0 0 0 1px #2A443C, 0 0 36px rgba(59,240,138,0.35)' }} />
            <span style={{ fontFamily: "'Unbounded', sans-serif", fontWeight: 700, fontSize: 20 }}>Nenhuma transmissão aberta</span>
            <span style={{ color: 'var(--texto-2)' }}>Reabra a tela ou a câmera de alguém aqui embaixo.</span>
          </div>
        )}

        {aoVivo && (
          <>
            <span style={{ ...PILULA, position: 'absolute', left: 26, top: 26, height: 30, padding: '0 12px', fontSize: 12, fontWeight: 700, letterSpacing: '0.08em', gap: 8 }}>
              <span className="zp-blink" aria-hidden="true" style={{ width: 7, height: 7, borderRadius: '50%', background: '#FF5D6E' }} />
              AO VIVO
              <span style={{ fontWeight: 500, letterSpacing: 0, color: 'var(--texto-2)' }}>{`· ${nome}`}</span>
            </span>
            <span style={{ position: 'absolute', right: 26, top: 22, display: 'flex', alignItems: 'center', gap: 8 }}>
              <span style={{ ...PILULA, height: 30, padding: '0 12px', fontFamily: "'JetBrains Mono', monospace", fontSize: 11.5, color: 'var(--texto-2)', gap: 10 }}>{rotulo}</span>
              <button type="button" className="zp-x" onClick={p.onCloseMain} aria-label={`Fechar tela de ${nome}`} title={`Fechar tela de ${nome}`} style={{ ...BTN_X, width: 40, height: 40, borderRadius: 12 }}>
                <IconeX size={18} />
              </button>
            </span>
          </>
        )}

        {p.reconnecting && (
          <span role="status" style={{ position: 'absolute', left: '50%', top: 26, transform: 'translateX(-50%)', height: 34, padding: '0 16px', borderRadius: 999, background: '#FFB547', color: '#1A1204', fontWeight: 700, fontSize: 13, display: 'flex', alignItems: 'center', gap: 8 }}>
            <span className="zp-blink" aria-hidden="true" style={{ width: 7, height: 7, borderRadius: '50%', background: '#1A1204' }} />
            Conexão caiu — reconectando…
          </span>
        )}

        {mostrarPip && (
          <span style={{ position: 'absolute', right: 26, bottom: 96, width: '22%', borderRadius: 14, overflow: 'hidden', border: '2px solid #0A100E', boxShadow: '0 0 0 1px #3BF08A, 0 10px 30px rgba(0,0,0,0.5)' }}>
            <ScenePreview scene="cama" />
            <span style={{ position: 'absolute', left: 8, bottom: 8, padding: '2px 8px', borderRadius: 6, background: 'rgba(10,16,14,0.8)', fontSize: 11.5 }}>{`cam · ${nome}`}</span>
            <button type="button" className="zp-x" onClick={p.onTogglePip} aria-label={`Fechar câmera de ${nome}`} title="Fechar câmera" style={{ ...BTN_X, position: 'absolute', right: 6, top: 6, width: 36, height: 36, borderRadius: 10 }}>
              <IconeX size={16} />
            </button>
          </span>
        )}

        {temCamera && p.pipHidden && (
          <button type="button" className="zp-ghost" onClick={p.onTogglePip} style={{ position: 'absolute', right: 26, bottom: 96, height: 40, padding: '0 14px', borderRadius: 999, border: '1px solid #2A443C', background: 'rgba(10,16,14,0.86)', color: '#E6F4EC', fontWeight: 600, fontSize: 13, display: 'flex', alignItems: 'center', gap: 8 }}>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinejoin="round" aria-hidden="true">
              <rect x="3" y="7" width="13" height="10" rx="2" />
              <path d="m16 11 5-3v8l-5-3z" />
            </svg>
            Mostrar câmera
          </button>
        )}

        <SalaControles
          muted={mudo}
          volume={p.volume ?? 100}
          camOn={p.camOn ?? false}
          sharing={compartilhando}
          smoothingMs={main ? Math.round(main.smoothing_ms) : 0}
          onFullscreen={p.onFullscreen}
          onToggleMute={alternarMudo}
          onVolume={p.onVolume ?? noop}
          onToggleCam={p.onToggleCamera}
          onToggleShare={p.onToggleShare}
          onLeave={p.onLeave}
        />
      </VideoFrame>
    </main>
  )
}
