// Faixa de baixo da sala: as transmissões que não estão na tela principal (miniaturas, quem
// compartilha só o som e as fechadas), ao lado do cartão "Você" quando eu estou transmitindo.
import type { CSSProperties, ReactNode } from 'react'
import type { RoomView } from '../../bridge/types'
import { Avatar } from '../../components/Avatar'
import { CanvasSlot } from './CanvasSlot'
import { SalaVoce } from './SalaVoce'
import { kindLabel, type Lugar, type Stream, type VolumePessoa } from './salaModel'

export interface SalaTilesProps {
  room: RoomView
  /** Transmissões fora da tela principal (na faixa ou fechadas). */
  streams: Stream[]
  lugares: Record<string, Lugar>
  canvasDe: (trackId: string) => HTMLCanvasElement
  volumeDe: (ownerId: string) => VolumePessoa
  onVolumePessoa: (ownerId: string, v: VolumePessoa) => void
  onMover: (trackId: string, lugar: Lugar) => void
  /** Mostra o cartão "Você" (só quando estou transmitindo: o botão de compartilhar já está nos controles). */
  mostrarVoce?: boolean
  /** Minha tela/janela e minha câmera no ar (o cartão Você mostra as que estão na faixa). */
  minhaTela?: Stream | null
  minhaCam?: Stream | null
  meId?: string
  camOn?: boolean
  sharing?: boolean
  onToggleCam?: () => void
  onToggleShare?: () => void
}

// card(on) do script x-dc
const CARD: CSSProperties = {
  display: 'flex', flexDirection: 'column', minWidth: 0, borderRadius: 16, overflow: 'hidden', background: '#0F1916', border: '2px solid #1F332D',
}
const small = (alert: boolean): CSSProperties => ({
  flex: '0 0 18px', width: 18, height: 18, border: 0, padding: 0, background: 'transparent', display: 'flex', alignItems: 'center', justifyContent: 'center',
  color: alert ? '#FF8592' : '#E6F4EC',
})
const BTN: CSSProperties = {
  width: 34, height: 34, borderRadius: 10, border: '1px solid #2A443C', background: 'rgba(10,16,14,0.82)', padding: 0,
  color: '#E6F4EC', display: 'flex', alignItems: 'center', justifyContent: 'center',
}
const CENTRO: CSSProperties = {
  position: 'absolute', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center', padding: 8, textAlign: 'center', fontSize: 12, color: '#8BA498',
}
const EQ: CSSProperties = { width: 2, background: '#3BF08A', borderRadius: 1 }
const MONO = "'JetBrains Mono', monospace"
const SR_ONLY: CSSProperties = { position: 'absolute', width: 1, height: 1, overflow: 'hidden', clip: 'rect(0 0 0 0)' }

function IconeX(): ReactNode {
  return <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true"><path d="M6 6l12 12M18 6 6 18" /></svg>
}
/** Seta para cima saindo da bandeja: "pôr na tela principal". */
function IconeCima(): ReactNode {
  return <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M12 15V5M8 9l4-4 4 4M4 19h16" /></svg>
}
function Equalizador(): ReactNode {
  return (
    <span aria-hidden="true" style={{ display: 'flex', alignItems: 'flex-end', gap: 2, height: 11 }}>
      <span className="zp-eq" style={EQ} /><span className="zp-eq zp-d2" style={EQ} /><span className="zp-eq zp-d3" style={EQ} />
    </span>
  )
}

interface CartaoProps {
  room: RoomView
  stream: Stream
  fechada: boolean
  canvasDe: (trackId: string) => HTMLCanvasElement
  volume: VolumePessoa
  onVolumePessoa: (ownerId: string, v: VolumePessoa) => void
  onMover: (trackId: string, lugar: Lugar) => void
}

function Cartao({ room, stream: s, fechada, canvasDe, volume, onVolumePessoa, onMover }: CartaoProps): ReactNode {
  const pessoa = room.participants.find((x) => x.id === s.ownerId)
  const nome = pessoa?.name ?? 'Alguém'
  const tipo = kindLabel(s.kind)
  const soSom = s.kind === 'audio'
  const audivel = !fechada && !volume.muted && volume.vol > 0
  const muteLabel = (volume.muted ? 'Ativar som de ' : 'Silenciar ') + nome
  const volId = `vol-${s.tile}`
  const aplicar = (v: VolumePessoa) => onVolumePessoa(s.ownerId, v)

  let corpo: ReactNode
  if (fechada) {
    corpo = (
      <div className="zp-dots" style={{ ...CENTRO, flexDirection: 'column', gap: 6, color: '#E6F4EC' }}>
        <span style={{ fontWeight: 600, fontSize: 13 }}>{soSom ? `Som de ${nome} desligado` : `Tela de ${nome} fechada`}</span>
        <span style={{ fontSize: 11.5, color: '#8BA498' }}>sem receber · economiza banda</span>
        <button type="button" className="zp-ghost" onClick={() => onMover(s.trackId, 'faixa')} style={{ marginTop: 2, height: 34, padding: '0 14px', borderRadius: 999, border: '1px solid #3BF08A', background: 'transparent', color: '#3BF08A', fontSize: 12.5, fontWeight: 600 }}>
          {soSom ? 'Ouvir de novo' : 'Reabrir'}
        </button>
      </div>
    )
  } else if (soSom) {
    corpo = (
      <div className="zp-dots" style={{ ...CENTRO, flexDirection: 'column', gap: 8, color: '#E6F4EC' }}>
        <Avatar name={nome} color={pessoa?.color ?? '#5CD6FF'} size={44} />
        <span style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 12.5 }}>
          {audivel && <Equalizador />}
          {`${nome} · só o som`}
        </span>
        <button type="button" className="zp-x" onClick={() => onMover(s.trackId, 'fechada')} aria-label={`Parar de ouvir ${nome}`} title={`Parar de ouvir ${nome}`} style={{ ...BTN, position: 'absolute', right: 6, top: 6 }}>
          <IconeX />
        </button>
      </div>
    )
  } else {
    corpo = (
      <>
        <span style={CENTRO}>Carregando…</span>
        <CanvasSlot canvas={canvasDe(s.trackId)} label={`${tipo} de ${nome}`} />
        {/* Clicar na miniatura leva para a tela principal. */}
        <button type="button" onClick={() => onMover(s.trackId, 'principal')} aria-label={`Assistir ${nome}, ${tipo}`} style={{ position: 'absolute', inset: 0, width: '100%', padding: 0, border: 0, background: 'transparent', cursor: 'pointer' }} />
        <span style={{ position: 'absolute', left: 6, bottom: 6, maxWidth: 'calc(100% - 12px)', padding: '3px 8px', borderRadius: 7, background: 'rgba(10,16,14,0.84)', fontSize: 12, display: 'flex', alignItems: 'center', gap: 6, whiteSpace: 'nowrap', overflow: 'hidden', pointerEvents: 'none' }}>
          {audivel && <Equalizador />}
          <b style={{ fontWeight: 600 }}>{nome}</b><span style={{ color: '#A3BDB1', overflow: 'hidden', textOverflow: 'ellipsis' }}>{`· ${tipo}`}</span>
        </span>
        <span style={{ position: 'absolute', right: 6, top: 6, display: 'flex', gap: 6 }}>
          <button type="button" className="zp-x" onClick={() => onMover(s.trackId, 'principal')} aria-label={`Pôr ${tipo.toLowerCase()} de ${nome} na tela principal`} title="Pôr na tela principal" style={BTN}>
            <IconeCima />
          </button>
          <button type="button" className="zp-x" onClick={() => onMover(s.trackId, 'fechada')} aria-label={`Fechar tela de ${nome}`} title={`Fechar tela de ${nome}`} style={BTN}>
            <IconeX />
          </button>
        </span>
      </>
    )
  }

  return (
    <div style={CARD} data-testid="cartao-faixa">
      <div style={{ position: 'relative', width: '100%', aspectRatio: '16 / 9', background: '#0A100E' }}>{corpo}</div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 10px 6px 6px' }}>
        <Avatar name={nome} color={pessoa?.color ?? '#5CD6FF'} size={22} />
        <button type="button" className="zp-dock" onClick={() => aplicar({ ...volume, muted: !volume.muted })} aria-label={muteLabel} title={muteLabel} disabled={fechada} style={small(volume.muted)}>
          {!volume.muted ? (
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M4 9v6h4l5 4V5L8 9z" /><path d="M16.5 8.5a5 5 0 0 1 0 7" /></svg>
          ) : (
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M4 9v6h4l5 4V5L8 9z" /><path d="m17 9 5 6M22 9l-5 6" /></svg>
          )}
        </button>
        <label htmlFor={volId} style={SR_ONLY}>{`Volume de ${nome}`}</label>
        <input id={volId} className="zp-range" type="range" min={0} max={100} value={volume.vol} disabled={fechada} style={{ flex: 1, minWidth: 0 }}
          onChange={(e) => aplicar({ vol: Number(e.target.value), muted: false })} />
        <span style={{ flex: '0 0 38px', textAlign: 'right', fontFamily: MONO, fontSize: 11, color: '#A3BDB1' }}>{fechada ? '—' : volume.muted ? 'mudo' : `${volume.vol}%`}</span>
      </div>
    </div>
  )
}

export function SalaTiles(p: SalaTilesProps): ReactNode {
  const { room, streams, lugares, mostrarVoce = false } = p
  // Nada para mostrar embaixo: a faixa some (o botão de compartilhar está nos controles).
  if (streams.length === 0 && !mostrarVoce) return null
  return (
    <div role="group" aria-label="Transmissões na sala" style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(200px, 1fr))', gap: 14 }}>
      {streams.map((s) => (
        <Cartao
          key={s.trackId}
          room={room}
          stream={s}
          fechada={lugares[s.trackId] === 'fechada'}
          canvasDe={p.canvasDe}
          volume={p.volumeDe(s.ownerId)}
          onVolumePessoa={p.onVolumePessoa}
          onMover={p.onMover}
        />
      ))}
      {mostrarVoce && (
        <SalaVoce
          room={room}
          meId={p.meId ?? ''}
          camOn={p.camOn ?? false}
          sharing={p.sharing ?? false}
          tela={p.minhaTela ?? null}
          cam={p.minhaCam ?? null}
          lugares={lugares}
          canvasDe={p.canvasDe}
          onMover={p.onMover}
          onToggleCam={p.onToggleCam}
          onToggleShare={p.onToggleShare}
        />
      )}
    </div>
  )
}
