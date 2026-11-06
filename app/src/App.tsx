// Tarefa ui-28: navegação pelo trilho lateral e pelas telas (porte do esqueleto de docs/design).
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { isTauri } from './bridge/api'
import { useSnapshot } from './bridge/store'
import { NavRail } from './components/NavRail'
import { TitleBar } from './components/TitleBar'
import { SalaMetrics } from './components/shell/SalaMetrics'
import { UpdateBanner } from './components/shell/UpdateBanner'
import type { NavTarget } from './components/shell/types'
import { SalasScreen } from './screens/salas/SalasScreen'
import { CriarScreen } from './screens/criar/CriarScreen'
import { SalaScreen } from './screens/sala/SalaScreen'
import { ConfigScreen } from './screens/config/ConfigScreen'
import { DiagScreen } from './screens/diag/DiagScreen'

type View = NavTarget

const VERSAO = 'v3.1.3'

// Controles da janela só existem no Tauri. Fora dele viram no-op, sem tocar no módulo.
function acaoJanela(acao: (w: import('@tauri-apps/api/window').Window) => Promise<void>): void {
  if (!isTauri()) return
  void import('@tauri-apps/api/window')
    .then(({ getCurrentWindow }) => acao(getCurrentWindow()))
    .catch(() => {
      // Falha ao falar com a janela não deve derrubar a interface.
    })
}

export function App(): ReactNode {
  const snap = useSnapshot()
  const [view, setView] = useState<View>('salas')
  const temSala = snap.room !== null

  // Só reage à transição da sala (entrou ou saiu), não ao estado inicial:
  // se o app abre já com uma sala, a tela inicial continua sendo Salas.
  const temSalaAntes = useRef(temSala)
  useEffect(() => {
    if (temSalaAntes.current === temSala) return
    temSalaAntes.current = temSala
    if (temSala) setView((v) => (v === 'salas' || v === 'criar' ? 'sala' : v))
    else setView((v) => (v === 'sala' ? 'salas' : v))
  }, [temSala])

  // 'Sala atual' só faz sentido com sala ativa; 'Salas' sempre leva à lista.
  function onNavigate(to: NavTarget): void {
    if (to === 'sala' && !temSala) return
    setView(to)
  }

  // Trilha da barra de título por tela; Salas mostra a versão.
  let crumb: { atual: string; pai?: string } | undefined
  if (view === 'criar') crumb = { pai: 'Salas', atual: 'Nova sala' }
  else if (view === 'sala') crumb = { pai: 'Salas', atual: snap.room?.name ?? 'Sala' }
  else if (view === 'config') crumb = { atual: 'Configurações' }
  else if (view === 'diag') crumb = { atual: 'Diagnóstico de rede' }

  let tela: ReactNode
  switch (view) {
    case 'salas':
      tela = <SalasScreen onCreate={() => setView('criar')} onEntered={() => setView('sala')} />
      break
    case 'criar':
      tela = <CriarScreen onCreated={() => setView('sala')} onCancel={() => setView('salas')} />
      break
    case 'sala':
      tela = <SalaScreen onLeft={() => setView('salas')} />
      break
    case 'config':
      tela = <ConfigScreen />
      break
    case 'diag':
      tela = <DiagScreen />
      break
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh', background: 'var(--bg)' }}>
      <TitleBar
        version={VERSAO}
        crumb={crumb}
        radminIp={snap.network.radmin_ip}
        server={snap.server}
        right={view === 'sala' ? <SalaMetrics tile={snap.tiles[0] ?? null} share={snap.share} /> : undefined}
        onMinimize={() => acaoJanela((w) => w.minimize())}
        onMaximize={() => acaoJanela((w) => w.toggleMaximize())}
        onClose={() => acaoJanela((w) => w.close())}
      />
      <UpdateBanner onOpenConfig={() => setView('config')} />
      <div style={{ display: 'flex', flex: 1, minHeight: 0 }}>
        <NavRail active={view} onNavigate={onNavigate} inRoom={temSala} profileName={snap.me.name} />
        <main style={{ flex: 1, minWidth: 0, overflow: 'auto' }}>{tela}</main>
      </div>
    </div>
  )
}
