// Tarefa salas-06: aviso de firewall (porte de 1-salas.html, linhas 144-151).
// role="alert" faz leitor de tela anunciar o aviso assim que ele aparece.
import type { ReactNode } from 'react'
import { IconeEscudo } from './icons'

export interface FirewallAvisoProps { onCorrigir: () => void }

export function FirewallAviso(props: FirewallAvisoProps): ReactNode {
  return (
    <div
      role="alert"
      style={{
        display: 'flex',
        flexWrap: 'wrap',
        alignItems: 'center',
        gap: 14,
        padding: '12px 14px 12px 18px',
        borderRadius: 14,
        background: 'rgba(255,181,71,0.1)',
        border: '1px solid rgba(255,181,71,0.35)',
      }}
    >
      <IconeEscudo size={20} cor="#FFB547" />
      <span style={{ flex: '1 1 300px', color: '#F4E3C4' }}>
        O Firewall do Windows está bloqueando quem tenta entrar nas suas salas.
      </span>
      <button
        type="button"
        onClick={props.onCorrigir}
        style={{
          height: 40,
          padding: '0 16px',
          borderRadius: 10,
          border: 0,
          background: '#FFB547',
          color: '#1A1204',
          fontWeight: 700,
        }}
      >
        Corrigir firewall
      </button>
    </div>
  )
}
