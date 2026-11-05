// Tarefa config-04: seção 'Vídeo e câmera' (porte fiel de docs/design/4-configuracoes.html 177-216).
import type { CSSProperties, ReactNode } from 'react'
import type { Preset, Sources } from '../../bridge/types'
import { PreviaCamera } from './PreviaCamera'
import type { ConfigSecaoProps } from './ConfigPerfil'

// Cards do design, na ordem.
const PRESETS: { res: string; fps: string; value: Preset }[] = [
  { res: '720p', fps: '30 fps · Radmin', value: '720p30' },
  { res: '720p', fps: '60 fps', value: '720p60' },
  { res: '1080p', fps: '30 fps', value: '1080p30' },
  { res: '1080p', fps: '60 fps', value: '1080p60' },
]

const mono = "'JetBrains Mono', monospace"

function trilho(on: boolean): CSSProperties {
  return {
    flex: '0 0 44px', width: 44, height: 26, borderRadius: 999, padding: 3, display: 'flex',
    justifyContent: on ? 'flex-end' : 'flex-start', background: on ? '#3BF08A' : '#2A443C',
  }
}
function botao(on: boolean): CSSProperties {
  return { width: 20, height: 20, borderRadius: '50%', background: on ? '#03140A' : '#A3BDB1' }
}

export function ConfigVideo({ settings, onChange, sources }: ConfigSecaoProps & { sources: Sources }): ReactNode {
  const video = settings.video
  const selecionado = PRESETS.map((p) => p.value).lastIndexOf(video.default_preset)
  const auto = video.smoothing_auto

  return (
    <section
      aria-labelledby="h-video"
      style={{
        flex: '1 1 460px', minWidth: 0, display: 'flex', flexDirection: 'column', gap: 16,
        padding: 22, borderRadius: 18, background: '#0F1916', border: '1px solid #1F332D',
      }}
    >
      <h2 id="h-video" style={{ margin: 0, fontSize: 16, fontWeight: 700 }}>Vídeo e câmera</h2>
      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 16 }}>
        <div style={{ flex: '1 1 200px', minWidth: 0, display: 'flex', flexDirection: 'column', gap: 10 }}>
          <label htmlFor="camsel" style={{ fontSize: 12.5, color: '#A3BDB1' }}>Câmera</label>
          <select
            id="camsel"
            value={video.camera_id ?? ''}
            onChange={(e) => onChange({ video: { ...video, camera_id: e.target.value === '' ? null : e.target.value } })}
            style={{ height: 44, padding: '0 10px', borderRadius: 12, border: '1px solid #2A443C', background: '#0A100E', color: '#E6F4EC' }}
          >
            <option value="">Padrão do Windows</option>
            {sources.cameras.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
          </select>
          <button
            type="button"
            role="switch"
            aria-checked={video.mirror_preview}
            onClick={() => onChange({ video: { ...video, mirror_preview: !video.mirror_preview } })}
            style={{ display: 'flex', alignItems: 'center', gap: 12, padding: 0, border: 0, background: 'transparent', textAlign: 'left' }}
          >
            <span style={trilho(video.mirror_preview)}><span style={botao(video.mirror_preview)} /></span>
            <span style={{ fontSize: 13, color: '#A3BDB1' }}>Espelhar só a minha prévia</span>
          </button>
        </div>
        <div style={{ flex: '1 1 200px', minWidth: 0, position: 'relative', aspectRatio: '16 / 9', borderRadius: 14, overflow: 'hidden', border: '1px solid #2A443C', background: '#0A100E' }}>
          <PreviaCamera cameraId={video.camera_id} espelhar={video.mirror_preview} />
          <span
            style={{
              position: 'absolute', left: 8, top: 8, padding: '2px 8px', borderRadius: 6,
              background: 'rgba(10,16,14,0.82)', fontFamily: mono, fontSize: 10.5, letterSpacing: '0.08em', pointerEvents: 'none',
            }}
          >
            SUA CÂMERA
          </span>
        </div>
      </div>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
        <span id="q-label" style={{ fontSize: 12.5, color: '#A3BDB1' }}>Qualidade padrão das suas salas</span>
        <div role="radiogroup" aria-labelledby="q-label" style={{ display: 'grid', gridTemplateColumns: 'repeat(4, minmax(0, 1fr))', gap: 8 }}>
          {PRESETS.map((p, i) => {
            const on = i === selecionado
            return (
              <button
                key={i}
                type="button"
                role="radio"
                aria-checked={on}
                onClick={() => onChange({ video: { ...video, default_preset: p.value } })}
                style={{
                  display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 2, padding: '10px 4px',
                  borderRadius: 12, border: `1px solid ${on ? '#3BF08A' : '#1F332D'}`,
                  background: on ? 'rgba(59,240,138,0.1)' : '#0A100E', color: on ? '#3BF08A' : '#E6F4EC',
                }}
              >
                <span style={{ fontFamily: mono, fontWeight: 600, fontSize: 13 }}>{p.res}</span>
                <span style={{ fontSize: 11, color: '#A3BDB1' }}>{p.fps}</span>
              </button>
            )
          })}
        </div>
      </div>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 10, padding: 14, borderRadius: 14, background: '#0A100E', border: '1px solid #1F332D' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <span style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: 2 }}>
            <span style={{ fontWeight: 600 }}>Suavização</span>
            <span style={{ fontSize: 12, color: '#A3BDB1' }}>Segura um pouquinho o vídeo pra não engasgar.</span>
          </span>
          <button
            type="button"
            role="switch"
            aria-checked={auto}
            aria-label="Suavização automática"
            onClick={() => onChange({ video: { ...video, smoothing_auto: !auto } })}
            style={{ display: 'flex', alignItems: 'center', gap: 8, padding: 0, border: 0, background: 'transparent', color: '#A3BDB1', fontSize: 13 }}
          >
            <span style={trilho(auto)}><span style={botao(auto)} /></span>
            Auto
          </button>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <label htmlFor="smooth" style={{ position: 'absolute', width: 1, height: 1, overflow: 'hidden', clip: 'rect(0 0 0 0)' }}>
            Suavização fixa em milissegundos
          </label>
          <input
            id="smooth"
            className="zp-range"
            type="range"
            min={0}
            max={1000}
            step={50}
            value={video.smoothing_ms}
            disabled={auto}
            onChange={(e) => onChange({ video: { ...video, smoothing_ms: Number(e.target.value) } })}
            style={{ flex: 1 }}
          />
          <span style={{ flex: '0 0 72px', textAlign: 'right', fontFamily: mono, fontSize: 13, color: '#E6F4EC' }}>
            {auto ? 'auto' : `${video.smoothing_ms} ms`}
          </span>
        </div>
      </div>
    </section>
  )
}
