// Tarefa ui-12: coluna esquerda da tela Salas (lista, busca e filtros).
// Porte de docs/design/1-salas.html (section aria-label="Salas na rede").
import type { ReactNode } from 'react'
import type { RoomListing } from '../../bridge/types'
import { Avatar } from '../../components/Avatar'
import { Badge } from '../../components/Badge'
import { Segmented } from '../../components/Segmented'
import { TextField } from '../../components/TextField'

export type SalasFiltro = 'todas' | 'abertas' | 'senha'
export interface SalasListaProps {
  rooms: RoomListing[]
  selectedId: string | null
  onSelect: (id: string) => void
  query: string
  onQuery: (q: string) => void
  filter: SalasFiltro
  onFilter: (f: SalasFiltro) => void
}

// Remove acentos e põe em minúscula, para a busca não depender de acento nem de caixa.
function normaliza(texto: string): string {
  return texto.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase()
}

// Aplica o filtro de segmento (Todas / Abertas / Com senha) e a busca por nome.
function filtrar(rooms: RoomListing[], query: string, filter: SalasFiltro): RoomListing[] {
  const termo = normaliza(query.trim())
  return rooms.filter((r) => {
    if (filter === 'abertas' && r.has_password) return false
    if (filter === 'senha' && !r.has_password) return false
    if (termo && !normaliza(r.name).includes(termo)) return false
    return true
  })
}

const OPCOES_FILTRO = [
  { value: 'todas', label: 'Todas' },
  { value: 'abertas', label: 'Abertas' },
  { value: 'senha', label: 'Com senha' },
]

export function SalasLista({
  rooms,
  selectedId,
  onSelect,
  query,
  onQuery,
  filter,
  onFilter,
}: SalasListaProps): ReactNode {
  const visiveis = filtrar(rooms, query, filter)

  return (
    <section
      aria-label="Salas na rede"
      style={{
        flex: '1 1 320px',
        maxWidth: 392,
        minWidth: 0,
        display: 'flex',
        flexDirection: 'column',
        background: 'var(--superficie)',
        borderRight: '1px solid var(--borda)',
      }}
    >
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 10,
          padding: '22px 20px 14px',
        }}
      >
        <h1
          style={{
            margin: 0,
            fontFamily: 'var(--font-titulo)',
            fontWeight: 700,
            fontSize: 22,
            letterSpacing: '-0.01em',
          }}
        >
          Salas
        </h1>
        <span
          style={{
            fontFamily: 'var(--font-mono)',
            fontSize: 11,
            color: 'var(--verde)',
            padding: '3px 8px',
            borderRadius: 999,
            background: 'rgba(59,240,138,0.12)',
          }}
        >
          {rooms.length} salas
        </span>
      </div>

      <div style={{ margin: '0 20px 12px' }}>
        <Segmented
          options={[
            { value: 'online', label: 'Online', disabled: true },
            { value: 'local', label: 'Local' },
          ]}
          value="local"
          onChange={() => {}}
        />
        <p
          style={{
            margin: '4px 0 0',
            fontSize: 11,
            color: 'var(--texto)',
            opacity: 0.6,
          }}
        >
          Online: em breve
        </p>
      </div>

      <div style={{ padding: '0 20px 12px' }}>
        <TextField value={query} onChange={onQuery} placeholder="Buscar sala ou pessoa" />
      </div>

      <div role="group" aria-label="Filtrar salas" style={{ padding: '0 20px 14px' }}>
        <Segmented
          options={OPCOES_FILTRO}
          value={filter}
          onChange={(v) => onFilter(v as SalasFiltro)}
        />
      </div>

      <div
        style={{
          flex: 1,
          display: 'flex',
          flexDirection: 'column',
          borderTop: '1px solid var(--borda)',
        }}
      >
        {visiveis.map((room) => {
          const selecionada = room.room_id === selectedId
          return (
            <button
              key={room.room_id}
              type="button"
              className="zp-row"
              aria-pressed={selecionada}
              onClick={() => onSelect(room.room_id)}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 12,
                padding: '12px 20px',
                width: '100%',
                textAlign: 'left',
                background: selecionada ? 'var(--hover)' : 'transparent',
                border: 'none',
                borderLeft: `3px solid ${selecionada ? 'var(--verde)' : 'transparent'}`,
                color: 'inherit',
                cursor: 'pointer',
              }}
            >
              <Avatar name={room.name} color="var(--verde)" size={40} />
              <span
                style={{
                  flex: 1,
                  minWidth: 0,
                  display: 'flex',
                  flexDirection: 'column',
                  gap: 4,
                }}
              >
                <span style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                  <span
                    style={{
                      fontWeight: 600,
                      fontSize: 15,
                      whiteSpace: 'nowrap',
                      overflow: 'hidden',
                      textOverflow: 'ellipsis',
                    }}
                  >
                    {room.name}
                  </span>
                  {room.live_title ? <Badge tone="red">AO VIVO</Badge> : null}
                </span>
                <span
                  style={{
                    fontSize: 13,
                    color: 'var(--texto)',
                    opacity: 0.7,
                    whiteSpace: 'nowrap',
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                  }}
                >
                  {room.host} · {room.participants} pessoas
                </span>
              </span>
              <Badge tone={room.has_password ? 'yellow' : 'green'}>
                {room.has_password ? 'Senha' : 'Aberta'}
              </Badge>
            </button>
          )
        })}
        {visiveis.length === 0 ? (
          <p
            style={{
              margin: 0,
              padding: '28px 20px',
              color: 'var(--texto)',
              opacity: 0.7,
              textAlign: 'center',
            }}
          >
            Nenhuma sala encontrada
          </p>
        ) : null}
      </div>
    </section>
  )
}
