// Seção "Conta" (dentro do cartão Online de Configurações → Conexão). Tarefa front-13.
import type { ReactNode } from 'react'
import { useSnapshot } from '../../bridge/store'
import { useContaEstado } from '../../bridge/contaStore'
import { temLogin } from '../../bridge/contaModel'
import { ContaEntrar } from './ContaEntrar'
import { ContaLogada } from './ContaLogada'

/**
 * Lê `useSnapshot().account` e `useContaEstado()`. Devolve `null` quando `!temLogin(account)`
 * (servidor sem `conta` não mostra login). Título "Conta"; `sessao === 'carregando'`: "Carregando
 * conta…"; `'fora'`: `<ContaEntrar />`; `'dentro'`: `<ContaLogada plano={account.plan} />`.
 * Não chama `iniciarConta` (isso é do App).
 */
export function ConfigConta(): ReactNode {
  const account = useSnapshot().account
  const conta = useContaEstado()
  if (!temLogin(account)) return null
  return (
    <div
      data-testid="config-conta"
      style={{ display: 'flex', flexDirection: 'column', gap: 10, padding: 14, borderRadius: 12, border: '1px solid #1F332D' }}
    >
      <span style={{ fontWeight: 600, fontSize: 13.5 }}>Conta</span>
      {conta.sessao === 'carregando' ? (
        <span style={{ fontSize: 12, color: '#7E978B' }}>Carregando conta…</span>
      ) : conta.sessao === 'fora' ? (
        <ContaEntrar />
      ) : (
        <ContaLogada plano={account?.plan} />
      )}
    </div>
  )
}
