// Cartao "Voce" da faixa de tiles: previa ao vivo do que eu transmito (tela/janela e, no canto,
// a camera). O nucleo entrega a camada mais leve direto, sem rede (watch da propria trilha).
import { useEffect, useState, type CSSProperties, type ReactNode } from 'react'
import type { RoomView } from '../../bridge/types'
import { api } from '../../bridge/api'
import { VideoPlayer } from '../../video/player'
import { LOCAL_CAMERA_TILE, LOCAL_SCREEN_TILE, ownTracks } from './salaModel'

export interface SalaVoceProps {
  room: RoomView
  meId: string
  camOn: boolean
  sharing: boolean
  onToggleCam?: () => void
  onToggleShare?: () => void
}

// card(on) do script x-dc
const card = (on: boolean): CSSProperties => ({
  display: 'flex', flexDirection: 'column', minWidth: 0, borderRadius: 16, overflow: 'hidden', background: '#0F1916',
  border: '2px solid ' + (on ? '#3BF08A' : '#1F332D'), ...(on ? { boxShadow: '0 0 20px rgba(59,240,138,0.22)' } : {}),
})
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

interface PreviaProps { trilhaId: string; tile: number; rotulo: string; style?: CSSProperties }

// Um canvas por trilha: camada pequena (2); unwatch + close na limpeza.
function Previa({ trilhaId, tile, rotulo, style }: PreviaProps): ReactNode {
  const [canvas, setCanvas] = useState<HTMLCanvasElement | null>(null)
  useEffect(() => {
    if (!canvas) return
    const player = new VideoPlayer(canvas)
    void api.watch(trilhaId, tile, [320, 180], 2, (buf) => player.push(buf))?.catch?.(() => {})
    return () => {
      void api.unwatch(tile)?.catch?.(() => {})
      player.close()
    }
  }, [canvas, trilhaId, tile])
  return (
    <div style={{ position: 'absolute', inset: 0, background: '#0A100E', ...style }}>
      <canvas ref={setCanvas} aria-label={`Prévia: ${rotulo}`} style={{ position: 'absolute', inset: 0, width: '100%', height: '100%', objectFit: 'contain' }} />
      <span style={ROTULO}>{rotulo}</span>
    </div>
  )
}

export function SalaVoce({ room, meId, camOn, sharing, onToggleCam, onToggleShare }: SalaVoceProps): ReactNode {
  const { screen, camera } = ownTracks(room, meId)
  // "Se eu quiser": a prévia pode ser escondida (e para de gastar a codificação da camada leve).
  const [verPrevia, setVerPrevia] = useState(true)
  const tela = sharing ? screen : null
  const cam = camOn ? camera : null
  const temVideo = tela !== null || cam !== null
  const rotuloOlho = verPrevia ? 'Esconder minha prévia' : 'Mostrar minha prévia'

  let aviso: string | null = null
  if (!temVideo) aviso = sharing ? 'Transmitindo só o som' : 'Você não está transmitindo'
  else if (!verPrevia) aviso = 'Prévia escondida'

  return (
    <div style={card(false)}>
      <div style={{ position: 'relative', width: '100%', aspectRatio: '16 / 9', background: '#0A100E' }}>
        {verPrevia && tela && <Previa trilhaId={tela.id} tile={LOCAL_SCREEN_TILE} rotulo="sua tela" />}
        {verPrevia && cam && (
          <Previa
            trilhaId={cam.id}
            tile={LOCAL_CAMERA_TILE}
            rotulo={tela ? 'câmera' : 'sua câmera'}
            // Com a tela no ar, a câmera vai para o canto (como os outros veem).
            style={tela ? { inset: 'auto 6px 6px auto', width: '36%', aspectRatio: '16 / 9', borderRadius: 8, border: '1px solid #2A443C', overflow: 'hidden' } : undefined}
          />
        )}
        {aviso && <span style={CENTRO}>{aviso}</span>}
        {temVideo && (
          <button
            type="button"
            className="zp-x"
            onClick={() => setVerPrevia((v) => !v)}
            aria-label={rotuloOlho}
            title={rotuloOlho}
            aria-pressed={verPrevia}
            style={{ position: 'absolute', right: 6, top: 6, width: 32, height: 32, borderRadius: 9, border: '1px solid #2A443C', background: 'rgba(10,16,14,0.82)', color: '#E6F4EC', display: 'flex', alignItems: 'center', justifyContent: 'center' }}
          >
            {verPrevia ? (
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z" /><circle cx="12" cy="12" r="3" /></svg>
            ) : (
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M3 3l18 18M10.6 5.1A10.4 10.4 0 0 1 12 5c6.5 0 10 7 10 7a17 17 0 0 1-3.2 4.1M6.6 6.6A17 17 0 0 0 2 12s3.5 7 10 7a9.8 9.8 0 0 0 5.4-1.6" /></svg>
            )}
          </button>
        )}
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
