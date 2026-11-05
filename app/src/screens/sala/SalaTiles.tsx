// Faixa de baixo da sala: as transmissões que não estão na tela principal (miniaturas e quem
// compartilha só o som), ao lado do cartão "Você" quando eu estou transmitindo. Uma miniatura
// fechada (a minha ou a de outra pessoa) vira um botão pequeno numa linha embaixo, que a reabre.
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
  /** Cartão Você oculto pelo usuário: no lugar dele fica o botão "Mostrar meu cartão". */
  voceOculto?: boolean
  onVoceOculto?: (oculto: boolean) => void
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
  canvasDe: (trackId: string) => HTMLCanvasElement
  volume: VolumePessoa
  onVolumePessoa: (ownerId: string, v: VolumePessoa) => void
  onMover: (trackId: string, lugar: Lugar) => void
}

function Cartao({ room, stream: s, canvasDe, volume, onVolumePessoa, onMover }: CartaoProps): ReactNode {
  const pessoa = room.participants.find((x) => x.id === s.ownerId)
  const nome = pessoa?.name ?? 'Alguém'
  const tipo = kindLabel(s.kind)
  const soSom = s.kind === 'audio'
  const audivel = !volume.muted && volume.vol > 0
  const muteLabel = (volume.muted ? 'Ativar som de ' : 'Silenciar ') + nome
  const volId = `vol-${s.tile}`
  const aplicar = (v: VolumePessoa) => onVolumePessoa(s.ownerId, v)

  let corpo: ReactNode
  if (soSom) {
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
        <button type="button" className="zp-dock" onClick={() => aplicar({ ...volume, muted: !volume.muted })} aria-label={muteLabel} title={muteLabel} style={small(volume.muted)}>
          {!volume.muted ? (
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M4 9v6h4l5 4V5L8 9z" /><path d="M16.5 8.5a5 5 0 0 1 0 7" /></svg>
          ) : (
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M4 9v6h4l5 4V5L8 9z" /><path d="m17 9 5 6M22 9l-5 6" /></svg>
          )}
        </button>
        <label htmlFor={volId} style={SR_ONLY}>{`Volume de ${nome}`}</label>
        <input id={volId} className="zp-range" type="range" min={0} max={100} value={volume.vol} style={{ flex: 1, minWidth: 0 }}
          onChange={(e) => aplicar({ vol: Number(e.target.value), muted: false })} />
        <span style={{ flex: '0 0 38px', textAlign: 'right', fontFamily: MONO, fontSize: 11, color: '#A3BDB1' }}>{volume.muted ? 'mudo' : `${volume.vol}%`}</span>
      </div>
    </div>
  )
}

const BOTAO_FECHADA: CSSProperties = {
  height: 34, padding: '0 14px', borderRadius: 999, border: '1px solid #2A443C', background: 'transparent', color: '#A3BDB1',
  fontSize: 12.5, fontWeight: 600, display: 'flex', alignItems: 'center', gap: 8, whiteSpace: 'nowrap', maxWidth: '100%', overflow: 'hidden',
}

function IconeReabrir(): ReactNode {
  return <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z" /><circle cx="12" cy="12" r="3" /></svg>
}

export function SalaTiles(p: SalaTilesProps): ReactNode {
  const { room, streams, lugares, mostrarVoce = false } = p
  // Nada para mostrar embaixo: a faixa some (o botão de compartilhar está nos controles).
  if (streams.length === 0 && !mostrarVoce) return null
  const abertas = streams.filter((s) => lugares[s.trackId] !== 'fechada')
  const fechadas = streams.filter((s) => lugares[s.trackId] === 'fechada')
  const voceAberto = mostrarVoce && !p.voceOculto
  const temBotoes = fechadas.length > 0 || (mostrarVoce && p.voceOculto)
  return (
    <div role="group" aria-label="Transmissões na sala" style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(200px, 1fr))', gap: 14 }}>
      {abertas.map((s) => (
        <Cartao
          key={s.trackId}
          room={room}
          stream={s}
          canvasDe={p.canvasDe}
          volume={p.volumeDe(s.ownerId)}
          onVolumePessoa={p.onVolumePessoa}
          onMover={p.onMover}
        />
      ))}
      {voceAberto && (
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
          onOcultar={p.onVoceOculto ? () => p.onVoceOculto?.(true) : undefined}
        />
      )}
      {temBotoes && (
        // Miniaturas fechadas: um botão pequeno cada, na linha de baixo inteira.
        <div role="group" aria-label="Miniaturas fechadas" style={{ gridColumn: '1 / -1', display: 'flex', flexWrap: 'wrap', gap: 8 }}>
          {fechadas.map((s) => {
            const nome = room.participants.find((x) => x.id === s.ownerId)?.name ?? 'Alguém'
            const soSom = s.kind === 'audio'
            const rotulo = soSom ? `Ouvir ${nome} de novo` : `Reabrir ${kindLabel(s.kind).toLowerCase()} de ${nome}`
            return (
              <button key={s.trackId} type="button" className="zp-ghost" onClick={() => p.onMover(s.trackId, 'faixa')} aria-label={rotulo} title={`${rotulo} (fechada: sem receber)`} style={BOTAO_FECHADA}>
                <IconeReabrir />
                <span style={{ overflow: 'hidden', textOverflow: 'ellipsis' }}>{`${nome} · ${soSom ? 'só o som' : kindLabel(s.kind)}`}</span>
              </button>
            )
          })}
          {mostrarVoce && p.voceOculto && (
            <button type="button" className="zp-ghost" onClick={() => p.onVoceOculto?.(false)} aria-label="Mostrar meu cartão" title="Mostrar meu cartão" style={BOTAO_FECHADA}>
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><circle cx="12" cy="8" r="4" /><path d="M4 21a8 8 0 0 1 16 0" /></svg>
              Você
            </button>
          )}
        </div>
      )}
    </div>
  )
}
