// Tarefa ui-23: seções 'Perfil' e 'Sobre e atualizações' da tela de configurações.
import type { ReactNode } from 'react'
import type { Settings } from '../../bridge/types'
import { Avatar } from '../../components/Avatar'
import { Button } from '../../components/Button'
import { Select } from '../../components/Select'
import { TextField } from '../../components/TextField'
import { Toggle } from '../../components/Toggle'

export interface ConfigSecaoProps { settings: Settings; onChange: (patch: Partial<Settings>) => void }

// Versão exibida na seção Sobre. Constante local até o app ler a versão do pacote.
const VERSAO = 'v3.0.0-alpha.0'

// Paleta de cores do nome no chat (mesma do design 4-configuracoes.html).
const CORES = ['#3BF08A', '#5CD6FF', '#FFB547', '#FF8FB1', '#B79CFF', '#FF9E5C']

const CANAIS = [
  { value: 'estavel', label: 'Estável' },
  { value: 'beta', label: 'Beta' },
]

// Frase de exemplo da prévia do chat, copiada do design.
const FRASE_PREVIA = 'bora, já tô na sala'

export function ConfigPerfil({ settings, onChange }: ConfigSecaoProps): ReactNode {
  return (
    <div style={{ display: 'flex', flexWrap: 'wrap', gap: 20, alignItems: 'stretch' }}>
      <section
        aria-labelledby="h-perfil"
        style={{
          flex: '1 1 420px', minWidth: 0, display: 'flex', flexDirection: 'column', gap: 16,
          padding: 22, borderRadius: 18, background: '#0F1916', border: '1px solid #1F332D',
        }}
      >
        <h2 id="h-perfil" style={{ margin: 0, fontSize: 16, fontWeight: 700 }}>Perfil</h2>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
          <span style={{ fontSize: 12.5, color: '#A3BDB1' }}>Nome</span>
          <TextField
            value={settings.name}
            placeholder="Seu nome"
            onChange={(name) => onChange({ name })}
          />
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
          <span id="cor-label" style={{ fontSize: 12.5, color: '#A3BDB1' }}>Cor do seu nome no chat</span>
          <div role="group" aria-labelledby="cor-label" style={{ display: 'flex', flexWrap: 'wrap', gap: 10 }}>
            {CORES.map((cor) => {
              const selecionada = settings.color.toUpperCase() === cor.toUpperCase()
              return (
                <button
                  key={cor}
                  type="button"
                  role="radio"
                  aria-checked={selecionada}
                  aria-label={cor}
                  title={cor}
                  onClick={() => onChange({ color: cor })}
                  style={{
                    width: 34, height: 34, borderRadius: '50%', padding: 0, cursor: 'pointer',
                    background: cor, border: 'none',
                    // Anel externo marca a cor escolhida, com um espaço de cor de fundo entre eles.
                    boxShadow: selecionada ? `0 0 0 2px #0F1916, 0 0 0 4px ${cor}` : 'none',
                  }}
                />
              )
            })}
          </div>
        </div>

        <div
          style={{
            display: 'flex', flexDirection: 'column', gap: 6, padding: 14, borderRadius: 14,
            background: '#0C1513', border: '1px solid #1F332D',
          }}
        >
          <span style={{ fontSize: 11, color: '#7E978B', fontFamily: "'JetBrains Mono', monospace", letterSpacing: '0.1em' }}>
            PRÉVIA
          </span>
          <div
            style={{
              alignSelf: 'flex-start', display: 'flex', alignItems: 'flex-start', gap: 10,
              padding: '8px 12px', borderRadius: '4px 16px 16px 16px', background: '#16241F',
            }}
          >
            <Avatar name={settings.name} color={settings.color} size={28} />
            <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
              <span style={{ fontWeight: 700, color: settings.color, fontSize: 13 }}>{settings.name}</span>
              <span style={{ fontSize: 14 }}>{FRASE_PREVIA}</span>
            </div>
          </div>
        </div>
      </section>

      <section
        aria-labelledby="h-sobre"
        style={{
          flex: '1 1 380px', minWidth: 0, display: 'flex', flexDirection: 'column', alignItems: 'center',
          gap: 14, padding: '26px 22px 22px', borderRadius: 18, background: '#0F1916',
          border: '1px solid #1F332D', textAlign: 'center',
        }}
      >
        <h2 id="h-sobre" style={{ margin: 0, fontSize: 16, fontWeight: 700 }}>Sobre e atualizações</h2>
        <img
          src="/icone.png"
          alt="Logo do streamaZap"
          style={{
            width: 136, height: 136, borderRadius: 34, display: 'block',
            boxShadow: '0 0 0 1px #2A443C, 0 0 48px rgba(59,240,138,0.4)',
          }}
        />
        <span style={{ fontFamily: "'Unbounded', sans-serif", fontWeight: 700, fontSize: 22 }}>
          {`streamaZap ${VERSAO}`}
        </span>

        <div style={{ width: '100%', display: 'flex', flexDirection: 'column', gap: 12, marginTop: 4 }}>
          <Toggle
            checked={settings.update.auto}
            label="Atualizar automaticamente"
            onChange={(auto) => onChange({ update: { ...settings.update, auto } })}
          />
          <div style={{ display: 'flex', flexDirection: 'column', gap: 6, textAlign: 'left' }}>
            <span style={{ fontSize: 12.5, color: '#A3BDB1' }}>Canal de atualização</span>
            <Select
              options={CANAIS}
              value={settings.update.channel}
              onChange={(v) => onChange({ update: { ...settings.update, channel: v as 'estavel' | 'beta' } })}
            />
          </div>
          <Button
            variant="ghost"
            onClick={() => {
              // M5: a busca real nas Releases do GitHub entra na tarefa de atualização; por ora sem ação.
            }}
          >
            Procurar atualização
          </Button>
        </div>
      </section>
    </div>
  )
}
