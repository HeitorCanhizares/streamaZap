// Seletor de qualidade da câmera (4 presets; os que a câmera não faz ficam desligados). Plano B2.
// Usado por CriarQualidade, SalaTransmissao e ConfigVideo. Tarefa front-07.
import type { ReactNode } from 'react'
import type { Preset, PresetLock, SourceItem } from '../../bridge/types'
import { PRESETS_UI } from './criarModel'
import { opcoesDaCamera } from './cameraModel'
import { travaDe, seloPlano } from '../../bridge/planoModel'
import { SeloPlano } from '../../components/SeloPlano'
import { useCliqueTravado } from '../planos/useCliqueTravado'

export interface CameraQualidadeProps {
  preset: Preset
  onPreset: (p: Preset) => void
  /** A câmera escolhida (com `max_height`/`max_fps`); sem ela nada fica desligado. */
  camera?: SourceItem
  /** Rótulo acessível do grupo (padrão "Qualidade da câmera"). */
  rotulo?: string
  /** Presets acima do plano (cadeado); ausente = nada travado. */
  travas?: PresetLock[]
}

const MONO = "'JetBrains Mono', monospace"

export function CameraQualidade({ preset, onPreset, camera, rotulo, travas }: CameraQualidadeProps): ReactNode {
  const cliqueTravado = useCliqueTravado()
  const opcoes = opcoesDaCamera(camera)
  const algumDesligado = opcoes.some((o) => o.desligado)

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
      <div role="radiogroup" aria-label={rotulo ?? 'Qualidade da câmera'} style={{ display: 'grid', gridTemplateColumns: 'repeat(4, minmax(0, 1fr))', gap: 8 }}>
        {PRESETS_UI.map((p) => {
          const ativo = preset === p.value
          const opcao = opcoes.find((o) => o.preset === p.value)
          const desligado = opcao?.desligado ?? false
          // A limitação da câmera ganha da trava do plano.
          const lock = desligado ? undefined : travaDe(p.value, travas ?? [])
          return (
            <button
              key={p.value}
              type="button"
              role="radio"
              aria-checked={ativo}
              disabled={desligado}
              aria-disabled={desligado || lock ? 'true' : undefined}
              data-travado={lock ? 'true' : undefined}
              title={desligado ? opcao?.motivo : lock ? `Precisa do plano ${seloPlano(lock.needs ?? 'fullhd')}` : undefined}
              onClick={() => { if (desligado) return; if (lock) cliqueTravado.aoClicar(lock); else onPreset(p.value) }}
              style={{
                display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 2,
                padding: '10px 4px', borderRadius: 12,
                cursor: desligado ? 'not-allowed' : 'pointer',
                opacity: desligado ? 0.4 : lock ? 0.6 : 1,
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
      {algumDesligado && (
        <p style={{ margin: 0, fontSize: 11.5, color: 'var(--texto-2)' }}>
          Algumas qualidades estão desligadas porque a câmera não faz.
        </p>
      )}
    </div>
  )
}
