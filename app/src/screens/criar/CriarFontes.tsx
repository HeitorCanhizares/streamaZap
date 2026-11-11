// Tarefa criar-04
import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react'
import type { Corner, SourceItem, Sources, SceneKind } from '../../bridge/types'
import { api, isTauri } from '../../bridge/api'
import { ScenePreview } from '../../components/ScenePreview'
import type { CameraJeito } from './cameraModel'
import { CameraJeito as CameraJeitoSeletor } from './CameraJeito'
import { cenaDaFonte, fonteMeta, listaDaAba, type FonteTab } from './criarModel'

export type { FonteTab } from './criarModel'
// cameraId/onCameraId: câmera escolhida para o canto (id da lista de fontes); sem eles, sem seletor.
export interface CriarFontesProps { sources: Sources; tab: FonteTab; onTab: (t: FonteTab) => void; selected: string | null; onSelect: (id: string) => void; cameraCorner: boolean; onCameraCorner: (v: boolean) => void; onRefresh?: () => void; cameraId?: string | null; onCameraId?: (id: string) => void; numero?: string; mostrarCanto?: boolean; cameraJeito?: CameraJeito; onCameraJeito?: (j: CameraJeito) => void; cameraCanto?: Corner; onCameraCanto?: (c: Corner) => void; mostrarNada?: boolean; bloqueado?: ReactNode }

// Miniatura pedida ao Rust: RGBA 320x180 (foto real da fonte; ver source_thumbnail no lado Tauri).
const THUMB_W = 320
const THUMB_H = 180
// A fonte selecionada (tela ou janela) é fotografada de novo a cada 2 s: dá para ver o que vai ao ar.
const ATUALIZAR_MS = 2000

// Fila única: uma foto por vez (cada foto abre uma captura no Windows).
let fila: Promise<unknown> = Promise.resolve()
function naFila<T>(tarefa: () => Promise<T>): Promise<T> {
  const p = fila.then(tarefa, tarefa)
  fila = p.catch(() => {})
  return p
}

const ABAS: { value: FonteTab; label: string }[] = [
  { value: 'telas', label: 'Telas' },
  { value: 'janelas', label: 'Janelas' },
  { value: 'webcam', label: 'Webcam' },
  { value: 'audio', label: 'Só som' },
]

const MONO = "'JetBrains Mono', monospace"

// Miniatura de uma fonte. No Tauri pede a foto real ao backend (e, com `atualizar`, de novo a
// cada 2 s); sem nenhuma foto (falha ou fora do Tauri) mostra a cena SVG de exemplo, para o
// cartão nunca ficar vazio. Uma falha depois de uma foto boa mantém a última foto.
function Miniatura({ id, cena, atualizar }: { id: string; cena: SceneKind; atualizar: boolean }): ReactNode {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const [ok, setOk] = useState(false)
  const tauri = isTauri()

  useEffect(() => {
    if (!tauri) return
    let vivo = true
    let timer: ReturnType<typeof setTimeout> | undefined
    const pedir = async (): Promise<void> => {
      try {
        const foto = await naFila(() => api.sourceThumbnail(id, THUMB_W, THUMB_H))
        const canvas = canvasRef.current
        const ctx = canvas?.getContext('2d')
        if (vivo && foto && canvas && ctx) {
          canvas.width = foto.width
          canvas.height = foto.height
          ctx.putImageData(new ImageData(new Uint8ClampedArray(foto.rgba), foto.width, foto.height), 0, 0)
          setOk(true)
        }
      } catch {
        // Sem foto: fica a anterior (ou a cena de exemplo).
      }
      if (vivo && atualizar) timer = setTimeout(() => void pedir(), ATUALIZAR_MS)
    }
    void pedir()
    return () => {
      vivo = false
      if (timer !== undefined) clearTimeout(timer)
    }
  }, [id, tauri, atualizar])

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
    flex: '1 0 auto', height: 38, padding: '0 12px', borderRadius: 9, border: 'none', fontSize: 13, fontWeight: 600, cursor: 'pointer', whiteSpace: 'nowrap',
    background: ativo ? '#1A3A2C' : 'transparent',
    color: ativo ? 'var(--verde)' : 'var(--texto-2)',
  }
}

// Grade de fontes da aba escolhida. Cada cartão é um botão com aria-pressed.
export function CriarFontes({ sources, tab, onTab, selected, onSelect, cameraCorner, onCameraCorner, onRefresh, cameraId, onCameraId, numero = '04', mostrarCanto = true, cameraJeito, onCameraJeito, cameraCanto, onCameraCanto, mostrarNada = false, bloqueado }: CriarFontesProps): ReactNode {
  const itens: SourceItem[] = listaDaAba(sources, tab)
  const abas = mostrarNada ? [...ABAS, { value: 'nada' as FonteTab, label: 'Nada por enquanto' }] : ABAS
  const novoJeito = cameraJeito !== undefined && onCameraJeito !== undefined
  const emTelaOuJanela = bloqueado === undefined && mostrarCanto && (tab === 'telas' || tab === 'janelas')
  // Câmera do canto: a escolhida, ou a primeira da lista.
  const cameraAtual = cameraId && sources.cameras.some((c) => c.id === cameraId) ? cameraId : (sources.cameras[0]?.id ?? '')

  return (
    <section style={{ display: 'flex', flexDirection: 'column', gap: 14, padding: 20, borderRadius: 18, background: 'var(--superficie)', border: '1px solid var(--borda)' }}>
      <div style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 12 }}>
        <h2 style={{ margin: 0, display: 'flex', alignItems: 'baseline', gap: 10, fontSize: 14, fontWeight: 600 }}>
          <span style={{ fontFamily: MONO, fontSize: 11, color: 'var(--verde)' }}>{numero}</span>O que compartilhar
        </h2>
        <span style={{ flex: 1 }} />
        {bloqueado === undefined && (<>
        <div role="tablist" aria-label="Tipo de fonte" style={{ display: 'flex', gap: 4, padding: 4, borderRadius: 12, background: 'var(--bg)', border: '1px solid var(--borda)' }}>
          {abas.map((a) => (
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
        </>)}
      </div>

      {bloqueado !== undefined ? (
        bloqueado
      ) : tab === 'nada' ? (
        <div role="note" style={{ padding: '16px 18px', borderRadius: 14, border: '1px solid var(--borda)', background: 'var(--bg)' }}>
          <span style={{ fontSize: 14, color: 'var(--texto-2)', lineHeight: 1.4 }}>Você cria a sala agora e começa a transmitir quando quiser, pelo botão Compartilhar dentro da sala.</span>
        </div>
      ) : tab === 'audio' ? (
        <div role="note" style={{ display: 'flex', alignItems: 'center', gap: 14, padding: '16px 18px', borderRadius: 14, border: '1px solid var(--borda)', background: 'var(--bg)' }}>
          <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="var(--verde)" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" style={{ flex: '0 0 28px' }}>
            <path d="M4 9v6h4l5 4V5L8 9z" />
            <path d="M16.5 8.5a5 5 0 0 1 0 7M19 6a8.5 8.5 0 0 1 0 12" />
          </svg>
          <span style={{ display: 'flex', flexDirection: 'column', gap: 2, minWidth: 0 }}>
            <span style={{ fontWeight: 600 }}>Só o som vai para a sala</span>
            <span style={{ fontSize: 13, color: 'var(--texto-2)', lineHeight: 1.4 }}>Ninguém vê sua tela nem sua câmera. Escolha em "Som" o que vai junto: todo o PC ou só alguns apps.</span>
          </span>
        </div>
      ) : itens.length === 0 ? (
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
                  {/* A câmera não é refotografada sozinha: cada foto liga a câmera de novo. */}
                  <Miniatura id={item.id} cena={cenaDaFonte(tab, i)} atualizar={ativo && tab !== 'webcam'} />
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

      {emTelaOuJanela && novoJeito && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
          <CameraJeitoSeletor jeito={cameraJeito} onJeito={onCameraJeito} canto={cameraCanto ?? 'inf_dir'} onCanto={onCameraCanto ?? (() => {})} />
        </div>
      )}
      {emTelaOuJanela && !novoJeito && (
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
      {emTelaOuJanela && (novoJeito ? cameraJeito !== 'off' : cameraCorner) && onCameraId && sources.cameras.length > 0 && (
        <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '0 4px' }}>
          <label htmlFor="camera-canto" style={{ fontSize: 13, color: 'var(--texto-2)', whiteSpace: 'nowrap' }}>Câmera</label>
          <select
            id="camera-canto"
            value={cameraAtual}
            onChange={(e) => onCameraId(e.target.value)}
            style={{ flex: 1, minWidth: 0, height: 40, padding: '0 10px', borderRadius: 10, border: '1px solid var(--borda-forte)', background: 'var(--bg)', color: 'var(--texto)' }}
          >
            {sources.cameras.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
          </select>
        </div>
      )}
    </section>
  )
}
