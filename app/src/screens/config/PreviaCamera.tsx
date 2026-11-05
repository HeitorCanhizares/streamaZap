// Prévia ao vivo da câmera nas configurações. Só liga no botão (a câmera acende a luz e fica
// ocupada enquanto estiver aberta) e desliga ao sair da tela. Com a câmera já ligada na sala, o
// núcleo mostra a da sala em vez de abrir o dispositivo de novo.
import { useEffect, useState, type CSSProperties, type ReactNode } from 'react'
import { api } from '../../bridge/api'
import { VideoPlayer } from '../../video/player'

export interface PreviaCameraProps {
  /** Id da câmera na lista de fontes (null = a padrão do Windows). */
  cameraId: string | null
  /** Espelhar a imagem (só aqui, como a pessoa se vê). */
  espelhar: boolean
}

// Tiles da prévia da câmera: acima dos da sala e do "Você" (9000/9001), abaixo da prévia da lista.
// Cada abertura usa um tile novo: o fechar da anterior não derruba o canal da seguinte.
let proximoTile = 9100
function novoTile(): number {
  const t = proximoTile
  proximoTile = proximoTile >= 9199 ? 9100 : proximoTile + 1
  return t
}

const BOTAO: CSSProperties = {
  height: 38, padding: '0 16px', borderRadius: 999, border: '1px solid #3BF08A', background: 'rgba(10,16,14,0.82)',
  color: '#3BF08A', fontWeight: 600, fontSize: 13,
}

export function PreviaCamera({ cameraId, espelhar }: PreviaCameraProps): ReactNode {
  const [ligada, setLigada] = useState(false)
  const [erro, setErro] = useState<string | null>(null)
  const [canvas, setCanvas] = useState<HTMLCanvasElement | null>(null)

  useEffect(() => {
    if (!ligada || !canvas) return
    let player: VideoPlayer
    try {
      player = new VideoPlayer(canvas)
    } catch {
      setErro('Este computador não consegue mostrar o vídeo da prévia.')
      setLigada(false)
      return
    }
    let vivo = true
    const tile = novoTile()
    void Promise.resolve(api.cameraPreview(cameraId, tile, (b) => player.push(b))).catch((e: unknown) => {
      if (!vivo) return
      setErro(`Não deu para abrir a câmera: ${e instanceof Error ? e.message : String(e)}`)
      setLigada(false)
    })
    return () => {
      vivo = false
      void Promise.resolve(api.cameraPreviewStop(tile)).catch(() => {})
      player.close()
    }
  }, [ligada, canvas, cameraId])

  return (
    <>
      {ligada && (
        <canvas
          ref={setCanvas}
          data-testid="camera-preview"
          aria-label="Prévia ao vivo da sua câmera"
          style={{ position: 'absolute', inset: 0, width: '100%', height: '100%', objectFit: 'cover', transform: espelhar ? 'scaleX(-1)' : 'none' }}
        />
      )}
      {!ligada && (
        <div style={{ position: 'absolute', inset: 0, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: 10, padding: 12, textAlign: 'center' }}>
          {erro && <span role="status" style={{ fontSize: 12, color: '#FF8592' }}>{erro}</span>}
          <button type="button" className="zp-ghost" onClick={() => { setErro(null); setLigada(true) }} style={BOTAO}>
            Testar câmera
          </button>
        </div>
      )}
      {ligada && (
        <button type="button" className="zp-ghost" onClick={() => setLigada(false)} style={{ ...BOTAO, position: 'absolute', right: 8, bottom: 8, height: 32, padding: '0 12px', fontSize: 12 }}>
          Parar
        </button>
      )}
    </>
  )
}
