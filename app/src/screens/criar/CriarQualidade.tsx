import { useEffect, type ReactNode } from 'react'
import type { Preset, PresetLock, SourceItem } from '../../bridge/types'
import { PRESETS_UI } from './criarModel'
import { CameraQualidade } from './CameraQualidade'
import { presetAjustado } from './cameraModel'
import { travaDe, seloPlano } from '../../bridge/planoModel'
import { SeloPlano } from '../../components/SeloPlano'
import { useCliqueTravado } from '../planos/useCliqueTravado'

export interface CriarQualidadeProps { encoders: string[]; preset: Preset; onPreset: (p: Preset) => void; encoder: string; onEncoder: (e: string) => void; adaptavel: boolean; onAdaptavel: (v: boolean) => void; numero?: string; cameraPreset?: Preset; onCameraPreset?: (p: Preset) => void; cameraInfo?: SourceItem; travasTela?: PresetLock[]; travasCamera?: PresetLock[] }

// Seção 06 "Qualidade" (2-criar-sala.html 167-191).

const MONO = "'JetBrains Mono', monospace"

export function CriarQualidade({ encoders, preset, onPreset, encoder, onEncoder, adaptavel, onAdaptavel, numero = '06', cameraPreset, onCameraPreset, cameraInfo, travasTela, travasCamera }: CriarQualidadeProps): ReactNode {
  const cliqueTravado = useCliqueTravado()
  const bloco = { display: 'flex', flexDirection: 'column' as const, gap: 12, padding: 20, borderRadius: 18, background: 'var(--superficie)', border: '1px solid var(--borda)' }

  // Opção "auto" é o padrão: o servidor escolhe o codificador disponível.
  const opcoes = [{ value: 'auto', label: 'Automático' }, ...encoders.map((e) => ({ value: e, label: e }))]

  // Dois seletores (Tela e Câmera) só quando quem chama passa as props da câmera.
  const doisSeletores = onCameraPreset !== undefined && cameraPreset !== undefined
  const subtitulo = { fontSize: 12.5, color: 'var(--texto-2)' }

  // A escolha nunca fica numa qualidade que a câmera não faz.
  useEffect(() => {
    if (!onCameraPreset || cameraPreset === undefined || !cameraInfo) return
    const ajustado = presetAjustado(cameraPreset, cameraInfo)
    if (ajustado !== cameraPreset) onCameraPreset(ajustado)
  }, [cameraInfo, cameraPreset, onCameraPreset])

  return (
    <section style={{ ...bloco, flex: '1 1 320px', minWidth: 0 }}>
      <h2 style={{ margin: 0, display: 'flex', alignItems: 'baseline', gap: 10, fontSize: 14, fontWeight: 600 }}>
        <span style={{ fontFamily: MONO, fontSize: 11, color: 'var(--verde)' }}>{numero}</span>
        Qualidade
      </h2>
      {doisSeletores && <div style={subtitulo}>Tela</div>}
      <div role="radiogroup" aria-label={doisSeletores ? 'Qualidade da tela' : 'Qualidade'} style={{ display: 'grid', gridTemplateColumns: 'repeat(4, minmax(0, 1fr))', gap: 8 }}>
        {PRESETS_UI.map((p) => {
          const ativo = preset === p.value
          const lock = travaDe(p.value, travasTela ?? [])
          return (
            <button
              key={p.value}
              type="button"
              role="radio"
              aria-checked={ativo}
              aria-disabled={lock ? 'true' : undefined}
              data-travado={lock ? 'true' : undefined}
              title={lock ? `Precisa do plano ${seloPlano(lock.needs ?? 'fullhd')}` : undefined}
              onClick={() => { if (lock) cliqueTravado.aoClicar(lock); else onPreset(p.value) }}
              style={{
                display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 2,
                padding: '10px 4px', borderRadius: 12, cursor: 'pointer', opacity: lock ? 0.6 : 1,
                border: `1px solid ${ativo ? 'var(--verde)' : 'var(--borda)'}`,
                background: ativo ? 'rgba(59,240,138,0.1)' : 'var(--bg)',
                color: ativo ? 'var(--verde)' : 'var(--texto)',
              }}
            >
              <span style={{ fontFamily: MONO, fontSize: 13, fontWeight: 600 }}>{p.res}</span>
              <span style={{ fontSize: 11, color: 'var(--texto-2)' }}>{p.fps}</span>
              {lock && <SeloPlano plano={lock.needs ?? 'fullhd'} cadeado />}
            </button>
          )
        })}
      </div>
      {cliqueTravado.explicacao !== null && (
        <p role="status" style={{ margin: 0, fontSize: 11.5, color: 'var(--texto-2)' }}>{cliqueTravado.explicacao}</p>
      )}
      {doisSeletores && (
        <>
          <div style={subtitulo}>Câmera</div>
          <CameraQualidade preset={cameraPreset} onPreset={onCameraPreset} camera={cameraInfo} travas={travasCamera} />
        </>
      )}
      <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
        <label htmlFor="encoder" style={{ fontSize: 13, color: 'var(--texto-2)', whiteSpace: 'nowrap' }}>Codificador</label>
        {/* Select compartilhado não aceita id/aria-label, então usa o <select> nativo. */}
        <select
          id="encoder"
          value={encoder}
          onChange={(e) => onEncoder(e.target.value)}
          style={{
            flex: 1, minWidth: 0, height: 40, padding: '0 10px', borderRadius: 10,
            border: '1px solid var(--borda-forte)', background: 'var(--bg)', color: 'var(--texto)',
          }}
        >
          {opcoes.map((o) => (
            <option key={o.value} value={o.value}>{o.label}</option>
          ))}
        </select>
      </div>
      <button
        type="button"
        role="switch"
        aria-checked={adaptavel}
        onClick={() => onAdaptavel(!adaptavel)}
        style={{ display: 'flex', alignItems: 'center', gap: 12, padding: 0, border: 0, background: 'transparent', textAlign: 'left', cursor: 'pointer' }}
      >
        <span style={{
          flex: '0 0 44px', width: 44, height: 26, borderRadius: 999, padding: 3,
          display: 'flex', justifyContent: adaptavel ? 'flex-end' : 'flex-start',
          background: adaptavel ? 'var(--verde)' : 'var(--borda-forte)',
        }}>
          <span style={{
            width: 20, height: 20, borderRadius: '50%',
            background: adaptavel ? 'var(--verde-texto)' : 'var(--texto-2)',
          }} />
        </span>
        <span style={{ fontSize: 13, color: 'var(--texto-2)', lineHeight: 1.4 }}>
          <b style={{ color: 'var(--texto)', fontWeight: 600 }}>Qualidade adaptável</b> — baixa sozinha se alguém estiver com a conexão lenta.
        </span>
      </button>
    </section>
  )
}
