// Tela principal da sala: uma ou mais transmissões lado a lado (a câmera de quem está com a tela
// aqui vai no canto dela), o cabeçalho e os controles, que somem com o mouse parado.
import { useEffect, useRef, type CSSProperties, type ReactNode } from 'react'
import type { RoomView, ShareView, TileView } from '../../bridge/types'
import { useTravas } from '../../bridge/usePlano'
import { VideoFrame } from '../../components/VideoFrame'
import { CanvasSlot } from './CanvasSlot'
import { CelulaMusica } from './musica/BlocoMusica'
import { SalaCabecalho } from './SalaCabecalho'
import { SalaControles } from './SalaControles'
import { kindLabel, type Celula, type Lugar, type VolumePessoa } from './salaModel'
import { useOcioso } from './useOcioso'

export interface SalaPrincipalProps {
  room: RoomView
  /** Transmissões na tela principal (cada uma uma célula; câmera no canto da tela do dono). */
  celulas: Celula[]
  /** Métricas do núcleo por tile (camada, ping, resolução). */
  tiles: TileView[]
  canvasDe: (trackId: string) => HTMLCanvasElement
  /** Há transmissões fora da principal? Muda o texto da tela vazia. */
  temOutras: boolean
  /** Há a faixa de baixo? O vídeo encolhe para ela caber na janela sem rolar. 'botao' = só o botão
   *  "Mostrar meu cartão" embaixo (ocupa pouco). */
  temFaixa?: boolean | 'botao'
  share: ShareView | null
  volumeDe: (ownerId: string) => VolumePessoa
  onVolumePessoa: (ownerId: string, v: VolumePessoa) => void
  onMover: (trackId: string, lugar: Lugar) => void
  /** Câmera de quem está com a tela: 'canto' (no canto da tela) ou 'solta' (célula própria). */
  onJeito?: (trackId: string, jeito: 'canto' | 'solta') => void
  smoothingAuto: boolean
  onSmoothingAuto: (v: boolean) => void
  onToggleCamera: () => void
  onToggleShare: () => void
  /** Parar a transmissão (botão do cabeçalho); sem ele, usa onToggleShare. */
  onStopShare?: () => void
  onLeave: () => void
  onFullscreen: () => void
  /** Volume geral da sala (0..100) e mudo geral. */
  muted?: boolean
  volume?: number
  camOn?: boolean
  panelOpen?: boolean
  onOpenPanel?: () => void
  onToggleMute?: () => void
  onVolume?: (v: number) => void
  reconnecting?: boolean
  // Texto copiado pelo botão Copiar convite (vem de inviteText na SalaScreen).
  invite?: string
}

const noop = () => {}
const PILULA: CSSProperties = { background: 'rgba(10,16,14,0.8)', borderRadius: 9, display: 'flex', alignItems: 'center' }
const BTN: CSSProperties = { border: '1px solid #2A443C', background: 'rgba(10,16,14,0.8)', color: '#E6F4EC', display: 'flex', alignItems: 'center', justifyContent: 'center', padding: 0 }
const CENTRO: CSSProperties = {
  position: 'absolute', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center', padding: 16, textAlign: 'center',
  color: '#8BA498', fontSize: 13,
}
const MONO = "'JetBrains Mono', monospace"
// Altura que fica fora do vídeo (px): barra de título, cabeçalho da sala e margens; com a faixa de
// baixo, também a altura de um cartão.
const RESERVA_SEM_FAIXA = 180
const RESERVA_COM_FAIXA = 400
const RESERVA_COM_BOTAO = 250
// Altura da barra de controles (com a margem de baixo): com a música na principal, as células
// terminam acima dela.
const RESERVA_CONTROLES = 84

function IconeX({ size }: { size: number }): ReactNode {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" aria-hidden="true">
      <path d="M6 6l12 12M18 6 6 18" />
    </svg>
  )
}

/** Seta para baixo numa bandeja: "mandar para a faixa de baixo". */
function IconeBaixo({ size }: { size: number }): ReactNode {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M12 4v10M8 10l4 4 4-4M4 18h16" />
    </svg>
  )
}

function IconeSom({ mudo, size }: { mudo: boolean; size: number }): ReactNode {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M4 9v6h4l5 4V5L8 9z" />
      {mudo ? <path d="m17 9 5 6M22 9l-5 6" /> : <path d="M16.5 8.5a5 5 0 0 1 0 7" />}
    </svg>
  )
}

/** Colunas da grade da principal para `n` células. */
function colunas(n: number): number {
  if (n <= 1) return 1
  if (n <= 4) return 2
  return 3
}

interface CelulaProps {
  room: RoomView
  celula: Celula
  tile: TileView | undefined
  canvasDe: (trackId: string) => HTMLCanvasElement
  visivel: boolean
  varias: boolean
  volume: VolumePessoa
  onVolumePessoa: (ownerId: string, v: VolumePessoa) => void
  onMover: (trackId: string, lugar: Lugar) => void
  onJeito?: (trackId: string, jeito: 'canto' | 'solta') => void
}

function IconeSolta({ size }: { size: number }): ReactNode {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M14 4h6v6M20 4l-8 8M10 6H6a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-4" />
    </svg>
  )
}

function CelulaPrincipal({ room, celula, tile, canvasDe, visivel, varias, volume, onVolumePessoa, onMover, onJeito }: CelulaProps): ReactNode {
  const s = celula.stream
  const eu = !!s.mine
  const pessoa = room.participants.find((x) => x.id === s.ownerId)
  const nome = eu ? 'Você' : (pessoa?.name ?? 'Alguém')
  const tipo = kindLabel(s.kind)
  const tipoMin = tipo.toLowerCase()
  const rotuloVideo = eu ? `Sua ${tipoMin}` : `${tipo} de ${nome}`
  const rotuloBaixo = eu ? `Mandar minha ${tipoMin} para baixo` : `Mandar ${tipoMin} de ${nome} para baixo`
  const rotuloFechar = eu ? `Esconder minha ${tipoMin}` : `Fechar tela de ${nome}`
  const rotulo = tile ? `camada ${tile.layer + 1} · ${Math.round(tile.ping_ms)} ms ${tile.height}p${tile.fps}` : 'conectando…'
  const some: CSSProperties = { opacity: visivel ? 1 : 0, pointerEvents: visivel ? 'auto' : 'none', transition: 'opacity 0.25s ease' }
  const margem = varias ? 12 : 26
  const lado = varias ? 34 : 40
  const somLabel = (volume.muted ? 'Ativar som de ' : 'Silenciar ') + nome
  const pip = celula.pip
  const canto = celula.corner ?? 'inf_dir'
  const posPip: CSSProperties = {
    ...(canto.endsWith('esq') ? { left: margem } : { right: margem }),
    ...(canto.startsWith('sup') ? { top: margem + 44 } : { bottom: varias ? margem : 96 }),
  }
  const rotuloCanto = eu ? 'Pôr minha câmera no canto' : `Pôr a câmera de ${nome} no canto`
  const rotuloSolta = eu ? 'Soltar minha câmera' : `Soltar a câmera de ${nome}`

  return (
    <div data-testid="celula-principal" style={{ position: 'relative', minWidth: 0, minHeight: 0, background: '#0A100E', borderRadius: varias ? 12 : 0, overflow: 'hidden' }}>
      <span style={CENTRO}>{eu ? `Carregando: sua ${tipoMin}…` : `Carregando: ${tipoMin} de ${nome}…`}</span>
      <CanvasSlot canvas={canvasDe(s.trackId)} label={rotuloVideo} />

      <span style={{ ...PILULA, ...some, position: 'absolute', left: margem, top: margem, height: 30, maxWidth: `calc(100% - ${margem * 2 + 140}px)`, padding: '0 12px', fontSize: 12, fontWeight: 700, letterSpacing: '0.08em', gap: 8, whiteSpace: 'nowrap', overflow: 'hidden' }}>
        <span className="zp-blink" aria-hidden="true" style={{ flex: '0 0 7px', width: 7, height: 7, borderRadius: '50%', background: '#FF5D6E' }} />
        AO VIVO
        <span style={{ fontWeight: 500, letterSpacing: 0, color: 'var(--texto-2)', overflow: 'hidden', textOverflow: 'ellipsis' }}>{`· ${nome} · ${tipo}`}</span>
      </span>

      <span style={{ ...some, position: 'absolute', right: margem, top: margem - 4, display: 'flex', alignItems: 'center', gap: 6 }}>
        {!varias && (
          <span style={{ ...PILULA, height: 30, padding: '0 12px', fontFamily: MONO, fontSize: 11.5, color: 'var(--texto-2)', whiteSpace: 'nowrap' }}>{rotulo}</span>
        )}
        {celula.podeCanto && onJeito && (
          <button type="button" className="zp-x" onClick={() => onJeito(s.trackId, 'canto')} aria-label={rotuloCanto} title="Pôr no canto"
            style={{ ...BTN, height: lado, borderRadius: 12, padding: '0 12px', fontSize: 12, fontWeight: 700, whiteSpace: 'nowrap' }}>
            Pôr no canto
          </button>
        )}
        {!eu && (
          <button type="button" className="zp-x" onClick={() => onVolumePessoa(s.ownerId, { ...volume, muted: !volume.muted })} aria-label={somLabel} title={somLabel}
            style={{ ...BTN, width: lado, height: lado, borderRadius: 12, color: volume.muted ? '#FF8592' : '#E6F4EC' }}>
            <IconeSom mudo={volume.muted} size={18} />
          </button>
        )}
        <button type="button" className="zp-x" onClick={() => onMover(s.trackId, 'faixa')} aria-label={rotuloBaixo} title="Mandar para a faixa de baixo"
          style={{ ...BTN, width: lado, height: lado, borderRadius: 12 }}>
          <IconeBaixo size={18} />
        </button>
        <button type="button" className="zp-x" onClick={() => onMover(s.trackId, 'fechada')} aria-label={rotuloFechar} title={rotuloFechar}
          style={{ ...BTN, width: lado, height: lado, borderRadius: 12 }}>
          <IconeX size={18} />
        </button>
      </span>

      {pip && (
        <span style={{ position: 'absolute', ...posPip, width: varias ? '30%' : '22%', aspectRatio: '16 / 9', borderRadius: 12, overflow: 'hidden', border: '2px solid #0A100E', boxShadow: '0 0 0 1px #3BF08A, 0 10px 30px rgba(0,0,0,0.5)', background: '#0A100E' }}>
          <CanvasSlot canvas={canvasDe(pip.trackId)} label={eu ? 'Sua câmera' : `Câmera de ${nome}`} />
          <span style={{ position: 'absolute', left: 6, bottom: 6, padding: '2px 8px', borderRadius: 6, background: 'rgba(10,16,14,0.8)', fontSize: 11.5, pointerEvents: 'none' }}>{`cam · ${nome}`}</span>
          <button type="button" className="zp-x" onClick={() => onMover(pip.trackId, 'faixa')} aria-label={eu ? 'Tirar minha câmera do canto' : `Tirar a câmera de ${nome} do canto`} title="Mandar a câmera para a faixa de baixo"
            style={{ ...BTN, ...some, position: 'absolute', right: 6, top: 6, width: 32, height: 32, borderRadius: 10 }}>
            <IconeX size={15} />
          </button>
          {onJeito && (
            <button type="button" className="zp-x" onClick={() => onJeito(pip.trackId, 'solta')} aria-label={rotuloSolta} title="Soltar a câmera"
              style={{ ...BTN, ...some, position: 'absolute', right: 44, top: 6, width: 32, height: 32, borderRadius: 10 }}>
              <IconeSolta size={15} />
            </button>
          )}
        </span>
      )}
    </div>
  )
}

export function SalaPrincipal(p: SalaPrincipalProps): ReactNode {
  const { room, celulas } = p
  const ocioso = useOcioso()
  const mudo = p.muted ?? false
  const travas = useTravas(room.mode)

  // Guarda as últimas props para os atalhos sem reinscrever o listener a cada render.
  const ref = useRef({ p, acordar: ocioso.acordar, podeTransmitir: travas.podeTransmitir })
  ref.current = { p, acordar: ocioso.acordar, podeTransmitir: travas.podeTransmitir }

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement | null)?.tagName
      if (tag === 'INPUT' || tag === 'TEXTAREA') return
      const { p: pr, acordar } = ref.current
      acordar()
      if (e.key === 'F11') {
        pr.onFullscreen()
      } else if (e.key === 'Escape') {
        if (document.fullscreenElement) pr.onFullscreen()
      } else if (e.key === 'm' || e.key === 'M') {
        pr.onToggleMute?.()
      } else if (e.key === 'c' || e.key === 'C') {
        // Plano sem transmissão: a câmera fica travada, a tecla não faz nada.
        if (ref.current.podeTransmitir) pr.onToggleCamera()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  const compartilhando = p.share != null
  const primeira = celulas[0]?.stream
  const tilePrimeira = primeira ? p.tiles.find((t) => t.tile === primeira.tile) : undefined
  const n = celulas.length
  const cols = colunas(n)
  const linhas = Math.max(1, Math.ceil(n / cols))
  // Termos do YouTube: a barra de controles da sala não pode ficar por cima do player da música.
  const temMusica = celulas.some((c) => c.stream.kind === 'musica')

  return (
    <main style={{ flex: '0 0 auto', minWidth: 0, display: 'flex', flexDirection: 'column', gap: 16, padding: '20px 28px 28px' }}>
      <SalaCabecalho
        room={room}
        main={primeira ? { track_id: primeira.trackId } : null}
        sharing={compartilhando}
        panelOpen={p.panelOpen ?? true}
        onOpenPanel={p.onOpenPanel ?? noop}
        onStop={p.onStopShare ?? p.onToggleShare}
        invite={p.invite}
      />

      <div
        data-testid="palco"
        className="zp-palco"
        onPointerMove={ocioso.acordar}
        onPointerDown={ocioso.acordar}
        onPointerLeave={ocioso.sair}
        style={{
          cursor: ocioso.visivel ? 'auto' : 'none',
          width: '100%',
          // 16:9 que cabe na altura da janela: barra de título, cabeçalho e margens (e a faixa de
          // baixo, quando há) ficam de fora. Nunca menor que 480 px de largura.
          maxWidth: `max(480px, calc((100vh - ${p.temFaixa === 'botao' ? RESERVA_COM_BOTAO : p.temFaixa ? RESERVA_COM_FAIXA : RESERVA_SEM_FAIXA}px) * 16 / 9))`,
          alignSelf: 'center',
        }}
      >
        <VideoFrame>
          {n > 0 ? (
            <div style={{ position: 'absolute', top: 0, left: 0, right: 0, bottom: temMusica ? RESERVA_CONTROLES : 0, display: 'grid', gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))`, gridTemplateRows: `repeat(${linhas}, minmax(0, 1fr))`, gap: n > 1 ? 8 : 0, padding: n > 1 ? 8 : 0 }}>
              {celulas.map((c) => c.stream.kind === 'musica' ? (
                <CelulaMusica key={c.stream.trackId} onMover={p.onMover} varias={n > 1} />
              ) : (
                <CelulaPrincipal
                  key={c.stream.trackId}
                  room={room}
                  celula={c}
                  tile={p.tiles.find((t) => t.tile === c.stream.tile)}
                  canvasDe={p.canvasDe}
                  visivel={ocioso.visivel}
                  varias={n > 1}
                  volume={p.volumeDe(c.stream.ownerId)}
                  onVolumePessoa={p.onVolumePessoa}
                  onMover={p.onMover}
                  onJeito={p.onJeito}
                />
              ))}
            </div>
          ) : (
            <div className="zp-dots" style={{ position: 'absolute', inset: 0, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: 14, textAlign: 'center', padding: 24 }}>
              <img src="/icone.png" alt="" style={{ width: 104, height: 104, borderRadius: 26, display: 'block', boxShadow: '0 0 0 1px #2A443C, 0 0 36px rgba(59,240,138,0.35)' }} />
              <span style={{ fontFamily: "'Unbounded', sans-serif", fontWeight: 700, fontSize: 20 }}>Nenhuma transmissão na tela principal</span>
              <span style={{ color: 'var(--texto-2)' }}>
                {p.temOutras ? 'Escolha uma transmissão aqui embaixo.' : 'Quando alguém compartilhar a tela, ela aparece aqui.'}
              </span>
            </div>
          )}

          {p.reconnecting && (
            <span role="status" style={{ position: 'absolute', left: '50%', top: 26, transform: 'translateX(-50%)', height: 34, padding: '0 16px', borderRadius: 999, background: '#FFB547', color: '#1A1204', fontWeight: 700, fontSize: 13, display: 'flex', alignItems: 'center', gap: 8 }}>
              <span className="zp-blink" aria-hidden="true" style={{ width: 7, height: 7, borderRadius: '50%', background: '#1A1204' }} />
              Conexão caiu — reconectando…
            </span>
          )}

          <SalaControles
            muted={mudo}
            volume={p.volume ?? 100}
            camOn={p.camOn ?? false}
            sharing={compartilhando}
            smoothingMs={tilePrimeira ? Math.round(tilePrimeira.smoothing_ms) : 0}
            onFullscreen={p.onFullscreen}
            onToggleMute={p.onToggleMute ?? noop}
            onVolume={p.onVolume ?? noop}
            onToggleCam={p.onToggleCamera}
            onToggleShare={p.onToggleShare}
            onLeave={p.onLeave}
            visivel={ocioso.visivel}
            onPreso={ocioso.prender}
            podeTransmitir={travas.podeTransmitir}
          />
        </VideoFrame>
      </div>
    </main>
  )
}
