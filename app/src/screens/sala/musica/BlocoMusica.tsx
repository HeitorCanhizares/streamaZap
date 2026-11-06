// A música da sala como transmissão: na tela principal (célula com a barra em cima) ou na faixa de
// baixo (cartão largo com a barra embaixo). Os controles ficam sempre fora da área do player
// (termos do YouTube: nada por cima dele).
//
// O player é um só e nunca muda de lugar no DOM: mudar recarregaria o vídeo (e podia trazer outro
// anúncio), e a música troca de lugar sozinha quando alguém começa ou para de transmitir. A célula
// e o cartão só deixam uma vaga vazia (VagaMusica); a CamadaMusica, filha da coluna da sala, põe o
// player por cima do retângulo dessa vaga.
import { createContext, useContext, useEffect, useState, type CSSProperties, type ReactNode, type RefObject } from 'react'
import type { MusicNowView, MusicReport } from '../../../bridge/types'
import type { Lugar, VolumePessoa } from '../salaModel'
import { MUSICA } from '../salaModel'
import { PlayerYoutube } from './PlayerYoutube'

/** O que a célula, o cartão e a camada da música precisam (vem da SalaScreen). */
export interface MusicaCtx {
  atual: MusicNowView
  /** Volume que escolhi para a música (0..100) e se está muda para mim. */
  volume: VolumePessoa
  onVolume: (v: VolumePessoa) => void
  /** Volume geral da sala por cima do da música. */
  volumeGeral: number
  mudoGeral: boolean
  onReport: (r: MusicReport) => void
  travado: boolean
  onTravado: (v: boolean) => void
  /** A célula ou o cartão avisa onde está a vaga do player (ref de callback). */
  registrarVaga: (el: HTMLElement | null) => void
}

export const MusicaContexto = createContext<MusicaCtx | null>(null)

const BTN: CSSProperties = {
  flex: '0 0 auto', width: 32, height: 32, borderRadius: 10, border: '1px solid #2A443C', background: 'rgba(10,16,14,0.82)', padding: 0,
  color: '#E6F4EC', display: 'flex', alignItems: 'center', justifyContent: 'center',
}
const MONO = "'JetBrains Mono', monospace"

function Icone({ d, size = 16 }: { d: string; size?: number }): ReactNode {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d={d} />
    </svg>
  )
}
const SOM = 'M4 9v6h4l5 4V5L8 9z M16.5 8.5a5 5 0 0 1 0 7'
const MUDO = 'M4 9v6h4l5 4V5L8 9z m13 0 5 6M22 9l-5 6'
const CIMA = 'M12 15V5M8 9l4-4 4 4M4 19h16'
const BAIXO = 'M12 4v10M8 10l4 4 4-4M4 18h16'
const X = 'M6 6l12 12M18 6 6 18'

/** Barra da música: título, AO VIVO, aviso de anúncio e os botões. */
function Barra({ ctx, lugar, onMover }: { ctx: MusicaCtx; lugar: 'principal' | 'faixa'; onMover: (trackId: string, lugar: Lugar) => void }): ReactNode {
  const { item } = ctx.atual
  const mudo = ctx.volume.muted
  const somLabel = mudo ? 'Ativar o som da música' : 'Silenciar a música'
  return (
    <div style={{ flex: '0 0 auto', display: 'flex', alignItems: 'center', gap: 8, padding: '6px 8px', minWidth: 0, background: '#0F1916' }}>
      <span aria-hidden="true" style={{ flex: '0 0 auto', fontSize: 15 }}>♪</span>
      <span style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column' }}>
        <span style={{ fontSize: 12.5, fontWeight: 600, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
          {item.live && <b style={{ color: '#FF5D6E', fontFamily: MONO, fontSize: 10.5, marginRight: 6 }}>AO VIVO</b>}
          {item.title}
        </span>
        <span role={ctx.travado ? 'status' : undefined} style={{ fontSize: 11.5, color: ctx.travado ? '#FFB547' : '#8BA498', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
          {ctx.travado ? 'Anúncio ou carregando · você volta junto da sala' : `pôs: ${item.added_by_name}`}
        </span>
      </span>
      <button type="button" className="zp-x" onClick={() => ctx.onVolume({ ...ctx.volume, muted: !mudo })} aria-label={somLabel} title={somLabel} style={{ ...BTN, color: mudo ? '#FF8592' : '#E6F4EC' }}>
        <Icone d={mudo ? MUDO : SOM} />
      </button>
      {lugar === 'principal' ? (
        <button type="button" className="zp-x" onClick={() => onMover(MUSICA, 'faixa')} aria-label="Mandar a música para baixo" title="Mandar para a faixa de baixo" style={BTN}>
          <Icone d={BAIXO} />
        </button>
      ) : (
        <button type="button" className="zp-x" onClick={() => onMover(MUSICA, 'principal')} aria-label="Pôr a música na tela principal" title="Pôr na tela principal" style={BTN}>
          <Icone d={CIMA} />
        </button>
      )}
      <button type="button" className="zp-x" onClick={() => onMover(MUSICA, 'fechada')} aria-label="Fechar a música (parar de tocar para mim)" title="Fechar a música: para de tocar só para você" style={BTN}>
        <Icone d={X} />
      </button>
    </div>
  )
}

/** Espaço vazio onde a CamadaMusica põe o player. */
function VagaMusica({ ctx }: { ctx: MusicaCtx }): ReactNode {
  return <div ref={ctx.registrarVaga} data-testid="vaga-musica" style={{ position: 'absolute', inset: 0 }} />
}

/** Célula da tela principal com a música: a barra em cima, a vaga do player ocupando o resto. */
export function CelulaMusica({ onMover, varias }: { onMover: (trackId: string, lugar: Lugar) => void; varias: boolean }): ReactNode {
  const ctx = useContext(MusicaContexto)
  if (!ctx) return null
  return (
    <div data-testid="celula-musica" style={{ position: 'relative', minWidth: 0, minHeight: 0, display: 'flex', flexDirection: 'column', background: '#0A100E', borderRadius: varias ? 12 : 0, overflow: 'hidden' }}>
      <Barra ctx={ctx} lugar="principal" onMover={onMover} />
      <div style={{ position: 'relative', flex: 1, minHeight: 200, background: '#000' }}>
        <VagaMusica ctx={ctx} />
      </div>
    </div>
  )
}

/** Cartão da faixa de baixo com a música: largo (duas colunas) para o player ter 200 px de altura. */
export function CartaoMusica({ onMover }: { onMover: (trackId: string, lugar: Lugar) => void }): ReactNode {
  const ctx = useContext(MusicaContexto)
  if (!ctx) return null
  const volId = 'vol-musica'
  return (
    <div data-testid="cartao-musica" style={{ gridColumn: 'span 2', minWidth: 356, display: 'flex', flexDirection: 'column', borderRadius: 16, overflow: 'hidden', background: '#0F1916', border: '2px solid #1F332D' }}>
      <div style={{ position: 'relative', width: '100%', aspectRatio: '16 / 9', minHeight: 200, background: '#000' }}>
        <VagaMusica ctx={ctx} />
      </div>
      <Barra ctx={ctx} lugar="faixa" onMover={onMover} />
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '0 10px 8px' }}>
        <label htmlFor={volId} style={{ fontSize: 11.5, color: '#8BA498' }}>Volume</label>
        <input id={volId} className="zp-range" type="range" min={0} max={100} value={ctx.volume.vol} style={{ flex: 1, minWidth: 0 }}
          onChange={(e) => ctx.onVolume({ vol: Number(e.target.value), muted: false })} />
        <span style={{ flex: '0 0 38px', textAlign: 'right', fontFamily: MONO, fontSize: 11, color: '#A3BDB1' }}>{ctx.volume.muted ? 'mudo' : `${ctx.volume.vol}%`}</span>
      </div>
    </div>
  )
}

interface Retangulo { left: number; top: number; width: number; height: number }

/** Retângulo da vaga em coordenadas do conteúdo da coluna (que rola). */
function retanguloNaColuna(vaga: HTMLElement, coluna: HTMLElement): Retangulo {
  const v = vaga.getBoundingClientRect()
  const c = coluna.getBoundingClientRect()
  return { left: v.left - c.left + coluna.scrollLeft, top: v.top - c.top + coluna.scrollTop, width: v.width, height: v.height }
}

const mesmo = (a: Retangulo | null, b: Retangulo): boolean =>
  !!a && Math.abs(a.left - b.left) < 0.5 && Math.abs(a.top - b.top) < 0.5 && Math.abs(a.width - b.width) < 0.5 && Math.abs(a.height - b.height) < 0.5

/**
 * O player da música, um só, filho da coluna da sala (position: relative). Fica sobre o retângulo
 * da vaga e o acompanha a cada quadro (a vaga muda de lugar quando a música sobe ou desce, o painel
 * abre ou fecha, a janela muda de tamanho). Como está dentro da coluna, rola junto com ela. Sem
 * vaga por um instante (a célula saiu e o cartão ainda não entrou), some sem desmontar.
 */
export function CamadaMusica({ coluna, vaga }: { coluna: RefObject<HTMLElement | null>; vaga: HTMLElement | null }): ReactNode {
  const ctx = useContext(MusicaContexto)
  const [ret, setRet] = useState<Retangulo | null>(null)

  useEffect(() => {
    if (!vaga) return
    let vivo = true
    let ultimo: Retangulo | null = null
    const medir = () => {
      const c = coluna.current
      if (!c || !vivo) return
      const r = retanguloNaColuna(vaga, c)
      if (!mesmo(ultimo, r)) {
        ultimo = r
        setRet(r)
      }
    }
    medir()
    // A cada quadro (só lê duas posições); sem requestAnimationFrame, a cada 100 ms.
    const raf = typeof globalThis.requestAnimationFrame === 'function'
    let id = 0
    const laco = () => {
      medir()
      if (vivo) id = raf ? globalThis.requestAnimationFrame(laco) : window.setTimeout(laco, 100)
    }
    id = raf ? globalThis.requestAnimationFrame(laco) : window.setTimeout(laco, 100)
    return () => {
      vivo = false
      if (raf) globalThis.cancelAnimationFrame(id)
      else window.clearTimeout(id)
    }
  }, [vaga, coluna])

  if (!ctx) return null
  const mudo = ctx.volume.muted || ctx.mudoGeral
  const volume = Math.round((ctx.volume.vol * ctx.volumeGeral) / 100)
  const visivel = vaga !== null && ret !== null
  return (
    <div
      data-testid="camada-musica"
      style={{
        position: 'absolute',
        left: ret?.left ?? 0,
        top: ret?.top ?? 0,
        width: ret?.width ?? 0,
        height: ret?.height ?? 0,
        zIndex: 3,
        visibility: visivel ? 'visible' : 'hidden',
      }}
    >
      <PlayerYoutube atual={ctx.atual} volume={volume} mudo={mudo} onReport={ctx.onReport} onTravado={ctx.onTravado} />
    </div>
  )
}
