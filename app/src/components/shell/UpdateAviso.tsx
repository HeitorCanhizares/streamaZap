// Aviso de 30 s "Atualizando para a vX em 30 s · Adiar 1 h" (plano §13). Tarefa front-13.
// A instalação em si é do Rust: aqui só mostramos a contagem e deixamos adiar.
import { useEffect, useState, type ReactNode } from 'react'
import { api } from '../../bridge/api'
import { setUpdateAviso, useUpdateAviso } from '../../bridge/updateAvisoStore'

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

export function UpdateAviso(): ReactNode {
  const aviso = useUpdateAviso()
  const versaoAviso = aviso?.versao
  const segundosAviso = aviso?.segundos
  const [restante, setRestante] = useState(segundosAviso ?? 0)
  const [ocupado, setOcupado] = useState(false)
  const [erro, setErro] = useState<string | null>(null)

  // Reinicia a contagem quando o Rust manda um aviso novo.
  useEffect(() => {
    if (segundosAviso === undefined) return
    setRestante(segundosAviso)
    setErro(null)
    const id = setInterval(() => {
      setRestante((r) => Math.max(0, r - 1))
    }, 1000)
    return () => clearInterval(id)
  }, [versaoAviso, segundosAviso])

  if (aviso === null) return null

  async function adiar(): Promise<void> {
    if (aviso === null) return
    setOcupado(true)
    setErro(null)
    try {
      await api.updatePostpone()
      setUpdateAviso({ versao: aviso.versao, segundos: 0 })
    } catch (e) {
      setErro(e instanceof Error ? e.message : String(e))
    } finally {
      setOcupado(false)
    }
  }

  const versao = aviso.versao.startsWith('v') ? aviso.versao : `v${aviso.versao}`

  return (
    <div role="status" aria-live="polite" data-testid="update-aviso" style={estiloFaixa}>
      <span>{`Atualizando para a ${versao} em ${restante} s`}</span>
      <button
        type="button"
        className="zp-ghost"
        style={{ ...estiloBotao, cursor: ocupado ? 'not-allowed' : 'pointer' }}
        disabled={ocupado}
        onClick={() => void adiar()}
      >
        Adiar 1 h
      </button>
      {erro !== null && (
        <span role="alert" style={{ color: '#FF8A8A' }}>
          {erro}
        </span>
      )}
    </div>
  )
}
