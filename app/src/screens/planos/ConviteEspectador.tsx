// Convite para assinar o Espectador (plano que não entra em sala Online). Tarefa telas-03.
import type { CSSProperties, ReactNode } from 'react'
import { abrirPlanos } from '../../bridge/planoAcoes'
import { textoConvite } from '../../bridge/planoModel'
import { usePlanoEstado } from '../../bridge/planoStore'
import { useCobrancaAtiva } from '../../bridge/usePlano'
import { Button } from '../../components/Button'

const CARTAO: CSSProperties = {
  display: 'flex',
  flexDirection: 'column',
  gap: 10,
  padding: 20,
  borderRadius: 18,
  background: 'var(--superficie)',
  border: '1px solid var(--borda)',
}

export function ConviteEspectador(): ReactNode {
  const cobranca = useCobrancaAtiva()
  const { precos } = usePlanoEstado().config
  // Com cobrança mas sem preços (não deveria acontecer): trata como sem cobrança.
  const oferta = cobranca && precos != null ? precos : null
  return (
    <div style={CARTAO}>
      <div role="status" style={{ fontSize: 13, color: 'var(--texto-2)' }}>
        {oferta ? textoConvite(oferta) : 'Sua conta não tem um plano para entrar em salas Online'}
      </div>
      {oferta && (
        <div>
          <Button variant="primary" onClick={() => abrirPlanos('espectador')}>
            Ver planos
          </Button>
        </div>
      )}
    </div>
  )
}
