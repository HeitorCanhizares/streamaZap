// Tarefa ui-16
import type { ReactNode } from 'react'
import type { Preset, Sources } from '../../bridge/types'
import { Segmented } from '../../components/Segmented'
import { Select } from '../../components/Select'
import { Toggle } from '../../components/Toggle'

export type SomModo = 'none' | 'all' | 'apps'
export interface CriarOpcoesProps { sources: Sources; sound: SomModo; onSound: (v: SomModo) => void; apps: string[]; onApps: (v: string[]) => void; preset: Preset; onPreset: (p: Preset) => void; encoder: string; onEncoder: (e: string) => void }

// Opções de som, na ordem da tela.
const OPCOES_SOM: { value: SomModo; label: string }[] = [
  { value: 'none', label: 'Sem som' },
  { value: 'all', label: 'Todo o PC' },
  { value: 'apps', label: 'Só alguns apps' },
]

// Cartões de qualidade: rótulo de resolução/fps e a linha de apoio com a banda estimada.
const PRESETS: { value: Preset; label: string; apoio: string }[] = [
  { value: '480p30', label: '480p · 30', apoio: '~2 Mbps' },
  { value: '720p30', label: '720p · 30', apoio: '~4 Mbps' },
  { value: '720p60', label: '720p · 60', apoio: '~7 Mbps' },
  { value: '1080p60', label: '1080p · 60', apoio: '~12 Mbps' },
]

// Seção com título numerado, no mesmo padrão visual das outras seções da tela.
function Secao({ titulo, children }: { titulo: string; children: ReactNode }): ReactNode {
  return (
    <section className="zp-card" style={{ display: 'flex', flexDirection: 'column', gap: 12, padding: 20, flex: '1 1 320px', minWidth: 0 }}>
      <h2 style={{ margin: 0, fontSize: 14, fontWeight: 600 }}>{titulo}</h2>
      {children}
    </section>
  )
}

// Opções de transmissão: som, qualidade e codificador.
export function CriarOpcoes({ sources, sound, onSound, apps, onApps, preset, onPreset, encoder, onEncoder }: CriarOpcoesProps): ReactNode {
  // Liga/desliga um app sem mutar a lista recebida: sempre entrega um array novo.
  function alternarApp(id: string, ligado: boolean): void {
    if (ligado) {
      if (!apps.includes(id)) onApps([...apps, id])
    } else {
      onApps(apps.filter((a) => a !== id))
    }
  }

  const opcoesEncoder = [
    { value: 'auto', label: 'Automático' },
    ...sources.encoders.map((e) => ({ value: e, label: e })),
  ]

  return (
    <div style={{ display: 'flex', flexWrap: 'wrap', gap: 18 }}>
      <Secao titulo="Som">
        <Segmented options={OPCOES_SOM} value={sound} onChange={(v) => onSound(v as SomModo)} />
        {sound === 'apps' && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
            {sources.apps.map((app) => (
              <Toggle
                key={app.id}
                label={app.name}
                checked={apps.includes(app.id)}
                onChange={(ligado) => alternarApp(app.id, ligado)}
              />
            ))}
          </div>
        )}
        <span style={{ fontSize: 12, color: 'var(--texto-3)' }}>O som do próprio streamaZap nunca vai junto.</span>
      </Secao>

      <Secao titulo="Qualidade">
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, minmax(0, 1fr))', gap: 8 }}>
          {PRESETS.map((p) => {
            const ativo = p.value === preset
            return (
              <button
                key={p.value}
                type="button"
                className="zp-card"
                aria-label={p.label}
                aria-pressed={ativo}
                onClick={() => onPreset(p.value)}
                style={{
                  display: 'flex',
                  flexDirection: 'column',
                  gap: 4,
                  padding: '10px 8px',
                  alignItems: 'flex-start',
                  color: 'var(--texto)',
                  cursor: 'pointer',
                  border: ativo ? '2px solid var(--verde)' : '1px solid var(--borda)',
                }}
              >
                <span style={{ fontFamily: 'var(--font-mono)', fontWeight: 600, fontSize: 13 }}>{p.label}</span>
                <span style={{ fontSize: 11, color: 'var(--texto-2)' }}>{p.apoio}</span>
              </button>
            )
          })}
        </div>
      </Secao>

      <Secao titulo="Codificador">
        <label style={{ display: 'flex', alignItems: 'center', gap: 10, fontSize: 13, color: 'var(--texto-2)' }}>
          <span style={{ whiteSpace: 'nowrap' }}>Codificador</span>
          <span style={{ flex: 1, minWidth: 0 }}>
            <Select options={opcoesEncoder} value={encoder} onChange={onEncoder} />
          </span>
        </label>
      </Secao>
    </div>
  )
}
