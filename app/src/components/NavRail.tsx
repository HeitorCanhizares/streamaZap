// Tarefa shell-06: trilho lateral fiel a docs/design/1-salas.html (linhas 55-69) e ao estado em sala de 3-sala.html.
import type { ReactNode } from 'react'
import type { NavRailProps, NavTarget } from './shell/types'
import { IconConfig, IconCriar, IconDiag, IconSalaAtual, IconSalas } from './shell/railIcons'

export type { NavRailProps, NavTarget } from './shell/types'

// Inicial do perfil: primeiro code point do nome, em maiúscula. Vazio vira "?".
function inicialDe(name: string): string {
  const primeiro = [...name.trim()][0]
  return primeiro ? primeiro.toUpperCase() : '?'
}

// Trilho lateral de navegação. O item ativo recebe aria-current="page" e a classe zp-rail-active.
export function NavRail({ active, onNavigate, inRoom = false, profileName }: NavRailProps): ReactNode {
  const name = profileName ?? ''

  const item = (alvo: NavTarget): { className: string; 'aria-current': 'page' | undefined } => {
    const ativo = active === alvo
    return {
      className: 'zp-rail' + (ativo ? ' zp-rail-active' : ''),
      'aria-current': ativo ? 'page' : undefined,
    }
  }

  return (
    <nav className="zp-navrail" aria-label="Navegação principal">
      <img
        src="/icone.png"
        alt="streamaZap"
        style={{
          width: 52,
          height: 52,
          borderRadius: 15,
          display: 'block',
          marginBottom: 10,
          boxShadow: '0 0 0 1px var(--borda-forte), 0 0 20px rgba(59,240,138,0.35)',
        }}
      />

      <button type="button" aria-label="Salas" title="Salas" {...item('salas')} onClick={() => onNavigate('salas')}>
        <IconSalas />
      </button>

      {!inRoom ? (
        <button type="button" aria-label="Criar sala" title="Criar sala" {...item('criar')} onClick={() => onNavigate('criar')}>
          <IconCriar />
        </button>
      ) : (
        // Sem aria-label: o nome acessível vem do svg "Sala atual", evitando duplicar o nome.
        <button type="button" title="Sala atual" {...item('sala')} onClick={() => onNavigate('sala')}>
          <IconSalaAtual />
          <span
            className="zp-blink"
            aria-hidden="true"
            style={{
              position: 'absolute',
              right: 8,
              top: 8,
              width: 7,
              height: 7,
              borderRadius: '50%',
              background: 'var(--vermelho)',
            }}
          />
        </button>
      )}

      <button type="button" aria-label="Diagnóstico de rede" title="Diagnóstico" {...item('diag')} onClick={() => onNavigate('diag')}>
        <IconDiag />
      </button>

      <button type="button" aria-label="Configurações" title="Configurações" {...item('config')} onClick={() => onNavigate('config')}>
        <IconConfig />
      </button>

      <span style={{ flex: 1 }} />

      <button
        type="button"
        aria-label={`Seu perfil: ${name}`}
        title={name}
        onClick={() => onNavigate('config')}
        style={{
          width: 44,
          height: 44,
          borderRadius: '50%',
          border: 0,
          background: 'rgba(59,240,138,0.16)',
          color: 'var(--verde)',
          fontWeight: 700,
          fontSize: 15,
          cursor: 'pointer',
          boxShadow: '0 0 0 2px var(--bg-barra), 0 0 0 3px var(--borda-forte)',
        }}
      >
        {inicialDe(name)}
      </button>
    </nav>
  )
}
