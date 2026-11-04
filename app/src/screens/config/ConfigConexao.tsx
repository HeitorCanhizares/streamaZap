// Tarefa ui-24: seção 'Conexão' da tela de configurações (porte de docs/design/4-configuracoes.html).
import type { ReactNode } from 'react'
import type { NetworkStatus, RoomMode } from '../../bridge/types'
import { Badge } from '../../components/Badge'
import { Segmented } from '../../components/Segmented'
import { Select } from '../../components/Select'
import { TextField } from '../../components/TextField'
import { Toggle } from '../../components/Toggle'
import type { ConfigSecaoProps } from './ConfigPerfil'

// Modo Online ainda não existe no app: a opção aparece desabilitada, como no design.
const MODOS = [
  { value: 'online', label: 'Online · em breve', disabled: true },
  { value: 'local', label: 'Local' },
]

// Porta válida é um número inteiro entre 1 e 65535. Qualquer outra entrada devolve null.
function portaValida(texto: string): number | null {
  if (!/^\d+$/.test(texto)) return null
  const n = Number(texto)
  return n >= 1 && n <= 65535 ? n : null
}

export function ConfigConexao({ settings, onChange, network }: ConfigSecaoProps & { network: NetworkStatus }): ReactNode {
  const local = settings.local

  return (
    <section
      aria-labelledby="h-conexao"
      style={{
        display: 'flex', flexDirection: 'column', gap: 16, padding: 22, borderRadius: 18,
        background: '#0F1916', border: '1px solid #1F332D',
      }}
    >
      <div style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 14 }}>
        <h2 id="h-conexao" style={{ margin: 0, fontSize: 16, fontWeight: 700 }}>Conexão</h2>
        <span style={{ flex: 1 }} />
        <span id="modo-label" style={{ fontSize: 12.5, color: '#A3BDB1' }}>Abrir o app em</span>
        <div role="radiogroup" aria-labelledby="modo-label">
          <Segmented
            options={MODOS}
            value={settings.open_in}
            onChange={(v) => onChange({ open_in: v as RoomMode })}
          />
        </div>
      </div>

      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 16 }}>
        {/* Bloco Online: sem campos por enquanto, só o aviso. */}
        <div
          style={{
            flex: '1 1 380px', minWidth: 0, display: 'flex', flexDirection: 'column', gap: 12, padding: 18,
            borderRadius: 16, background: '#0A100E', border: '1px solid #1F332D',
          }}
        >
          <span style={{ display: 'flex', alignItems: 'center', gap: 10, fontWeight: 700 }}>Online · servidor</span>
          <span style={{ fontSize: 12.5, color: '#7E978B' }}>Disponível em breve</span>
        </div>

        {/* Bloco Local: rede, porta e anúncio da sala na rede de casa ou Radmin. */}
        <div
          style={{
            flex: '1 1 380px', minWidth: 0, display: 'flex', flexDirection: 'column', gap: 12, padding: 18,
            borderRadius: 16, background: '#0A100E', border: '1px solid #1F332D',
          }}
        >
          <span style={{ display: 'flex', alignItems: 'center', gap: 10, fontWeight: 700 }}>
            Local · Radmin e rede de casa
          </span>

          <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
            <div style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 8 }}>
              <span style={{ fontSize: 12.5, color: '#A3BDB1' }}>Rede usada para as salas</span>
              {network.radmin_ip ? <Badge tone="blue">{`Radmin ${network.radmin_ip}`}</Badge> : null}
            </div>
            <Select
              options={[
                { value: '', label: 'Todas as redes' },
                ...network.interfaces.map((nome) => ({ value: nome, label: nome })),
              ]}
              value={local.interface ?? ''}
              onChange={(v) => onChange({ local: { ...local, interface: v === '' ? null : v } })}
            />
          </div>

          <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
            <span style={{ fontSize: 12.5, color: '#A3BDB1' }}>Porta da transmissão</span>
            <div style={{ width: 140 }}>
              <TextField
                value={local.port > 0 ? String(local.port) : ''}
                placeholder="47800"
                onChange={(texto) => {
                  // Entrada inválida é ignorada; o campo continua com o valor anterior.
                  const port = portaValida(texto)
                  if (port !== null) onChange({ local: { ...local, port } })
                }}
              />
            </div>
            <span style={{ fontSize: 12, color: '#7E978B' }}>
              Se estiver ocupada, o app usa a próxima livre. Descoberta de salas na 47801.
            </span>
          </div>

          <Toggle
            checked={local.announce}
            label="Anunciar minha sala na rede"
            onChange={(announce) => onChange({ local: { ...local, announce } })}
          />
        </div>
      </div>
    </section>
  )
}
