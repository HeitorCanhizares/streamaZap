// Tarefa ui-28: navegação pelo trilho lateral e pelas telas (porte do esqueleto de docs/design).
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { isTauri } from './bridge/api'
import { useSnapshot } from './bridge/store'
import { NavRail } from './components/NavRail'
import { TitleBar } from './components/TitleBar'
import { SalasScreen } from './screens/salas/SalasScreen'
import { CriarScreen } from './screens/criar/CriarScreen'
import { SalaScreen } from './screens/sala/SalaScreen'
import { ConfigScreen } from './screens/config/ConfigScreen'
import { DiagScreen } from './screens/diag/DiagScreen'

type View = 'salas' | 'criar' | 'sala' | 'config' | 'diag'
type Rail = 'salas' | 'config' | 'diag'

// Item do trilho ativo: 'criar' e 'sala' contam como 'salas'.
function railDe(view: View): Rail {
  return view === 'config' || view === 'diag' ? view : 'salas'
}

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

  // Com sala ativa, o trilho "Salas" leva direto à sala em andamento.
  function onNavigate(to: Rail): void {
    if (to === 'salas' && temSala) setView('sala')
    else setView(to)
  }

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
        title="streamaZap"
        onMinimize={() => acaoJanela((w) => w.minimize())}
        onMaximize={() => acaoJanela((w) => w.toggleMaximize())}
        onClose={() => acaoJanela((w) => w.close())}
      />
      <div style={{ display: 'flex', flex: 1, minHeight: 0 }}>
        <div style={{ flex: '0 0 76px', display: 'flex' }}>
          <NavRail active={railDe(view)} onNavigate={onNavigate} />
        </div>
        <main style={{ flex: 1, minWidth: 0, overflow: 'auto' }}>{tela}</main>
      </div>
    </div>
  )
}
