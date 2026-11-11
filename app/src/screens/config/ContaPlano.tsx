// Parte do plano dentro de `ContaLogada`. Tarefa telas-06.
import type { CSSProperties, ReactNode } from 'react'
import type { PlanView } from '../../bridge/types'
import { Button } from '../../components/Button'
import { usePlanoEstado } from '../../bridge/planoStore'
import { abrirPlanos, atualizarDepoisDePagar } from '../../bridge/planoAcoes'
import { textoAssinatura, textoPlanoConta } from '../../bridge/planoModel'

export interface ContaPlanoProps { plano?: PlanView }

const dica: CSSProperties = { fontSize: 12, color: '#7E978B' }
const coluna: CSSProperties = { display: 'flex', flexDirection: 'column', gap: 12 }
const linha: CSSProperties = { display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 10 }

/**
 * Linha do plano na Conta. Sem cobrança mostra só o texto do plano (beta gratuito); com cobrança
 * soma validade da assinatura, botões de assinar/atualizar e os avisos do fluxo de pagamento.
 */
export function ContaPlano({ plano }: ContaPlanoProps): ReactNode {
  const { config, assinatura, aguardandoPagamento, info, aviso, ocupado } = usePlanoEstado()
  const texto = textoPlanoConta(plano, config)
  if (texto === null) return null

  if (!config.cobrancaAtiva) return <span>{texto}</span>

  return (
    <div style={{ ...coluna, gap: 8 }}>
      <span>{texto}</span>
      {assinatura && <span style={dica}>{textoAssinatura(assinatura, new Date())}</span>}
      <div style={linha}>
        <Button variant="ghost" disabled={ocupado} onClick={() => abrirPlanos()}>Assinar / trocar plano</Button>
        {aguardandoPagamento && (
          <Button variant="ghost" disabled={ocupado} onClick={() => void atualizarDepoisDePagar()}>
            Já assinei, atualizar
          </Button>
        )}
      </div>
      {info && <span role="status" style={dica}>{info}</span>}
      {aviso && <span role="alert" style={{ fontSize: 13, color: '#FF6B6B' }}>{aviso}</span>}
    </div>
  )
}
