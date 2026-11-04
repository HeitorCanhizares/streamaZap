// Tarefa ui-03
import type { ReactNode } from 'react'

export interface SegmentedProps { options: { value: string; label: string; disabled?: boolean }[]; value: string; onChange: (v: string) => void }

// Controle segmentado: uma opção ativa por vez. Opção desabilitada não reage ao clique.
export function Segmented({ options, value, onChange }: SegmentedProps): ReactNode {
  return (
    <div className="zp-segmented">
      {options.map((opcao) => {
        const ativo = opcao.value === value
        return (
          <button
            key={opcao.value}
            type="button"
            className={'zp-seg-item' + (ativo ? ' zp-seg-active' : '')}
            disabled={opcao.disabled}
            onClick={() => {
              // Guarda extra: o disabled nativo já bloqueia, mas garante que nada escape.
              if (!opcao.disabled) onChange(opcao.value)
            }}
          >
            {opcao.label}
          </button>
        )
      })}
    </div>
  )
}
