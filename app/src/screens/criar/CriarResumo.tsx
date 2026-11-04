// Tarefa ui-17
import type { ReactNode } from 'react'
import type { Preset } from '../../bridge/types'
import { Badge } from '../../components/Badge'
import { Button } from '../../components/Button'
import { ScenePreview } from '../../components/ScenePreview'

export interface CriarResumoProps { roomName: string; sourceLabel: string; sound: string; preset: Preset; encoder: string; busy: boolean; onStart: () => void }

// Preset técnico (ex. '1080p60') vira rótulo legível ('1080p · 60').
function rotuloPreset(preset: Preset): string {
  return preset.replace(/^(\d+p)(\d+)$/, '$1 · $2')
}

// Linha rótulo/valor do resumo, no mesmo padrão da prévia do desenho.
function Linha({ rotulo, children, mono }: { rotulo: string; children: ReactNode; mono?: boolean }): ReactNode {
  return (
    <>
      <dt style={{ color: 'var(--texto-3)' }}>{rotulo}</dt>
      <dd style={{ margin: 0, fontWeight: mono ? 400 : 600, fontFamily: mono ? 'var(--fonte-mono, monospace)' : undefined, fontSize: mono ? 12.5 : undefined, overflowWrap: 'anywhere' }}>{children}</dd>
    </>
  )
}

// Prévia final antes de iniciar: o que a galera vai ver e os ajustes escolhidos.
export function CriarResumo({ roomName, sourceLabel, sound, preset, encoder, busy, onStart }: CriarResumoProps): ReactNode {
  const semFonte = sourceLabel === ''
  return (
    <aside
      aria-label="Prévia da transmissão"
      className="zp-card"
      style={{ display: 'flex', flexDirection: 'column', gap: 16, padding: 20, flex: '1 1 320px', minWidth: 0 }}
    >
      <h2 style={{ margin: 0, fontSize: 14, fontWeight: 600 }}>É isso que a galera vai ver</h2>

      <div style={{ position: 'relative', borderRadius: 14, overflow: 'hidden', border: '1px solid var(--borda-forte, #2A443C)', aspectRatio: '16 / 9' }}>
        <ScenePreview scene="game" />
        <div style={{ position: 'absolute', left: 8, top: 8 }}>
          <Badge tone="red">AO VIVO</Badge>
        </div>
      </div>

      <dl style={{ margin: 0, display: 'grid', gridTemplateColumns: 'auto 1fr', gap: '10px 16px', fontSize: 13 }}>
        <Linha rotulo="Sala">{roomName.trim() === '' ? 'Sua sala' : roomName}</Linha>
        <Linha rotulo="Fonte">{sourceLabel}</Linha>
        <Linha rotulo="Som">{sound}</Linha>
        <Linha rotulo="Qualidade" mono>{rotuloPreset(preset)}</Linha>
        <Linha rotulo="Codificador" mono>{encoder === 'auto' ? 'Automático' : encoder}</Linha>
      </dl>

      <Button variant="primary" disabled={busy || semFonte} onClick={onStart}>
        {busy ? 'Iniciando…' : 'Iniciar transmissão'}
      </Button>
    </aside>
  )
}
