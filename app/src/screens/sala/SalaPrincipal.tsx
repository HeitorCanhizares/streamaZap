// Tela principal da sala: o cabeçalho e uma ou mais transmissões lado a lado (a câmera de quem está
// com a tela aqui vai no canto dela). Cada transmissão tem os próprios controles por cima da imagem
// (som da pessoa, suavização, tela cheia, mandar para baixo, fechar), que somem com o mouse parado.
// A barra da sala (câmera, compartilhar, sair) fica fora daqui, no pé da coluna (SalaScreen).
import { useEffect, useRef, type CSSProperties, type FocusEvent, type ReactNode } from 'react'
import { createPortal } from 'react-dom'
import type { RoomView, ShareView, TileView } from '../../bridge/types'
import { useTravas } from '../../bridge/usePlano'
import { VideoFrame } from '../../components/VideoFrame'
import { CanvasSlot } from './CanvasSlot'
import { CelulaMusica } from './musica/BlocoMusica'
import { SalaCabecalho } from './SalaCabecalho'
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
  /** Altura (px) da faixa de baixo, medida na SalaScreen: o vídeo encolhe para ela caber na janela
   *  sem rolar (0 = sem faixa). */
  alturaFaixa?: number
  share: ShareView | null
  volumeDe: (ownerId: string) => VolumePessoa
  onVolumePessoa: (ownerId: string, v: VolumePessoa) => void
  onMover: (trackId: string, lugar: Lugar) => void
  /** Câmera de quem está com a tela: 'canto' (no canto da tela) ou 'solta' (célula própria). */
  onJeito?: (trackId: string, jeito: 'canto' | 'solta') => void
  /** Tecla C (a câmera em si fica na barra da sala). */
  onToggleCamera: () => void
  onToggleShare: () => void
  /** Parar a transmissão (botão do cabeçalho); sem ele, usa onToggleShare. */
  onStopShare?: () => void
  /** Tecla F11: tela cheia do app inteiro. */
  onFullscreen: () => void
  /** Esc sem transmissão em tela cheia: tira o app da tela cheia. */
  onSairTelaCheia?: () => void
  /** Transmissão em tela cheia (trackId) e como trocar; null = nenhuma. */
  telaCheia?: string | null
  onTelaCheia?: (trackId: string | null) => void
  panelOpen?: boolean
  onOpenPanel?: () => void
  reconnecting?: boolean
  // Texto copiado pelo botão Copiar convite (vem de inviteText na SalaScreen).
  invite?: string
}

const noop = () => {}
const PILULA: CSSProperties = { background: 'rgba(10,16,14,0.8)', borderRadius: 9, display: 'flex', alignItems: 'center' }
const BTN: CSSProperties = { border: '1px solid #2A443C', background: 'rgba(10,16,14,0.8)', color: '#E6F4EC', display: 'flex', alignItems: 'center', justifyContent: 'center', padding: 0, flex: '0 0 auto' }
const CENTRO: CSSProperties = {
  position: 'absolute', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center', padding: 16, textAlign: 'center',
  color: '#8BA498', fontSize: 13,
}
const SR_ONLY: CSSProperties = { position: 'absolute', width: 1, height: 1, overflow: 'hidden', clip: 'rect(0 0 0 0)' }
const MONO = "'JetBrains Mono', monospace"
// Altura que fica fora do vídeo (px), além da faixa de baixo: barra de título (44), a barra da sala
// no pé da coluna (86), o cabeçalho com as margens dele (100) e uma folga.
const RESERVA_BASE = 236

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

function IconeTelaCheia({ size, sair }: { size: number; sair?: boolean }): ReactNode {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d={sair ? 'M9 4v5H4M15 4v5h5M9 20v-5H4M15 20v-5h5' : 'M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5'} />
    </svg>
  )
}

function IconeSolta({ size }: { size: number }): ReactNode {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M14 4h6v6M20 4l-8 8M10 6H6a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-4" />
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
  /** Esta é a transmissão em tela cheia (ocupa a tela toda, por cima do app). */
  cheia?: boolean
  onTelaCheia?: (trackId: string | null) => void
  /** Mouse em cima dos controles ou foco neles: não deixam sumir (useOcioso). */
  onPreso?: (preso: boolean) => void
}

/** Mouse ou foco do teclado num grupo de controles: segura os controles visíveis. */
function prendeControles(onPreso?: (preso: boolean) => void) {
  return {
    onPointerEnter: () => onPreso?.(true),
    onPointerLeave: () => onPreso?.(false),
    onFocus: () => onPreso?.(true),
    onBlur: (e: FocusEvent<HTMLElement>) => {
      if (!e.currentTarget.contains(e.relatedTarget as Node | null)) onPreso?.(false)
    },
  }
}

function CelulaPrincipal({ room, celula, tile, canvasDe, visivel, varias, volume, onVolumePessoa, onMover, onJeito, cheia = false, onTelaCheia, onPreso }: CelulaProps): ReactNode {
  const s = celula.stream
  const eu = !!s.mine
  const pessoa = room.participants.find((x) => x.id === s.ownerId)
  const nome = eu ? 'Você' : (pessoa?.name ?? 'Alguém')
  const tipo = kindLabel(s.kind)
  const tipoMin = tipo.toLowerCase()
  const rotuloVideo = eu ? `Sua ${tipoMin}` : `${tipo} de ${nome}`
  const rotuloBaixo = eu ? `Mandar minha ${tipoMin} para baixo` : `Mandar ${tipoMin} de ${nome} para baixo`
  const rotuloFechar = eu ? `Esconder minha ${tipoMin}` : `Fechar tela de ${nome}`
  const rotuloCheia = eu ? `Pôr minha ${tipoMin} em tela cheia` : `Pôr ${tipoMin} de ${nome} em tela cheia`
  const rotulo = tile ? `camada ${tile.layer + 1} · ${Math.round(tile.ping_ms)} ms ${tile.height}p${tile.fps}` : 'conectando…'
  const suavizacao = `Suavização automática, ${tile ? Math.round(tile.smoothing_ms) : 0} ms`
  const some: CSSProperties = { opacity: visivel ? 1 : 0, pointerEvents: visivel ? 'auto' : 'none', transition: 'opacity 0.25s ease' }
  const compacta = varias && !cheia
  const margem = cheia ? 24 : compacta ? 12 : 26
  const lado = compacta ? 34 : 40
  const somLabel = (volume.muted ? 'Ativar som de ' : 'Silenciar ') + nome
  const volId = `vol-principal-${s.tile}`
  const pip = celula.pip
  const canto = celula.corner ?? 'inf_dir'
  // O canto de baixo fica acima da linha de controles da transmissão.
  const posPip: CSSProperties = {
    ...(canto.endsWith('esq') ? { left: margem } : { right: margem }),
    ...(canto.startsWith('sup') ? { top: margem + 44 } : { bottom: margem + lado + 8 }),
  }
  const rotuloCanto = eu ? 'Pôr minha câmera no canto' : `Pôr a câmera de ${nome} no canto`
  const rotuloSolta = eu ? 'Soltar minha câmera' : `Soltar a câmera de ${nome}`
  // A célula sumiu com o mouse em cima dos controles (mandou para baixo, fechou): solta.
  useEffect(() => () => onPreso?.(false), [onPreso])

  return (
    <div data-testid="celula-principal" style={{ position: 'relative', minWidth: 0, minHeight: 0, background: cheia ? '#000' : '#0A100E', borderRadius: compacta ? 12 : 0, overflow: 'hidden', ...(cheia ? { width: '100%', height: '100%' } : {}) }}>
      <span style={CENTRO}>{eu ? `Carregando: sua ${tipoMin}…` : `Carregando: ${tipoMin} de ${nome}…`}</span>
      <CanvasSlot canvas={canvasDe(s.trackId)} label={rotuloVideo} />

      <span style={{ ...PILULA, ...some, position: 'absolute', left: margem, top: margem, height: 30, maxWidth: `calc(100% - ${margem * 2 + 140}px)`, padding: '0 12px', fontSize: 12, fontWeight: 700, letterSpacing: '0.08em', gap: 8, whiteSpace: 'nowrap', overflow: 'hidden' }}>
        <span className="zp-blink" aria-hidden="true" style={{ flex: '0 0 7px', width: 7, height: 7, borderRadius: '50%', background: '#FF5D6E' }} />
        AO VIVO
        <span style={{ fontWeight: 500, letterSpacing: 0, color: 'var(--texto-2)', overflow: 'hidden', textOverflow: 'ellipsis' }}>{`· ${nome} · ${tipo}`}</span>
      </span>

      <span {...prendeControles(onPreso)} style={{ ...some, position: 'absolute', right: margem, top: margem - 4, display: 'flex', alignItems: 'center', gap: 6 }}>
        {!compacta && (
          <span style={{ ...PILULA, height: 30, padding: '0 12px', fontFamily: MONO, fontSize: 11.5, color: 'var(--texto-2)', whiteSpace: 'nowrap' }}>{rotulo}</span>
        )}
        {cheia ? (
          onTelaCheia && (
            <button type="button" className="zp-x" onClick={() => onTelaCheia(null)} aria-label="Sair da tela cheia" title="Sair da tela cheia (Esc)"
              style={{ ...BTN, height: lado, borderRadius: 12, padding: '0 14px', gap: 8, fontSize: 13, fontWeight: 600, whiteSpace: 'nowrap' }}>
              <IconeTelaCheia size={18} sair />
              Sair da tela cheia
              <span aria-hidden="true" style={{ fontFamily: MONO, fontSize: 11, color: '#7E978B' }}>Esc</span>
            </button>
          )
        ) : (
          <>
            {celula.podeCanto && onJeito && (
              <button type="button" className="zp-x" onClick={() => onJeito(s.trackId, 'canto')} aria-label={rotuloCanto} title="Pôr no canto"
                style={{ ...BTN, height: lado, borderRadius: 12, padding: '0 12px', fontSize: 12, fontWeight: 700, whiteSpace: 'nowrap' }}>
                Pôr no canto
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
          </>
        )}
      </span>

      {/* Linha de baixo: o som desta pessoa, a suavização e a tela cheia desta transmissão. */}
      <span data-testid="controles-transmissao" {...prendeControles(onPreso)} style={{ ...some, position: 'absolute', left: margem, right: margem, bottom: margem - 4, display: 'flex', alignItems: 'center', gap: 8, minWidth: 0 }}>
        {!eu && (
          <>
            <button type="button" className="zp-x" onClick={() => onVolumePessoa(s.ownerId, { ...volume, muted: !volume.muted })} aria-label={somLabel} title={somLabel}
              style={{ ...BTN, width: lado, height: lado, borderRadius: 12, color: volume.muted ? '#FF8592' : '#E6F4EC' }}>
              <IconeSom mudo={volume.muted} size={18} />
            </button>
            <label htmlFor={volId} style={SR_ONLY}>{`Volume de ${nome}`}</label>
            <input id={volId} className="zp-range" type="range" min={0} max={100} value={volume.vol}
              onChange={(e) => onVolumePessoa(s.ownerId, { vol: Number(e.currentTarget.value), muted: false })}
              style={{ flex: `0 1 ${compacta ? 80 : 120}px`, minWidth: 40 }} />
            {!compacta && (
              <span style={{ ...PILULA, height: 26, padding: '0 8px', fontFamily: MONO, fontSize: 11.5, color: 'var(--texto-2)', whiteSpace: 'nowrap' }}>
                {volume.muted ? 'mudo' : `${volume.vol}%`}
              </span>
            )}
          </>
        )}
        <span style={{ flex: 1 }} />
        {!compacta && (
          <span title={suavizacao} style={{ ...PILULA, height: lado, padding: '0 12px', borderRadius: 12, border: '1px solid #2A443C', gap: 8, fontFamily: MONO, fontSize: 12, color: 'var(--texto-2)', whiteSpace: 'nowrap' }}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" aria-hidden="true">
              <path d="M3 12c2.5-5 5.5-5 8 0s5.5 5 8 0" />
            </svg>
            auto
            <span style={SR_ONLY}>{`, ${suavizacao}`}</span>
          </span>
        )}
        {!cheia && onTelaCheia && (
          <button type="button" className="zp-x" onClick={() => onTelaCheia(s.trackId)} aria-label={rotuloCheia} title="Tela cheia desta transmissão (F)"
            style={{ ...BTN, width: lado, height: lado, borderRadius: 12 }}>
            <IconeTelaCheia size={18} />
          </button>
        )}
      </span>

      {pip && (
        <span style={{ position: 'absolute', ...posPip, width: compacta ? '30%' : '22%', aspectRatio: '16 / 9', borderRadius: 12, overflow: 'hidden', border: '2px solid #0A100E', boxShadow: '0 0 0 1px #3BF08A, 0 10px 30px rgba(0,0,0,0.5)', background: '#0A100E' }}>
          <CanvasSlot canvas={canvasDe(pip.trackId)} label={eu ? 'Sua câmera' : `Câmera de ${nome}`} />
          <span style={{ position: 'absolute', left: 6, bottom: 6, padding: '2px 8px', borderRadius: 6, background: 'rgba(10,16,14,0.8)', fontSize: 11.5, pointerEvents: 'none' }}>{`cam · ${nome}`}</span>
          {!cheia && (
            <button type="button" className="zp-x" onClick={() => onMover(pip.trackId, 'faixa')} aria-label={eu ? 'Tirar minha câmera do canto' : `Tirar a câmera de ${nome} do canto`} title="Mandar a câmera para a faixa de baixo"
              style={{ ...BTN, ...some, position: 'absolute', right: 6, top: 6, width: 32, height: 32, borderRadius: 10 }}>
              <IconeX size={15} />
            </button>
          )}
          {!cheia && onJeito && (
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
  const travas = useTravas(room.mode)

  // Guarda as últimas props para os atalhos sem reinscrever o listener a cada render.
  const ref = useRef({ p, acordar: ocioso.acordar, podeTransmitir: travas.podeTransmitir })
  ref.current = { p, acordar: ocioso.acordar, podeTransmitir: travas.podeTransmitir }

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement | null)?.tagName
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return
      // Ctrl+C, Ctrl+F e companhia são do sistema, não da sala.
      if (e.ctrlKey || e.metaKey || e.altKey) return
      const { p: pr, acordar } = ref.current
      acordar()
      // Transmissão com vídeo que as teclas F e M usam: a da tela cheia, senão a primeira da principal.
      const videos = pr.celulas.filter((c) => c.stream.kind !== 'musica')
      const daCheia = pr.telaCheia ? videos.find((c) => c.stream.trackId === pr.telaCheia) : undefined
      if (e.key === 'F11') {
        e.preventDefault()
        pr.onFullscreen()
      } else if (e.key === 'Escape') {
        if (pr.telaCheia) pr.onTelaCheia?.(null)
        else pr.onSairTelaCheia?.()
      } else if (e.key === 'f' || e.key === 'F') {
        if (pr.telaCheia) pr.onTelaCheia?.(null)
        else if (videos[0]) pr.onTelaCheia?.(videos[0].stream.trackId)
      } else if (e.key === 'm' || e.key === 'M') {
        // O som é de cada transmissão: M silencia (ou devolve) a pessoa que está na tela.
        const alvo = daCheia ?? videos.find((c) => !c.stream.mine)
        if (alvo && !alvo.stream.mine) {
          const v = pr.volumeDe(alvo.stream.ownerId)
          pr.onVolumePessoa(alvo.stream.ownerId, { ...v, muted: !v.muted })
        }
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
  const n = celulas.length
  const cols = colunas(n)
  const linhas = Math.max(1, Math.ceil(n / cols))
  const celulaCheia = p.telaCheia ? celulas.find((c) => c.stream.trackId === p.telaCheia && c.stream.kind !== 'musica') : undefined

  const celulaDe = (c: Celula, cheia: boolean) => (
    <CelulaPrincipal
      key={c.stream.trackId}
      room={room}
      celula={c}
      tile={p.tiles.find((t) => t.tile === c.stream.tile)}
      canvasDe={p.canvasDe}
      visivel={ocioso.visivel}
      varias={!cheia && n > 1}
      volume={p.volumeDe(c.stream.ownerId)}
      onVolumePessoa={p.onVolumePessoa}
      onMover={p.onMover}
      onJeito={p.onJeito}
      cheia={cheia}
      onTelaCheia={p.onTelaCheia}
      onPreso={ocioso.prender}
    />
  )

  return (
    <main style={{ flex: '0 0 auto', minWidth: 0, display: 'flex', flexDirection: 'column', gap: 16, padding: '20px 28px 16px' }}>
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
        onPointerMove={ocioso.acordar}
        onPointerDown={ocioso.acordar}
        onPointerLeave={ocioso.sair}
        style={{
          cursor: ocioso.visivel ? 'auto' : 'none',
          width: '100%',
          // 16:9 que cabe na altura da janela: barra de título, cabeçalho, margens e a barra da sala
          // (e a faixa de baixo, quando há) ficam de fora. Nunca menor que 400 px de largura (perto
          // disso só na janela mínima, 1100×700, com o cartão da música ou o Você embaixo).
          maxWidth: `max(400px, calc((100vh - ${RESERVA_BASE + Math.round(p.alturaFaixa ?? 0)}px) * 16 / 9))`,
          alignSelf: 'center',
        }}
      >
        <VideoFrame corners={false}>
          {n > 0 ? (
            <div style={{ position: 'absolute', inset: 0, display: 'grid', gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))`, gridTemplateRows: `repeat(${linhas}, minmax(0, 1fr))`, gap: n > 1 ? 8 : 0, padding: n > 1 ? 8 : 0 }}>
              {celulas.map((c) => c.stream.kind === 'musica' ? (
                <CelulaMusica key={c.stream.trackId} onMover={p.onMover} varias={n > 1} />
              ) : c === celulaCheia ? (
                // O canvas está na tela cheia; aqui fica só o lugar dela.
                <div key={c.stream.trackId} data-testid="celula-em-tela-cheia" style={{ position: 'relative', minWidth: 0, minHeight: 0, background: '#0A100E', borderRadius: n > 1 ? 12 : 0 }}>
                  <span style={CENTRO}>Em tela cheia</span>
                </div>
              ) : (
                celulaDe(c, false)
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
        </VideoFrame>
      </div>

      {/* Tela cheia de uma transmissão: ela sozinha por cima de tudo (a janela vai para tela cheia). */}
      {celulaCheia && createPortal(
        <div
          role="dialog"
          aria-label={`${celulaCheia.stream.mine ? 'Sua transmissão' : 'Transmissão'} em tela cheia`}
          data-testid="tela-cheia"
          onPointerMove={ocioso.acordar}
          onPointerDown={ocioso.acordar}
          style={{ position: 'fixed', inset: 0, zIndex: 1000, background: '#000', cursor: ocioso.visivel ? 'auto' : 'none' }}
        >
          {celulaDe(celulaCheia, true)}
        </div>,
        document.body,
      )}
    </main>
  )
}
