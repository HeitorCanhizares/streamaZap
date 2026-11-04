// Aviso global "Atualização X pronta" (tarefa ui5-06).
// Faixa fina acima do app: mostra o andamento da atualização e, quando há um amigo com
// protocolo mais novo, convida a atualizar pela tela de configuração.
import { useState, type ReactNode } from 'react'
import { api } from '../../bridge/api'
import { useSnapshot } from '../../bridge/store'
import { useUpdateState } from '../../bridge/updateStore'

const estiloFaixa = {
  height: 36,
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

export function UpdateBanner({ onOpenConfig }: { onOpenConfig: () => void }): ReactNode {
  const upd = useUpdateState()
  const snap = useSnapshot()
  const [erro, setErro] = useState<string | null>(null)

  // Reinstalar com sala aberta derrubaria a conversa: o botão só fica ativo fora de sala.
  const emSala = snap.room !== null

  async function reiniciar(): Promise<void> {
    setErro(null)
    try {
      await api.updateInstallNow()
    } catch (e) {
      setErro(e instanceof Error ? e.message : String(e))
    }
  }

  if (upd.fase === 'pronta') {
    return (
      <div role="status" style={estiloFaixa}>
        <span>{`Atualização ${upd.versao ?? ''} pronta — instala ao fechar`}</span>
        <button
          type="button"
          className="zp-ghost"
          style={{ ...estiloBotao, cursor: emSala ? 'not-allowed' : 'pointer' }}
          disabled={emSala}
          title={emSala ? 'Saia da sala para reiniciar' : undefined}
          onClick={() => void reiniciar()}
        >
          Reiniciar agora
        </button>
        {erro !== null && (
          <span role="alert" style={{ color: '#FF8A8A' }}>
            {erro}
          </span>
        )}
      </div>
    )
  }

  if (upd.fase === 'baixando') {
    return (
      <div role="status" style={estiloFaixa}>
        <span>{`Baixando atualização ${upd.versao ?? ''}… ${upd.progresso}%`}</span>
      </div>
    )
  }

  if (snap.newer_protocol !== undefined && upd.fase === 'nada') {
    return (
      <div role="status" style={estiloFaixa}>
        <span>Um amigo está com uma versão mais nova do streamaZap.</span>
        <button type="button" className="zp-ghost" style={estiloBotao} onClick={onOpenConfig}>
          Atualizar
        </button>
      </div>
    )
  }

  return null
}
