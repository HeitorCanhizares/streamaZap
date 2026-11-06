// Tarefa salas-03: abas Online/Local da tela Salas, porte de 1-salas.html (linhas 71-81 e
// lógica `modes` do script). Os estilos ficam inline para bater com o design pixel a pixel.
import type { CSSProperties, ReactNode } from 'react'
import type { RoomMode } from '../../bridge/types'
import { useOnlineEnabled } from '../../bridge/onlineStore'
import { IconeGlobo, IconeWifi } from './icons'

export interface ModoAbasProps { modo: RoomMode; onModo: (m: RoomMode) => void; contagemOnline: number; contagemLocal: number }

const CONTAINER: CSSProperties = {
  margin: '0 20px 12px',
  display: 'grid',
  gridTemplateColumns: 'repeat(2, minmax(0, 1fr))',
  gap: 4,
  padding: 4,
  borderRadius: 14,
  background: '#0A100E',
  border: '1px solid #1F332D',
}

// Estilo do botão de aba, conforme o `style` gerado no script do design.
function estiloBotao(ativo: boolean): CSSProperties {
  return {
    display: 'flex',
    alignItems: 'center',
    gap: 10,
    minHeight: 52,
    minWidth: 0,
    padding: '6px 12px',
    borderRadius: 10,
    border: 0,
    textAlign: 'left',
    background: ativo ? '#1A3A2C' : 'transparent',
    color: ativo ? '#3BF08A' : '#A3BDB1',
  }
}

function estiloSub(ativo: boolean): CSSProperties {
  return {
    fontSize: 11.5,
    color: ativo ? '#9FE8C0' : '#7E978B',
    lineHeight: 1.25,
    overflowWrap: 'anywhere',
  }
}

export function ModoAbas(props: ModoAbasProps): ReactNode {
  const { modo, onModo, contagemOnline, contagemLocal } = props
  const habilitado = useOnlineEnabled()
  // Online desligado: só a lista Local, sem abas.
  if (!habilitado) return null
  const abas: { id: RoomMode; label: string; sub: string; icone: ReactNode; contagem: number }[] = [
    { id: 'online', label: 'Online', sub: 'via servidor', icone: <IconeGlobo size={20} />, contagem: contagemOnline },
    { id: 'local', label: 'Local', sub: 'Radmin / rede local', icone: <IconeWifi size={20} />, contagem: contagemLocal },
  ]

  return (
    <div role="tablist" aria-label="Onde procurar salas" style={CONTAINER}>
      {abas.map((aba) => {
        const ativo = modo === aba.id
        return (
          <button
            key={aba.id}
            type="button"
            role="tab"
            aria-selected={ativo}
            onClick={() => onModo(aba.id)}
            style={estiloBotao(ativo)}
          >
            <span style={{ flex: '0 0 20px', display: 'flex' }}>{aba.icone}</span>
            <span style={{ display: 'flex', flexDirection: 'column', alignItems: 'flex-start', gap: 1, minWidth: 0, overflow: 'hidden' }}>
              <span style={{ fontWeight: 700, fontSize: 14 }}>{aba.label}</span>
              <span style={estiloSub(ativo)}>{`${aba.sub} · ${aba.contagem}`}</span>
            </span>
          </button>
        )
      })}
    </div>
  )
}
