// Parte "logada" da seção Conta: e-mail, plano, contas vinculadas, sair e excluir. Tarefa front-12.
import { useState } from 'react'
import type { CSSProperties, ReactNode } from 'react'
import type { PlanView } from '../../bridge/types'
import { Button } from '../../components/Button'
import { useContaEstado } from '../../bridge/contaStore'
import { sair } from '../../bridge/contaAcoes'
import { desvincular, excluirConta, vincular } from '../../bridge/contaVinculo'
import { podeDesvincular, rotuloProvedor } from '../../bridge/contaModel'
import { ContaPlano } from './ContaPlano'

export interface ContaLogadaProps {
  /** `snapshot.account.plan`; ausente = não mostra a linha do plano. */
  plano?: PlanView
}

const dica: CSSProperties = { fontSize: 12, color: '#7E978B' }
const coluna: CSSProperties = { display: 'flex', flexDirection: 'column', gap: 12 }
const linha: CSSProperties = { display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 10 }

/**
 * Lê `useContaEstado()`. Mostra "Conectado como <email>"; a linha do plano via `ContaPlano`
 * (sem preço, sem "Assinar"); lista de `identidades` com `rotuloProvedor` e e-mail, botão
 * "Desvincular" em cada (desabilitado se `!podeDesvincular`, com dica "Mantenha ao menos uma forma
 * de entrar."); botões "Vincular Google" / "Vincular Microsoft" para os provedores de
 * `provedores` ainda não vinculados (`vincular`); "Sair" (`sair`); "Excluir conta" abre confirmação
 * (`role="alertdialog"`, "Isso apaga sua conta e não pode ser desfeito.", botões "Excluir de
 * vez" -> `excluirConta`, "Cancelar"). `aviso` em `role="alert"`.
 */
export function ContaLogada({ plano }: ContaLogadaProps): ReactNode {
  const { email, identidades, aviso, ocupado, provedores } = useContaEstado()
  const [confirmando, setConfirmando] = useState(false)
  const podeDesv = podeDesvincular(identidades)

  return (
    <div style={{ ...coluna, padding: 18, borderRadius: 16, background: '#0A100E', border: '1px solid #1F332D' }}>
      <span>
        {email ? <>Conectado como <b>{email}</b></> : 'Conectado'}
      </span>
      <ContaPlano plano={plano} />

      <div style={{ ...coluna, gap: 8 }}>
        <span style={{ fontWeight: 600 }}>Contas vinculadas</span>
        <ul style={{ ...coluna, gap: 8, margin: 0, padding: 0, listStyle: 'none' }}>
          {identidades.map((i) => {
            const rotulo = rotuloProvedor(i.provider)
            return (
              <li key={i.identity_id} style={linha}>
                <span>{rotulo}{i.email ? ` · ${i.email}` : ''}</span>
                <button
                  type="button"
                  className="zp-btn zp-btn-ghost"
                  aria-label={`Desvincular ${rotulo}`}
                  disabled={!podeDesv || ocupado}
                  onClick={() => void desvincular(i)}
                >
                  Desvincular
                </button>
              </li>
            )
          })}
        </ul>
        {identidades.length === 1 && <span style={dica}>Mantenha ao menos uma forma de entrar.</span>}
      </div>

      <div style={linha}>
        {provedores.filter((id) => !identidades.some((i) => i.provider === id)).map((id) => (
          <Button key={id} variant="ghost" disabled={ocupado} onClick={() => void vincular(id)}>
            {`Vincular ${rotuloProvedor(id)}`}
          </Button>
        ))}
      </div>

      {aviso && <span role="alert" style={{ fontSize: 13, color: '#FF6B6B' }}>{aviso}</span>}

      <div style={linha}>
        <Button variant="ghost" disabled={ocupado} onClick={() => void sair()}>Sair</Button>
        {!confirmando && (
          <Button variant="danger" disabled={ocupado} onClick={() => setConfirmando(true)}>Excluir conta</Button>
        )}
      </div>

      {confirmando && (
        <div
          role="alertdialog"
          aria-label="Excluir conta"
          style={{ ...coluna, gap: 10, padding: 14, borderRadius: 12, background: '#0F1916', border: '1px solid #2A443C' }}
        >
          <span style={{ fontSize: 13, color: '#E6F4EC' }}>Isso apaga sua conta e não pode ser desfeito.</span>
          <div style={linha}>
            <Button variant="danger" disabled={ocupado} onClick={() => void excluirConta()}>Excluir de vez</Button>
            <Button variant="ghost" disabled={ocupado} onClick={() => setConfirmando(false)}>Cancelar</Button>
          </div>
        </div>
      )}
    </div>
  )
}
