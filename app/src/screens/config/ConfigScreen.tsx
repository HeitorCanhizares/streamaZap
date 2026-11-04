// Tarefa ui-26: tela de Configurações. Compõe as seções e salva sozinho a cada mudança.
import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../../bridge/api'
import { useSnapshot } from '../../bridge/store'
import type { Settings, Sources } from '../../bridge/types'
import { Badge } from '../../components/Badge'
import { ConfigConexao } from './ConfigConexao'
import { ConfigPerfil } from './ConfigPerfil'
import { ConfigVideoSom } from './ConfigVideoSom'

// Tempo que o indicador 'Salvo' fica visível depois de cada salvamento bem-sucedido.
const SALVO_MS = 1500

// Fontes vazias enquanto a lista não chega: as seções de vídeo continuam renderizando.
const FONTES_VAZIAS: Sources = { screens: [], windows: [], cameras: [], apps: [], encoders: [] }

export function ConfigScreen(): ReactNode {
  const { network } = useSnapshot()
  const [settings, setSettings] = useState<Settings | null>(null)
  const [sources, setSources] = useState<Sources | null>(null)
  const [erro, setErro] = useState<string | null>(null)
  // Contador de salvamentos: cada um reinicia o aviso 'Salvo' e seu temporizador.
  const [salvamentos, setSalvamentos] = useState(0)
  const [mostrarSalvo, setMostrarSalvo] = useState(false)

  // Carrega as configurações e a lista de fontes ao abrir a tela.
  useEffect(() => {
    let vivo = true
    api
      .settingsGet()
      .then((s) => {
        if (vivo) setSettings(s)
      })
      .catch((e: unknown) => {
        if (vivo) setErro(String(e))
      })
    api
      .sourcesList()
      .then((s) => {
        if (vivo) setSources(s)
      })
      .catch((e: unknown) => {
        if (vivo) setErro(String(e))
      })
    return () => {
      vivo = false
    }
  }, [])

  // Mostra 'Salvo' por SALVO_MS; uma nova gravação cancela o temporizador anterior.
  useEffect(() => {
    if (salvamentos === 0) return
    setMostrarSalvo(true)
    const t = setTimeout(() => setMostrarSalvo(false), SALVO_MS)
    return () => clearTimeout(t)
  }, [salvamentos])

  // Mudança imediata na tela; o retorno do app substitui o estado. Em erro, recarrega do app.
  function onChange(patch: Partial<Settings>): void {
    setSettings((atual) => (atual ? { ...atual, ...patch } : atual))
    api
      .settingsSet(patch)
      .then((s) => {
        setErro(null)
        setSettings(s)
        setSalvamentos((n) => n + 1)
      })
      .catch(async (e: unknown) => {
        setErro(String(e))
        try {
          setSettings(await api.settingsGet())
        } catch {
          // Sem como recarregar: fica com o último estado mostrado.
        }
      })
  }

  return (
    <main style={{ display: 'flex', flexDirection: 'column', gap: 24, padding: '28px 32px', minWidth: 0 }}>
      <header style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 16, flexWrap: 'wrap' }}>
        <h1 style={{ margin: 0, fontFamily: "'Unbounded', sans-serif", fontWeight: 700, fontSize: 32, letterSpacing: '-0.02em' }}>
          Configurações
        </h1>
        {mostrarSalvo ? (
          <Badge tone="green">Salvo</Badge>
        ) : (
          <span style={{ fontSize: 12.5, color: '#A3BDB1' }}>Tudo salva sozinho</span>
        )}
      </header>

      {erro && (
        <div role="alert" style={{ color: '#FF6B6B', fontSize: 13 }}>
          {erro}
        </div>
      )}

      {settings === null ? (
        <p style={{ color: '#A3BDB1', margin: 0 }}>Carregando…</p>
      ) : (
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 20, alignItems: 'stretch' }}>
          <ConfigPerfil settings={settings} onChange={onChange} />
          <ConfigConexao settings={settings} onChange={onChange} network={network} />
          <ConfigVideoSom settings={settings} onChange={onChange} sources={sources ?? FONTES_VAZIAS} />
        </div>
      )}
    </main>
  )
}
