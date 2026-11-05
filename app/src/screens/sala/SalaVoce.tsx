// Cartão "Você" da faixa de baixo: a prévia ao vivo do que eu transmito (tela/janela e, no canto,
// a câmera), quando ela não está na tela principal. O canvas é o do useAssinaturas (prévia local do
// núcleo, sem rede): subir para a principal só move o canvas.
import type { CSSProperties, ReactNode } from 'react'
import type { RoomView } from '../../bridge/types'
import { CanvasSlot } from './CanvasSlot'
import type { Lugar, Stream } from './salaModel'

export interface SalaVoceProps {
  room: RoomView
  meId: string
  camOn: boolean
  sharing: boolean
  /** Minha tela/janela e minha câmera, se estão no ar (em qualquer lugar). */
  tela?: Stream | null
  cam?: Stream | null
  lugares?: Record<string, Lugar>
  canvasDe?: (trackId: string) => HTMLCanvasElement
  onMover?: (trackId: string, lugar: Lugar) => void
  onToggleCam?: () => void
  onToggleShare?: () => void
  /** Oculta o cartão inteiro (volta pelo botão "Mostrar meu cartão" da faixa). */
  onOcultar?: () => void
}

// card(on) do script x-dc
const CARD: CSSProperties = {
  display: 'flex', flexDirection: 'column', minWidth: 0, borderRadius: 16, overflow: 'hidden', background: '#0F1916', border: '2px solid #1F332D',
}
const meBtn = (on: boolean): CSSProperties => ({
  flex: 1, minWidth: 0, height: 44, padding: '0 6px', borderRadius: 12, display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 6, fontSize: 13, fontWeight: 600,
  whiteSpace: 'nowrap', overflow: 'hidden',
  border: '1px solid ' + (on ? '#3BF08A' : '#2A443C'), background: on ? 'rgba(59,240,138,0.12)' : 'transparent', color: on ? '#3BF08A' : '#A3BDB1',
})
const ROTULO: CSSProperties = {
  position: 'absolute', left: 6, bottom: 6, maxWidth: 'calc(100% - 12px)', padding: '2px 7px', borderRadius: 7, background: 'rgba(10,16,14,0.84)',
  fontSize: 11.5, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis', pointerEvents: 'none',
}
const CENTRO: CSSProperties = {
  position: 'absolute', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center', padding: 8, textAlign: 'center',
  fontSize: 12.5, color: '#8BA498',
}
const BTN: CSSProperties = {
  width: 32, height: 32, borderRadius: 9, border: '1px solid #2A443C', background: 'rgba(10,16,14,0.82)', padding: 0,
  color: '#E6F4EC', display: 'flex', alignItems: 'center', justifyContent: 'center',
}
const CANTO: CSSProperties = { inset: 'auto 6px 6px auto', width: '36%', height: 'auto', aspectRatio: '16 / 9', borderRadius: 8, border: '1px solid #2A443C', overflow: 'hidden', background: '#0A100E' }

function IconeOlho({ aberto }: { aberto: boolean }): ReactNode {
  return aberto ? (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z" /><circle cx="12" cy="12" r="3" /></svg>
  ) : (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M3 3l18 18M10.6 5.1A10.4 10.4 0 0 1 12 5c6.5 0 10 7 10 7a17 17 0 0 1-3.2 4.1M6.6 6.6A17 17 0 0 0 2 12s3.5 7 10 7a9.8 9.8 0 0 0 5.4-1.6" /></svg>
  )
}
function IconeCima(): ReactNode {
  return <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M12 15V5M8 9l4-4 4 4M4 19h16" /></svg>
}

export function SalaVoce({ tela = null, cam = null, lugares = {}, canvasDe, onMover, camOn, sharing, onToggleCam, onToggleShare, onOcultar }: SalaVoceProps): ReactNode {
  const lugarDe = (s: Stream | null): Lugar | null => (s ? (lugares[s.trackId] ?? 'faixa') : null)
  const telaAqui = lugarDe(tela) === 'faixa' ? tela : null
  const camAqui = lugarDe(cam) === 'faixa' ? cam : null
  const escondidas = [tela, cam].filter((s): s is Stream => s !== null && lugarDe(s) === 'fechada')
  const naPrincipal = [tela, cam].some((s) => s !== null && lugarDe(s) === 'principal')
  const aqui = [telaAqui, camAqui].filter((s): s is Stream => s !== null)

  let aviso: string | null = null
  if (!tela && !cam) aviso = sharing ? 'Transmitindo só o som' : 'Você não está transmitindo'
  else if (aqui.length === 0) aviso = escondidas.length > 0 ? 'Prévia escondida' : naPrincipal ? 'Sua transmissão está na tela principal' : null

  // O que sobe para a principal: a tela (a câmera do canto vai junto) ou, sem tela, a câmera.
  const subir = telaAqui ?? camAqui
  const rotuloSubir = telaAqui ? 'Pôr minha tela na tela principal' : 'Pôr minha câmera na tela principal'
  const olhoAberto = aqui.length > 0
  const rotuloOlho = olhoAberto ? 'Esconder minha prévia' : 'Mostrar minha prévia'
  const alternarPrevia = () => {
    if (!onMover) return
    if (olhoAberto) for (const s of aqui) onMover(s.trackId, 'fechada')
    else for (const s of escondidas) onMover(s.trackId, 'faixa')
  }

  return (
    <div style={CARD}>
      <div style={{ position: 'relative', width: '100%', aspectRatio: '16 / 9', background: '#0A100E' }}>
        {telaAqui && canvasDe && (
          <>
            <CanvasSlot canvas={canvasDe(telaAqui.trackId)} label="Prévia: sua tela" />
            <span style={ROTULO}>sua tela</span>
          </>
        )}
        {camAqui && canvasDe && (
          <div style={{ position: 'absolute', inset: 0, ...(telaAqui ? CANTO : {}) }}>
            <CanvasSlot canvas={canvasDe(camAqui.trackId)} label={telaAqui ? 'Prévia: câmera' : 'Prévia: sua câmera'} />
            <span style={ROTULO}>{telaAqui ? 'câmera' : 'sua câmera'}</span>
          </div>
        )}
        {aviso && <span style={CENTRO}>{aviso}</span>}
        <span style={{ position: 'absolute', right: 6, top: 6, display: 'flex', gap: 6 }}>
          {subir && onMover && (
            <button type="button" className="zp-x" onClick={() => onMover(subir.trackId, 'principal')} aria-label={rotuloSubir} title={rotuloSubir} style={BTN}>
              <IconeCima />
            </button>
          )}
          {(olhoAberto || escondidas.length > 0) && onMover && (
            <button type="button" className="zp-x" onClick={alternarPrevia} aria-label={rotuloOlho} title={rotuloOlho} aria-pressed={olhoAberto} style={BTN}>
              <IconeOlho aberto={olhoAberto} />
            </button>
          )}
          {onOcultar && (
            <button type="button" className="zp-x" onClick={onOcultar} aria-label="Ocultar meu cartão" title="Ocultar meu cartão" style={BTN}>
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true"><path d="M6 6l12 12M18 6 6 18" /></svg>
            </button>
          )}
        </span>
      </div>
      <div style={{ padding: '10px 12px 0', fontWeight: 700 }}>Você</div>
      <div style={{ display: 'flex', gap: 8, padding: 6 }}>
        <button type="button" onClick={onToggleCam} aria-pressed={camOn} style={meBtn(camOn)}>
          {camOn ? (
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" aria-hidden="true" style={{ flex: '0 0 18px' }}><rect x="3" y="7" width="13" height="10" rx="2" /><path d="m16 11 5-3v8l-5-3z" /></svg>
          ) : (
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" style={{ flex: '0 0 18px' }}><rect x="3" y="7" width="13" height="10" rx="2" /><path d="m16 11 5-3v8l-5-3zM3 3l18 18" /></svg>
          )}
          Câmera
        </button>
        <button type="button" onClick={onToggleShare} aria-pressed={sharing} title={sharing ? 'Ajustar a transmissão' : 'Compartilhar'} style={meBtn(sharing)}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" style={{ flex: '0 0 18px' }}><rect x="3" y="4" width="18" height="12" rx="2" /><path d="M8 20h8M12 16v4" /></svg>
          Tela
        </button>
      </div>
    </div>
  )
}
