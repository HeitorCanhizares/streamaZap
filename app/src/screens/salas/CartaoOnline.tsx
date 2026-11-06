// Cartão discreto da tela Salas com o Online desligado (plano §4). Tarefa front-03.
import type { ReactNode } from 'react'
import { IconeGlobo } from './icons'

export interface CartaoOnlineProps { onAbrirConfig?: () => void }

const estiloCartao = {
  display: 'flex',
  alignItems: 'center',
  gap: 10,
  margin: '0 20px 12px',
  padding: '10px 14px',
  borderRadius: 12,
  border: '1px solid var(--borda)',
  background: 'var(--superficie)',
  fontSize: 12.5,
  color: 'var(--texto-2)',
} as const

const estiloLink = {
  border: 'none',
  background: 'transparent',
  padding: 0,
  cursor: 'pointer',
  color: 'var(--verde)',
  font: 'inherit',
} as const

// Aviso de que as salas pela internet estão desligadas. Só o trecho "ative o modo Online
// em Configurações" vira botão, quando há ação para ele; sem a prop, é texto simples.
export function CartaoOnline({ onAbrirConfig }: CartaoOnlineProps): ReactNode {
  return (
    <div data-testid="cartao-online" style={estiloCartao}>
      <IconeGlobo size={16} />
      <span>
        Salas pela internet, sem Radmin:{' '}
        {onAbrirConfig ? (
          <button type="button" onClick={onAbrirConfig} style={estiloLink}>
            ative o modo Online em Configurações
          </button>
        ) : (
          'ative o modo Online em Configurações'
        )}
        .
      </span>
    </div>
  )
}
