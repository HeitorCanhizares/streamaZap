// Tarefa salas-05: coluna da lista completa (design 1-salas.html linhas 63-140).
import type { CSSProperties, ReactNode } from 'react'
import type { RoomMode, ServerStatus } from '../../bridge/types'
import { IconeChave, IconeLupa, IconeMais } from './icons'
import { ModoAbas } from './ModoAbas'
import { SalaItem } from './SalaItem'
import type { Contagens, FiltroSala, SalaVM } from './model'
import './salas.css'

export interface ListaSalasProps {
  modo: RoomMode; onModo: (m: RoomMode) => void
  /** Salas já filtradas (modo + filtro + busca). */
  salas: SalaVM[]
  /** Quantidade de salas de cada modo, para as abas. */
  contagemOnline: number; contagemLocal: number
  /** Contagens do modo atual, para os filtros. */
  contagens: Contagens
  filtro: FiltroSala; onFiltro: (f: FiltroSala) => void
  busca: string; onBusca: (q: string) => void
  selecionadaId: string | null; onSelect: (id: string) => void
  apelido: string; onApelido: (n: string) => void; onApelidoConfirmar: () => void
  onCriar: () => void
  onEntrarManual: () => void
  /** Estado do servidor (modo online). Ausente = não configurado. */
  servidor?: ServerStatus
}

function estiloFiltro(ativo: boolean): CSSProperties {
  return {
    height: 36, padding: '0 14px', borderRadius: 999, fontSize: 13, fontWeight: 600, whiteSpace: 'nowrap', flex: '0 0 auto',
    border: `1px solid ${ativo ? '#3BF08A' : '#1F332D'}`,
    background: ativo ? 'rgba(59,240,138,0.13)' : 'transparent',
    color: ativo ? '#3BF08A' : '#A3BDB1',
  }
}

const VERDE = '#3BF08A'
const CINZA = '#7E978B'

/** Texto do rodapé no modo online, conforme o estado do servidor. */
function textoRodape(servidor: ServerStatus | undefined): string {
  switch (servidor?.state) {
    case 'online': return 'Conectado ao servidor streamaZap. A lista atualiza sozinha.'
    case 'connecting': return 'Conectando ao servidor…'
    case 'error': return 'Servidor desconectado. Tentando reconectar…'
    default: return 'Servidor não configurado. Informe o endereço em Configurações › Conexão.'
  }
}

export function ListaSalas(props: ListaSalasProps): ReactNode {
  const { modo, salas, contagens, filtro, busca, apelido, servidor } = props
  const filtros: { id: FiltroSala; label: string }[] = [
    { id: 'todas', label: `Todas · ${contagens.todas}` },
    { id: 'abertas', label: `Abertas · ${contagens.abertas}` },
    { id: 'senha', label: `Com senha · ${contagens.senha}` },
  ]
  const online = modo === 'online'
  // Local mantém o ponto verde pulsante; online só fica verde com o servidor conectado.
  const pontoAtivo = !online || servidor?.state === 'online'
  const corPonto = pontoAtivo ? VERDE : CINZA
  return (
    <section
      aria-label="Salas na rede"
      style={{
        flex: '1 1 340px', maxWidth: 392, minWidth: 340, display: 'flex', flexDirection: 'column',
        background: '#0F1916', borderRight: '1px solid #1F332D',
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '22px 20px 14px' }}>
        <h1 style={{ margin: 0, fontFamily: "'Unbounded', sans-serif", fontWeight: 700, fontSize: 22, letterSpacing: '-0.01em' }}>Salas</h1>
        <span
          style={{
            fontFamily: "'JetBrains Mono', monospace", fontSize: 11, color: '#3BF08A', padding: '3px 8px',
            borderRadius: 999, background: 'rgba(59,240,138,0.12)', whiteSpace: 'nowrap', flex: '0 0 auto',
          }}
        >{contagens.todas} salas</span>
        <span style={{ flex: 1 }}></span>
        <button
          type="button"
          className="zp-cta"
          onClick={props.onCriar}
          style={{
            height: 44, padding: '0 16px', borderRadius: 12, background: '#3BF08A', color: '#03140A', fontWeight: 700,
            border: 0, display: 'flex', alignItems: 'center', gap: 6, whiteSpace: 'nowrap', flex: '0 0 auto',
          }}
        ><IconeMais size={18} />Nova sala</button>
      </div>

      <ModoAbas modo={modo} onModo={props.onModo} contagemOnline={props.contagemOnline} contagemLocal={props.contagemLocal} />

      <div style={{ padding: '0 20px 12px' }}>
        <label htmlFor="busca" className="zp-sr">Buscar sala</label>
        <div style={{ position: 'relative' }}>
          <span style={{ position: 'absolute', left: 14, top: 13, display: 'flex' }}><IconeLupa size={18} cor="#7E978B" /></span>
          <input
            id="busca"
            type="search"
            placeholder="Buscar sala ou pessoa"
            value={busca}
            onChange={(e) => props.onBusca(e.target.value)}
            style={{
              width: '100%', height: 44, padding: '0 14px 0 42px', borderRadius: 12, border: '1px solid #1F332D',
              background: '#0A100E', color: '#E6F4EC', fontSize: 14,
            }}
          />
        </div>
      </div>

      <div role="group" aria-label="Filtrar salas" style={{ display: 'flex', flexWrap: 'wrap', gap: 8, padding: '0 20px 14px' }}>
        {filtros.map((f) => (
          <button key={f.id} type="button" aria-pressed={filtro === f.id} onClick={() => props.onFiltro(f.id)} style={estiloFiltro(filtro === f.id)}>
            {f.label}
          </button>
        ))}
      </div>

      <div style={{ flex: 1, display: 'flex', flexDirection: 'column', borderTop: '1px solid #18292A' }}>
        {salas.map((s) => (
          <SalaItem key={s.id} sala={s} selecionada={props.selecionadaId === s.id} onSelect={props.onSelect} />
        ))}
        {salas.length === 0 ? (
          <p style={{ margin: 0, padding: '28px 20px', color: '#A3BDB1', textAlign: 'center' }}>{props.busca.trim() ? 'Nenhuma sala com esse nome.' : online ? (servidor?.state === 'online' ? 'Nenhuma sala no servidor agora.' : 'Sem conexão com o servidor.') : 'Nenhuma sala na rede ainda. Crie uma ou peça o IP de quem está transmitindo.'}</p>
        ) : null}
        <div style={{ display: 'flex', alignItems: 'center', gap: 14, padding: '18px 20px' }}>
          <span style={{ position: 'relative', width: 24, height: 24, flex: '0 0 24px', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
            <span className={pontoAtivo ? 'zp-pulse' : undefined} style={{ position: 'absolute', width: 12, height: 12, borderRadius: '50%', border: `1px solid ${corPonto}` }}></span>
            <span style={{ width: 6, height: 6, borderRadius: '50%', background: corPonto }}></span>
          </span>
          <span style={{ color: '#7E978B', fontSize: 12.5, lineHeight: 1.45 }}>
            {online ? textoRodape(servidor) : 'Procurando mais salas no Radmin e na rede local…'}
          </span>
        </div>
      </div>

      <div style={{ display: 'flex', flexDirection: 'column', gap: 10, padding: '14px 20px 18px', borderTop: '1px solid #1F332D' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          <label htmlFor="apelido" style={{ fontSize: 12, color: '#A3BDB1', whiteSpace: 'nowrap' }}>Você aparece como</label>
          <input
            id="apelido"
            value={apelido}
            onChange={(e) => props.onApelido(e.target.value)}
            onBlur={props.onApelidoConfirmar}
            onKeyDown={(e) => { if (e.key === 'Enter') props.onApelidoConfirmar() }}
            style={{
              flex: 1, minWidth: 0, height: 40, padding: '0 12px', borderRadius: 10, border: '1px solid #1F332D',
              background: '#0A100E', color: '#E6F4EC', fontWeight: 600,
            }}
          />
        </div>
        <button
          type="button"
          className="zp-ghost"
          onClick={props.onEntrarManual}
          style={{
            height: 44, borderRadius: 12, border: '1px solid #2A443C', background: 'transparent', color: 'inherit',
            display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8, fontWeight: 600,
          }}
        ><IconeChave size={18} />{online ? 'Entrar com código' : 'Entrar por IP'}</button>
      </div>
    </section>
  )
}
