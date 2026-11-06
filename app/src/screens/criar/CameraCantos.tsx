// Seletor visual dos 4 cantos: miniatura de uma tela com 4 alvos (plano B4). Tarefa front-06.
import type { CSSProperties, ReactNode } from 'react'
import type { Corner } from '../../bridge/types'
import { CANTOS } from './cameraModel'

export interface CameraCantosProps { canto: Corner; onCanto: (c: Corner) => void; disabled?: boolean }

const INSET = 8

// Posição de cada alvo dentro da miniatura.
const POSICAO: Record<Corner, CSSProperties> = {
  sup_esq: { top: INSET, left: INSET },
  sup_dir: { top: INSET, right: INSET },
  inf_esq: { bottom: INSET, left: INSET },
  inf_dir: { bottom: INSET, right: INSET },
}

function IconeCamera(): ReactNode {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--verde-texto)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M3 8a2 2 0 0 1 2-2h2l1.5-2h7L17 6h2a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
      <circle cx="12" cy="12.5" r="3.5" />
    </svg>
  )
}

export function CameraCantos({ canto, onCanto, disabled = false }: CameraCantosProps): ReactNode {
  return (
    <div
      role="radiogroup"
      aria-label="Canto da câmera"
      style={{
        position: 'relative', width: 168, aspectRatio: '16 / 9', borderRadius: 10,
        background: '#0A100E', border: '1px solid var(--borda-forte)',
        opacity: disabled ? 0.5 : 1,
      }}
    >
      {CANTOS.map((c) => {
        const ativo = c.id === canto
        return (
          <button
            key={c.id}
            type="button"
            role="radio"
            aria-checked={ativo}
            aria-label={c.label}
            disabled={disabled}
            onClick={() => onCanto(c.id)}
            style={{
              position: 'absolute', ...POSICAO[c.id],
              width: 24, height: 24, borderRadius: 7, padding: 0,
              display: 'flex', alignItems: 'center', justifyContent: 'center',
              cursor: disabled ? 'default' : 'pointer',
              background: ativo ? 'var(--verde)' : 'rgba(59,240,138,0.12)',
              border: ativo ? '1px solid var(--verde)' : '1px dashed var(--borda-forte)',
            }}
          >
            {ativo ? <IconeCamera /> : null}
          </button>
        )
      })}
    </div>
  )
}
