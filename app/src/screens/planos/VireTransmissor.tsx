// Cartão "Vire Transmissor" (plano sem transmissão). Tarefa telas-03.
import type { CSSProperties, ReactNode } from 'react'
import { abrirPlanos } from '../../bridge/planoAcoes'
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

export function VireTransmissor(): ReactNode {
  const cobranca = useCobrancaAtiva()
  return (
    <div style={CARTAO}>
      <div style={{ fontSize: 14, fontWeight: 600 }}>Vire Transmissor</div>
      <div role="status" style={{ fontSize: 13, color: 'var(--texto-2)' }}>
        {cobranca
          ? 'Para transmitir no Online, assine o Transmissor HD ou o Transmissor Full HD.'
          : 'Seu plano não permite transmitir no Online'}
      </div>
      {cobranca && (
        <div>
          <Button variant="primary" onClick={() => abrirPlanos('hd')}>
            Ver planos
          </Button>
        </div>
      )}
    </div>
  )
}
