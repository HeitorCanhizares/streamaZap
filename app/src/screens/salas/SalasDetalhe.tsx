// Tarefa ui-13: detalhe da sala selecionada (coluna direita da tela Salas).
// Porte de docs/design/1-salas.html (área <main class="zp-dots">), sem a parte de
// compartilhamento e senha, que chegam em tarefas próprias.
import type { CSSProperties, ReactNode } from 'react'
import type { RoomListing, PeerListing } from '../../bridge/types'
import { Avatar } from '../../components/Avatar'
import { Badge } from '../../components/Badge'
import { Button } from '../../components/Button'
import { ScenePreview } from '../../components/ScenePreview'

export interface SalasDetalheProps { room: RoomListing | null; peers: PeerListing[]; onEnter: (room: RoomListing) => void; onEnterManual: () => void }

// Cor estável por nome: o mesmo nome sempre ganha o mesmo tom, sem guardar nada.
// Tom claro para a inicial em verde-escuro (--verde-texto) continuar legível.
function corDoNome(nome: string): string {
  let hash = 0
  for (const ch of nome) hash = (hash * 31 + (ch.codePointAt(0) ?? 0)) >>> 0
  return `hsl(${hash % 360}, 60%, 68%)`
}

const titulo: CSSProperties = {
  margin: 0,
  fontFamily: 'var(--font-mono)',
  fontWeight: 400,
  fontSize: 11,
  letterSpacing: '0.14em',
  textTransform: 'uppercase',
  color: 'var(--texto-3)',
}

export function SalasDetalhe({ room, peers, onEnter, onEnterManual }: SalasDetalheProps): ReactNode {
  if (room === null) {
    return (
      <main
        className="zp-dots"
        style={{ flex: '999 1 560px', minWidth: 0, display: 'flex', alignItems: 'center', justifyContent: 'center', padding: 32 }}
      >
        <p style={{ margin: 0, color: 'var(--texto-2)', fontSize: 15 }}>Escolha uma sala na lista</p>
      </main>
    )
  }

  return (
    <main
      className="zp-dots"
      style={{ flex: '999 1 560px', minWidth: 0, display: 'flex', flexDirection: 'column', gap: 22, padding: '28px 32px 32px' }}
    >
      <div style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 16 }}>
        <Avatar name={room.name} color="var(--verde)" size={56} />
        <div style={{ display: 'flex', flexDirection: 'column', gap: 4, minWidth: 0 }}>
          <span style={titulo}>Sala selecionada</span>
          <h2
            style={{
              margin: 0,
              fontFamily: 'var(--font-titulo)',
              fontWeight: 700,
              fontSize: 28,
              letterSpacing: '-0.015em',
            }}
          >
            {room.name}
          </h2>
          <span style={{ color: 'var(--texto-2)' }}>
            por <b style={{ color: 'var(--texto)', fontWeight: 600 }}>{room.host}</b>
          </span>
        </div>
        <span style={{ flex: 1 }} />
        <Badge tone={room.has_password ? 'yellow' : 'green'}>{room.has_password ? 'Senha' : 'Aberta'}</Badge>
      </div>

      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 22, alignItems: 'flex-start' }}>
        <div style={{ flex: '1 1 480px', minWidth: 0, display: 'flex', flexDirection: 'column', gap: 10 }}>
          <div
            style={{
              position: 'relative',
              width: '100%',
              aspectRatio: '16 / 9',
              borderRadius: 18,
              overflow: 'hidden',
              border: '1px solid var(--borda-forte)',
              background: 'var(--bg-barra)',
            }}
          >
            <ScenePreview scene={room.scene ?? 'game'} />
            {room.live_title ? (
              <span
                style={{
                  position: 'absolute',
                  left: 12,
                  top: 12,
                  padding: '4px 10px',
                  borderRadius: 8,
                  background: 'rgba(10,16,14,0.78)',
                  color: 'var(--texto)',
                  fontSize: 12,
                  fontWeight: 600,
                }}
              >
                {room.live_title}
              </span>
            ) : null}
          </div>
          <span style={{ fontSize: 12.5, color: 'var(--texto-3)' }}>Prévia em miniatura · sem áudio</span>
        </div>

        <div
          style={{
            flex: '1 1 260px',
            maxWidth: 340,
            display: 'flex',
            flexDirection: 'column',
            gap: 14,
            padding: 22,
            borderRadius: 18,
            background: 'var(--superficie)',
            border: '1px solid var(--borda)',
          }}
        >
          <Button variant="primary" onClick={() => onEnter(room)}>
            Entrar na sala
          </Button>
          <Button variant="ghost" onClick={onEnterManual}>
            Entrar com código/IP
          </Button>
        </div>
      </div>

      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 22, alignItems: 'flex-start' }}>
      <section style={{ flex: '1 1 200px', minWidth: 0, display: 'flex', flexDirection: 'column', gap: 12 }} aria-label="Na sala agora">
        <h3 style={titulo}>Na sala agora · {room.participants}</h3>
      </section>
      <section style={{ flex: '1 1 280px', minWidth: 0, display: 'flex', flexDirection: 'column', gap: 12 }} aria-label="Pessoas na rede">
        <h3 style={titulo}>Pessoas na rede</h3>
        {peers.length === 0 ? (
          <p style={{ margin: 0, color: 'var(--texto-2)' }}>Ninguém mais por aqui</p>
        ) : (
          <ul style={{ listStyle: 'none', margin: 0, padding: 0, display: 'flex', flexDirection: 'column', gap: 8 }}>
            {peers.map((peer) => (
              <li key={`${peer.name}-${peer.ip}`} style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                <Avatar name={peer.name} color={corDoNome(peer.name)} size={32} />
                <span style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column' }}>
                  <span style={{ fontWeight: 600 }}>{peer.name}</span>
                  <span style={{ fontFamily: 'var(--font-mono)', fontSize: 12, color: 'var(--texto-3)' }}>{peer.ip}</span>
                </span>
                {peer.receives_you ? (
                  <Badge tone="green">recebe você</Badge>
                ) : (
                  <Badge tone="yellow">não recebe</Badge>
                )}
              </li>
            ))}
          </ul>
        )}
      </section>
      </div>
    </main>
  )
}
