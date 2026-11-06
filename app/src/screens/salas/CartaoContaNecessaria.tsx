// Cartão da tela Salas quando o Online está ligado mas não há conta entrada (front-15).
// Mesmo visual do CartaoOnline, em coluna, com o botão de entrar logo abaixo do aviso.
import type { ReactNode } from 'react'
import { ContaEntrar } from '../config/ContaEntrar'

// Mesmo encaixe do AtualizadoCard no detalhe da tela Salas.
const estiloCartao = {
  display: 'flex',
  flexDirection: 'column',
  gap: 10,
  margin: '16px 32px 0',
  padding: '14px 18px',
  borderRadius: 14,
  border: '1px solid #1F332D',
  background: '#0A100E',
  fontSize: 12.5,
  color: '#A3BDB1',
} as const

export function CartaoContaNecessaria(): ReactNode {
  return (
    <div data-testid="cartao-conta-necessaria" style={estiloCartao}>
      <strong style={{ color: '#E6F4EC', fontSize: 14 }}>Entre com sua conta para usar o Online</strong>
      <ContaEntrar />
    </div>
  )
}
