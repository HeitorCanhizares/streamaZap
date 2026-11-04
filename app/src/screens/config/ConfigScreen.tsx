// Tarefa ui-26: tela de Configurações. Compõe as seções e salva sozinho a cada mudança.
import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { api } from '../../bridge/api'
import { useSnapshot } from '../../bridge/store'
import type { Settings, Sources } from '../../bridge/types'
import { ConfigConexao } from './ConfigConexao'
import { ConfigPerfil } from './ConfigPerfil'
import { ConfigSobre } from './ConfigSobre'
import { ConfigSom } from './ConfigSom'
import { ConfigVideo } from './ConfigVideo'
import './config.css'

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

  const fontes = sources ?? FONTES_VAZIAS
  const linha = { display: 'flex', flexWrap: 'wrap', gap: 20, alignItems: 'stretch' } as const

  return (
    <main
      className="zp-cfg zp-cfg-dots"
      style={{ flex: '999 1 600px', minWidth: 0, display: 'flex', flexDirection: 'column', gap: 20, padding: '28px 32px 36px' }}
    >
      <div style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'flex-end', gap: 16 }}>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
          <span style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: 11, letterSpacing: '0.14em', textTransform: 'uppercase', color: '#3BF08A' }}>
            Ajustes
          </span>
          <h1 style={{ margin: 0, fontFamily: "'Unbounded', sans-serif", fontWeight: 700, fontSize: 32, letterSpacing: '-0.02em' }}>
            Configurações
          </h1>
        </div>
        <span style={{ flex: 1 }} />
        <span
          style={{ height: 32, padding: '0 12px', borderRadius: 999, background: 'rgba(59,240,138,0.1)', color: '#9FE8C0', fontSize: 12.5, display: 'flex', alignItems: 'center', gap: 8 }}
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#3BF08A" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <path d="m5 12 5 5 9-10" />
          </svg>
          {mostrarSalvo ? 'Salvo' : 'Tudo salva sozinho'}
        </span>
      </div>

      {erro && (
        <div role="alert" style={{ color: '#FF6B6B', fontSize: 13 }}>
          {erro}
        </div>
      )}

      {settings === null ? (
        <p style={{ color: '#A3BDB1', margin: 0 }}>Carregando…</p>
      ) : (
        <>
          <div style={linha}>
            <ConfigPerfil settings={settings} onChange={onChange} />
            <ConfigSobre settings={settings} onChange={onChange} />
          </div>
          <ConfigConexao settings={settings} onChange={onChange} network={network} />
          <div style={linha}>
            <ConfigVideo settings={settings} onChange={onChange} sources={fontes} />
            <ConfigSom settings={settings} onChange={onChange} />
          </div>
        </>
      )}
    </main>
  )
}
