// Tarefa salas-12: painel de detalhe da sala (coluna direita da tela Salas).
// Porte de docs/design/1-salas.html (área <main class="zp-dots">). Sem lógica própria:
// só decide a ordem das seções e repassa as props.
import type { CSSProperties, ReactNode } from 'react'
import './salas.css'
import type { SalaVM } from './model'
import { FirewallAviso } from './FirewallAviso'
import { SalaCabecalho } from './SalaCabecalho'
import { SalaPrevia } from './SalaPrevia'
import { SalaAcesso } from './SalaAcesso'
import { SalaCompartilhando } from './SalaCompartilhando'
import { SalaNaSala } from './SalaNaSala'

export interface DetalheSalaProps {
  sala: SalaVM | null
  mostrarFirewall: boolean; onCorrigirFirewall: () => void
  senha: string; onSenha: (s: string) => void
  ocupado: boolean; erro: string | null; onEntrar: () => void
  /** Convite para assinar (conta sem plano para entrar no Online); aparece abaixo do acesso. */
  convite?: ReactNode
}

const principal: CSSProperties = {
  flex: '999 1 560px',
  minWidth: 0,
  display: 'flex',
  flexDirection: 'column',
  gap: 22,
  padding: '28px 32px 32px',
}

const linha: CSSProperties = { display: 'flex', flexWrap: 'wrap', gap: 22, alignItems: 'flex-start' }

export function DetalheSala({
  sala,
  mostrarFirewall,
  onCorrigirFirewall,
  senha,
  onSenha,
  ocupado,
  erro,
  onEntrar,
  convite,
}: DetalheSalaProps): ReactNode {
  // O aviso do firewall vale mesmo sem sala selecionada: é do computador, não da sala.
  const aviso = mostrarFirewall ? <FirewallAviso onCorrigir={onCorrigirFirewall} /> : null

  if (sala === null) {
    return (
      <main className="zp-dots" style={{ ...principal, alignItems: 'stretch' }}>
        {aviso}
        {convite}
        <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
          <p style={{ margin: 0, color: '#A3BDB1', textAlign: 'center' }}>Escolha uma sala na lista</p>
        </div>
      </main>
    )
  }

  return (
    <main className="zp-dots" style={principal}>
      {aviso}
      <SalaCabecalho sala={sala} />
      <div style={linha}>
        <SalaPrevia sala={sala} />
        <SalaAcesso
          sala={sala}
          senha={senha}
          onSenha={onSenha}
          ocupado={ocupado}
          erro={erro}
          onEntrar={onEntrar}
        />
      </div>
      {convite}
      <div style={linha}>
        <SalaCompartilhando sala={sala} />
        <SalaNaSala sala={sala} />
      </div>
    </main>
  )
}
