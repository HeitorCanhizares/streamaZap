// Tarefa salas-07: cabeçalho da sala selecionada com selos (design 1-salas.html linhas 153-167).
import type { CSSProperties, ReactNode } from 'react'
import { estiloAvatar } from './model'
import type { SalaVM } from './model'

export interface SalaCabecalhoProps { sala: SalaVM }

const mono = "'JetBrains Mono', monospace"

/** Base dos selos: pílula de 30px centralizada, como no design. */
const selo: CSSProperties = {
  height: 30,
  padding: '0 12px',
  borderRadius: 999,
  display: 'flex',
  alignItems: 'center',
}

export function SalaCabecalho({ sala }: SalaCabecalhoProps): ReactNode {
  const online = sala.modo === 'online'
  const corRede = online ? '#3BF08A' : '#5CD6FF'
  const fundoRede = online ? 'rgba(59,240,138,0.12)' : 'rgba(92,214,255,0.12)'

  return (
    <div style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 16 }}>
      <span style={estiloAvatar(sala.host, 56, true)}>{sala.host.slice(0, 1).toUpperCase()}</span>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 4, minWidth: 0 }}>
        <span style={{ fontFamily: mono, fontSize: 11, letterSpacing: '0.14em', textTransform: 'uppercase', color: '#7E978B' }}>
          Sala selecionada
        </span>
        <h2 style={{ margin: 0, fontFamily: "'Unbounded', sans-serif", fontWeight: 700, fontSize: 28, letterSpacing: '-0.015em' }}>
          {sala.nome}
        </h2>
        <span style={{ color: '#A3BDB1' }}>
          por <b style={{ color: '#E6F4EC', fontWeight: 600 }}>{sala.host}</b>
          {sala.addr ? (
            <>
              {' · '}
              <span style={{ fontFamily: mono, fontSize: 12.5 }}>{sala.addr}</span>
            </>
          ) : null}
        </span>
      </div>
      <span style={{ flex: 1 }}></span>
      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
        <span
          style={{
            ...selo,
            fontFamily: mono,
            fontSize: 11.5,
            fontWeight: 600,
            letterSpacing: '0.08em',
            color: corRede,
            background: fundoRede,
          }}
        >
          {online ? 'ONLINE' : 'LOCAL'}
        </span>
        <span
          style={{
            ...selo,
            background: 'rgba(255,93,110,0.14)',
            color: '#FF8592',
            fontWeight: 700,
            fontSize: 12,
            letterSpacing: '0.08em',
            gap: 7,
          }}
        >
          <span className="zp-blink" style={{ width: 7, height: 7, borderRadius: '50%', background: '#FF5D6E' }}></span>
          AO VIVO
        </span>
        <span style={{ ...selo, border: '1px solid #2A443C', fontFamily: mono, fontSize: 12, color: '#A3BDB1' }}>
          {sala.qualidade}
        </span>
        <span style={{ ...selo, border: '1px solid #2A443C', fontFamily: mono, fontSize: 12, color: '#A3BDB1' }}>
          {sala.assistindo} assistindo
        </span>
      </div>
    </div>
  )
}
