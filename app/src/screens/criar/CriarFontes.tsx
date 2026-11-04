// Tarefa criar-04
import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react'
import type { SourceItem, Sources, SceneKind } from '../../bridge/types'
import { isTauri } from '../../bridge/api'
import { ScenePreview } from '../../components/ScenePreview'
import { cenaDaFonte, fonteMeta, type FonteTab } from './criarModel'

export type { FonteTab } from './criarModel'
export interface CriarFontesProps { sources: Sources; tab: FonteTab; onTab: (t: FonteTab) => void; selected: string | null; onSelect: (id: string) => void; cameraCorner: boolean; onCameraCorner: (v: boolean) => void; onRefresh?: () => void }

// Miniatura fixa do Rust: RGBA 320x180 (ver source_thumbnail no lado Tauri).
const THUMB_W = 320
const THUMB_H = 180

const LISTA_POR_TAB: Record<FonteTab, (s: Sources) => SourceItem[]> = {
  telas: (s) => s.screens,
  janelas: (s) => s.windows,
  webcam: (s) => s.cameras,
}

const ABAS: { value: FonteTab; label: string }[] = [
  { value: 'telas', label: 'Telas' },
  { value: 'janelas', label: 'Janelas' },
  { value: 'webcam', label: 'Webcam' },
]

const MONO = "'JetBrains Mono', monospace"

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

function estiloAba(ativo: boolean): CSSProperties {
  return {
    flex: 1, height: 38, padding: '0 12px', borderRadius: 9, border: 'none', fontSize: 13, fontWeight: 600, cursor: 'pointer',
    background: ativo ? '#1A3A2C' : 'transparent',
    color: ativo ? 'var(--verde)' : 'var(--texto-2)',
  }
}

// Grade de fontes da aba escolhida. Cada cartão é um botão com aria-pressed.
export function CriarFontes({ sources, tab, onTab, selected, onSelect, cameraCorner, onCameraCorner, onRefresh }: CriarFontesProps): ReactNode {
  const itens = LISTA_POR_TAB[tab](sources)

  return (
    <section style={{ display: 'flex', flexDirection: 'column', gap: 14, padding: 20, borderRadius: 18, background: 'var(--superficie)', border: '1px solid var(--borda)' }}>
      <div style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 12 }}>
        <h2 style={{ margin: 0, display: 'flex', alignItems: 'baseline', gap: 10, fontSize: 14, fontWeight: 600 }}>
          <span style={{ fontFamily: MONO, fontSize: 11, color: 'var(--verde)' }}>04</span>O que compartilhar
        </h2>
        <span style={{ flex: 1 }} />
        <div role="tablist" aria-label="Tipo de fonte" style={{ display: 'flex', gap: 4, padding: 4, borderRadius: 12, background: 'var(--bg)', border: '1px solid var(--borda)' }}>
          {ABAS.map((a) => (
            <button key={a.value} type="button" role="tab" aria-selected={tab === a.value} onClick={() => onTab(a.value)} style={estiloAba(tab === a.value)}>
              {a.label}
            </button>
          ))}
        </div>
        <button
          type="button"
          className="zp-btn-ghost"
          aria-label="Atualizar lista de telas e janelas"
          title="Atualizar lista"
          onClick={() => onRefresh?.()}
          style={{ width: 44, height: 44, borderRadius: 12, border: '1px solid var(--borda)', background: 'transparent', color: 'var(--texto-2)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}
        >
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <path d="M20 11a8 8 0 0 0-14.6-4.5M4 13a8 8 0 0 0 14.6 4.5" />
            <path d="M5 3v4h4M19 21v-4h-4" />
          </svg>
        </button>
      </div>

      {itens.length === 0 ? (
        <p style={{ margin: 0, color: 'var(--texto-3)', fontSize: 14 }}>Nada encontrado</p>
      ) : (
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, minmax(0, 1fr))', gap: 12 }}>
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
                  borderRadius: 14,
                  textAlign: 'left',
                  background: ativo ? 'rgba(59,240,138,0.08)' : 'var(--bg)',
                  border: `1px solid ${ativo ? 'var(--verde)' : 'var(--borda)'}`,
                  boxShadow: ativo ? '0 0 18px rgba(59,240,138,0.18)' : undefined,
                }}
              >
                <span style={{ display: 'block', position: 'relative', borderRadius: 10, overflow: 'hidden' }}>
                  <Miniatura id={item.id} cena={cenaDaFonte(tab, i)} />
                  {ativo && (
                    <span data-testid="fonte-check" style={{ position: 'absolute', right: 8, top: 8, width: 24, height: 24, borderRadius: '50%', background: '#3BF08A', color: '#03140A', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                        <path d="m5 12 5 5 9-10" />
                      </svg>
                    </span>
                  )}
                </span>
                <span style={{ display: 'flex', flexDirection: 'column', gap: 2, padding: '2px 2px 0', textAlign: 'left' }}>
                  <span style={{ fontWeight: 600, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{item.name}</span>
                  <span style={{ fontFamily: MONO, fontSize: 11, color: 'var(--texto-3)' }}>{fonteMeta(item)}</span>
                </span>
              </button>
            )
          })}
        </div>
      )}

      {tab !== 'webcam' && (
        <button
          type="button"
          role="switch"
          aria-checked={cameraCorner}
          onClick={() => onCameraCorner(!cameraCorner)}
          style={{ display: 'flex', alignItems: 'center', gap: 14, padding: '12px 14px', borderRadius: 14, border: '1px solid var(--borda)', background: 'var(--bg)', textAlign: 'left' }}
        >
          <span style={{ display: 'flex', alignItems: 'center', width: 44, height: 26, borderRadius: 999, padding: 3, boxSizing: 'border-box', background: cameraCorner ? 'var(--verde)' : 'var(--borda-forte)', justifyContent: cameraCorner ? 'flex-end' : 'flex-start' }}>
            <span style={{ width: 20, height: 20, borderRadius: '50%', background: cameraCorner ? 'var(--verde-texto)' : 'var(--texto-2)' }} />
          </span>
          <span style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: 2 }}>
            <span style={{ fontWeight: 600 }}>Mostrar minha webcam no canto</span>
            <span style={{ fontSize: 12, color: 'var(--texto-2)' }}>Sua câmera aparece por cima da tela, no canto inferior direito.</span>
          </span>
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="var(--texto-2)" strokeWidth="1.8" strokeLinejoin="round" aria-hidden="true">
            <rect x="3" y="7" width="13" height="10" rx="2" />
            <path d="m16 11 5-3v8l-5-3z" />
          </svg>
        </button>
      )}
    </section>
  )
}
