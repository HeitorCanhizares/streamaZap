// Faixa global de recusa por plano (`snapshot.plan_notice`). Tarefa telas-04.
import { useState, type ReactNode } from 'react'
import { abrirPlanos } from '../../bridge/planoAcoes'
import { useSnapshot } from '../../bridge/store'
import { useCobrancaAtiva } from '../../bridge/usePlano'
import type { PlanoPago } from '../../bridge/planoTipos'

const estiloFaixa = {
  minHeight: 36,
  display: 'flex',
  alignItems: 'center',
  gap: 12,
  padding: '0 16px',
  background: 'rgba(59,240,138,0.12)',
  borderBottom: '1px solid #1F332D',
  color: '#E6F4EC',
  fontSize: 13,
} as const

const estiloBotao = {
  height: 26,
  padding: '0 12px',
  border: '1px solid #2A443C',
  borderRadius: 8,
  background: 'transparent',
  color: '#E6F4EC',
  cursor: 'pointer',
  fontSize: 13,
} as const

export function AvisoPlano(): ReactNode {
  const snap = useSnapshot()
  const cobranca = useCobrancaAtiva()
  // Último `seq` dispensado: um aviso novo (seq maior) volta a aparecer.
  const [fechado, setFechado] = useState<number>(0)
  const aviso = snap.plan_notice
  if (aviso === undefined || aviso.seq <= fechado) return null

  const destaque: PlanoPago =
    aviso.action === 'entrar' ? 'espectador' : aviso.action === 'transmitir' && aviso.plan === 'hd' ? 'fullhd' : 'hd'

  return (
    <div role="alert" style={estiloFaixa}>
      <span>{aviso.message}</span>
      {cobranca && (
        <button type="button" className="zp-ghost" style={estiloBotao} onClick={() => abrirPlanos(destaque)}>
          Ver planos
        </button>
      )}
      <button type="button" className="zp-ghost" style={estiloBotao} onClick={() => setFechado(aviso.seq)}>
        Fechar
      </button>
    </div>
  )
}
