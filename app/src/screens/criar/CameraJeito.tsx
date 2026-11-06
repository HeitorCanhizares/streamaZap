// "Câmera: desligada · no canto · solta" + os 4 cantos quando for no canto (plano B4). Tarefa front-06.
import type { ReactNode } from 'react'
import type { Corner } from '../../bridge/types'
import type { CameraJeito as Jeito } from './cameraModel'
import { CameraCantos } from './CameraCantos'

export interface CameraJeitoProps {
  jeito: Jeito
  onJeito: (j: Jeito) => void
  canto: Corner
  onCanto: (c: Corner) => void
  disabled?: boolean
}

const OPCOES: { value: Jeito; label: string }[] = [
  { value: 'off', label: 'Desligada' },
  { value: 'canto', label: 'No canto' },
  { value: 'solta', label: 'Solta' },
]

export function CameraJeito({ jeito, onJeito, canto, onCanto, disabled = false }: CameraJeitoProps): ReactNode {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
      <span style={{ fontSize: 13, color: 'var(--texto-2)' }}>Câmera</span>
      <div role="radiogroup" aria-label="Jeito da câmera" style={{ display: 'grid', gridTemplateColumns: 'repeat(3, minmax(0, 1fr))', gap: 8 }}>
        {OPCOES.map((o) => {
          const ativo = jeito === o.value
          return (
            <button
              key={o.value}
              type="button"
              role="radio"
              aria-checked={ativo}
              disabled={disabled}
              onClick={() => onJeito(o.value)}
              style={{
                padding: '10px 4px', borderRadius: 12, fontSize: 13, fontWeight: 600,
                cursor: disabled ? 'default' : 'pointer', opacity: disabled ? 0.5 : 1,
                border: `1px solid ${ativo ? 'var(--verde)' : 'var(--borda)'}`,
                background: ativo ? 'rgba(59,240,138,0.1)' : 'var(--bg)',
                color: ativo ? 'var(--verde)' : 'var(--texto)',
              }}
            >
              {o.label}
            </button>
          )
        })}
      </div>
      {jeito === 'canto' ? (
        <>
          <CameraCantos canto={canto} onCanto={onCanto} disabled={disabled} />
          <span style={{ fontSize: 12, color: 'var(--texto-2)' }}>A câmera fica nesse canto da sua tela.</span>
        </>
      ) : null}
      {jeito === 'solta' ? (
        <span style={{ fontSize: 12, color: 'var(--texto-2)' }}>A câmera vai como uma transmissão à parte, ao lado da tela.</span>
      ) : null}
    </div>
  )
}
