// Seção Perfil da tela de configurações (porte fiel de docs/design/4-configuracoes.html).
import type { ReactNode } from 'react'
import type { Settings } from '../../bridge/types'

export interface ConfigSecaoProps { settings: Settings; onChange: (patch: Partial<Settings>) => void }

// Paleta de cores do nome no chat, com os nomes do design.
const CORES: Array<[string, string]> = [
  ['#3BF08A', 'Verde zap'],
  ['#5CD6FF', 'Ciano'],
  ['#FFB547', 'Âmbar'],
  ['#FF8FB1', 'Rosa'],
  ['#B79CFF', 'Lilás'],
  ['#FF9E5C', 'Laranja'],
]

export function ConfigPerfil({ settings, onChange }: ConfigSecaoProps): ReactNode {
  const { name, color } = settings
  // Nome vazio mostra '?' como inicial.
  const inicial = (name || '?').slice(0, 1).toUpperCase()
  return (
    <section
      aria-labelledby="h-perfil"
      style={{
        flex: '1 1 420px', minWidth: 0, display: 'flex', flexDirection: 'column', gap: 16,
        padding: 22, borderRadius: 18, background: '#0F1916', border: '1px solid #1F332D',
      }}
    >
      <h2 id="h-perfil" style={{ margin: 0, fontSize: 16, fontWeight: 700 }}>Perfil</h2>
      <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
        <span
          data-testid="perfil-avatar"
          style={{
            flex: '0 0 72px', width: 72, height: 72, borderRadius: '50%', display: 'flex',
            alignItems: 'center', justifyContent: 'center', fontWeight: 700, fontSize: 28,
            color, background: color + '24', boxShadow: `0 0 0 2px #0F1916, 0 0 0 4px ${color}`,
          }}
        >
          {inicial}
        </span>
        <div style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', gap: 6 }}>
          <label htmlFor="apelido" style={{ fontSize: 12.5, color: '#A3BDB1' }}>Seu nome na sala</label>
          <input
            id="apelido"
            value={name}
            onChange={(e) => onChange({ name: e.target.value })}
            style={{
              height: 44, padding: '0 14px', borderRadius: 12, border: '1px solid #2A443C',
              background: '#0A100E', color: '#E6F4EC', fontSize: 15, fontWeight: 600,
            }}
          />
        </div>
      </div>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
        <span id="cor-label" style={{ fontSize: 12.5, color: '#A3BDB1' }}>Cor do seu nome no chat</span>
        <div role="radiogroup" aria-labelledby="cor-label" style={{ display: 'flex', flexWrap: 'wrap', gap: 10 }}>
          {CORES.map(([hex, label]) => {
            const on = color === hex
            return (
              <button
                key={hex}
                type="button"
                role="radio"
                aria-checked={on}
                aria-label={label}
                title={label}
                onClick={() => onChange({ color: hex })}
                style={{
                  width: 44, height: 44, borderRadius: '50%', border: 0, background: hex,
                  boxShadow: '0 0 0 3px #0F1916' + (on ? `, 0 0 0 5px ${hex}` : ', 0 0 0 4px #1F332D'),
                }}
              />
            )
          })}
        </div>
      </div>
      <div
        className="zp-dots"
        style={{
          display: 'flex', flexDirection: 'column', gap: 6, padding: 14, borderRadius: 14,
          backgroundColor: '#0C1513', border: '1px solid #1F332D',
        }}
      >
        <span style={{ fontSize: 11, color: '#7E978B', fontFamily: "'JetBrains Mono', monospace", letterSpacing: '0.1em' }}>
          PRÉVIA
        </span>
        <div
          style={{
            alignSelf: 'flex-start', display: 'flex', flexDirection: 'column', gap: 2,
            padding: '7px 10px 6px 12px', borderRadius: '4px 16px 16px 16px', background: '#16241F',
          }}
        >
          <span data-testid="perfil-previa-nome" style={{ fontSize: 12.5, fontWeight: 700, color }}>{name}</span>
          <span>bora, já tô na sala</span>
          <span style={{ alignSelf: 'flex-end', fontFamily: "'JetBrains Mono', monospace", fontSize: 10.5, color: '#8BA498' }}>
            22:14
          </span>
        </div>
      </div>
    </section>
  )
}
