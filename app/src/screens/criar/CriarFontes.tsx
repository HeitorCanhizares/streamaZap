// Tarefa ui-15
import { useEffect, useRef, useState, type ReactNode } from 'react'
import type { SourceItem, Sources, SceneKind } from '../../bridge/types'
import { isTauri } from '../../bridge/api'
import { ScenePreview } from '../../components/ScenePreview'
import { Segmented } from '../../components/Segmented'
import { Toggle } from '../../components/Toggle'

export type FonteTab = 'telas' | 'janelas' | 'webcam'
export interface CriarFontesProps { sources: Sources; tab: FonteTab; onTab: (t: FonteTab) => void; selected: string | null; onSelect: (id: string) => void; cameraCorner: boolean; onCameraCorner: (v: boolean) => void }

// Miniatura fixa do Rust: RGBA 320x180 (ver source_thumbnail no lado Tauri).
const THUMB_W = 320
const THUMB_H = 180

// Cenas de fallback, alternadas pelo índice do cartão (só aparecem fora do Tauri ou em erro).
const CENAS: SceneKind[] = ['game', 'race', 'movie', 'code']

const LISTA_POR_TAB: Record<FonteTab, (s: Sources) => SourceItem[]> = {
  telas: (s) => s.screens,
  janelas: (s) => s.windows,
  webcam: (s) => s.cameras,
}

// Miniatura de uma fonte. No Tauri pede o frame real ao backend; em qualquer falha
// (ou fora do Tauri) cai na cena SVG de exemplo, para o cartão nunca ficar vazio.
function Miniatura({ id, cena }: { id: string; cena: SceneKind }): ReactNode {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const [ok, setOk] = useState(false)
  const tauri = isTauri()

  useEffect(() => {
    if (!tauri) return
    let vivo = true
    ;(async () => {
      try {
        // Import dinamico: so carrega o modulo do Tauri dentro do app.
        const { invoke } = await import('@tauri-apps/api/core')
        const rgba = await invoke<number[]>('source_thumbnail', { source: id, w: THUMB_W, h: THUMB_H })
        const canvas = canvasRef.current
        const ctx = canvas?.getContext('2d')
        if (!vivo || !canvas || !ctx) return
        ctx.putImageData(new ImageData(new Uint8ClampedArray(rgba), THUMB_W, THUMB_H), 0, 0)
        setOk(true)
      } catch {
        // Sem miniatura real: mantém a cena de exemplo.
        if (vivo) setOk(false)
      }
    })()
    return () => {
      vivo = false
    }
  }, [id, tauri])

  return (
    <span style={{ display: 'block', position: 'relative', width: '100%', aspectRatio: '16 / 9', borderRadius: 10, overflow: 'hidden', background: 'var(--bg)' }}>
      {tauri && (
        <canvas
          ref={canvasRef}
          width={THUMB_W}
          height={THUMB_H}
          aria-hidden="true"
          style={{ display: ok ? 'block' : 'none', width: '100%', height: '100%' }}
        />
      )}
      {!ok && (
        <span style={{ position: 'absolute', inset: 0 }}>
          <ScenePreview scene={cena} />
        </span>
      )}
    </span>
  )
}

// Grade de fontes da aba escolhida. Cada cartão é um botão com aria-pressed.
export function CriarFontes({ sources, tab, onTab, selected, onSelect, cameraCorner, onCameraCorner }: CriarFontesProps): ReactNode {
  const itens = LISTA_POR_TAB[tab](sources)

  return (
    <section style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
      <Segmented
        options={[
          { value: 'telas', label: 'Telas' },
          { value: 'janelas', label: 'Janelas' },
          { value: 'webcam', label: 'Webcam' },
        ]}
        value={tab}
        onChange={(v) => onTab(v as FonteTab)}
      />

      {itens.length === 0 ? (
        <p style={{ margin: 0, color: 'var(--texto-3)', fontSize: 14 }}>Nada encontrado</p>
      ) : (
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(200px, 1fr))', gap: 12 }}>
          {itens.map((item, i) => {
            const ativo = item.id === selected
            return (
              <button
                key={item.id}
                type="button"
                className="zp-card"
                aria-pressed={ativo}
                onClick={() => onSelect(item.id)}
                style={{
                  display: 'flex',
                  flexDirection: 'column',
                  gap: 8,
                  padding: 8,
                  textAlign: 'left',
                  border: ativo ? '2px solid var(--verde)' : '1px solid var(--borda)',
                }}
              >
                <Miniatura id={item.id} cena={CENAS[i % CENAS.length]} />
                <span style={{ fontWeight: 600, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{item.name}</span>
              </button>
            )
          })}
        </div>
      )}

      {tab !== 'webcam' && (
        <Toggle checked={cameraCorner} onChange={onCameraCorner} label="Mostrar minha webcam no canto" />
      )}
    </section>
  )
}
