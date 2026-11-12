// Convite de sala chegou com a pessoa já numa sala: pergunta antes de sair dela.
import { useEffect, type ReactNode } from 'react'
import { Button } from '../Button'

export interface ConfirmarConviteProps {
  /** Como o destino aparece: 'ZAP-7K2Q' ou 'ip:porta'. */
  destino: string
  /** Nome da sala em que a pessoa está. */
  salaAtual: string
  ocupado: boolean
  erro: string | null
  onConfirmar: () => void
  onCancelar: () => void
}

export function ConfirmarConvite({ destino, salaAtual, ocupado, erro, onConfirmar, onCancelar }: ConfirmarConviteProps): ReactNode {
  // Esc fecha sem sair da sala.
  useEffect(() => {
    const aoTeclar = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') onCancelar()
    }
    window.addEventListener('keydown', aoTeclar)
    return () => window.removeEventListener('keydown', aoTeclar)
  }, [onCancelar])

  return (
    <div style={{ position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.6)', zIndex: 50, display: 'grid', placeItems: 'center' }}>
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="confirmar-convite-titulo"
        style={{ width: 'min(440px, 92vw)', display: 'flex', flexDirection: 'column', gap: 14, padding: 24, borderRadius: 18, background: 'var(--superficie)', border: '1px solid var(--borda)' }}
      >
        <h2 id="confirmar-convite-titulo" style={{ margin: 0, fontSize: 17, fontWeight: 600 }}>
          Entrar na sala do convite?
        </h2>
        <p style={{ margin: 0, fontSize: 14, color: 'var(--texto-2)', lineHeight: 1.5 }}>
          Você está na sala "{salaAtual}". Para entrar em <b style={{ color: 'var(--texto)' }}>{destino}</b>, o app sai dela.
        </p>
        {erro !== null ? (
          <p role="alert" style={{ margin: 0, color: 'var(--vermelho)', fontSize: 13 }}>{erro}</p>
        ) : null}
        <div style={{ display: 'flex', gap: 10, justifyContent: 'flex-end' }}>
          <Button variant="ghost" onClick={onCancelar} disabled={ocupado}>Agora não</Button>
          <Button variant="primary" onClick={onConfirmar} disabled={ocupado}>Sair e entrar</Button>
        </div>
      </div>
    </div>
  )
}
