import { useCallback, useEffect, useState, type ReactNode } from 'react'
import { api } from '../../bridge/api'
import { useSnapshot } from '../../bridge/store'
import type { Preset, RoomMode, SourceItem, Sources } from '../../bridge/types'
import { CriarFontes, type FonteTab } from './CriarFontes'
import { CriarModo } from './CriarModo'
import { CriarNomeAcesso } from './CriarNomeAcesso'
import { CriarQualidade } from './CriarQualidade'
import { CriarResumo } from './CriarResumo'
import { CriarSom } from './CriarSom'
import { cenaDaFonte, modoHint, qualidadeTexto, somTexto, type Acesso, type SomModo } from './criarModel'
import './criar.css'

export interface CriarScreenProps { onCreated: () => void; onCancel: () => void }

const SOURCES_VAZIO: Sources = { screens: [], windows: [], cameras: [], apps: [], encoders: [] }

// Lista de fontes de cada aba, na mesma ordem usada pela grade de CriarFontes.
function listaDa(sources: Sources, tab: FonteTab): SourceItem[] {
  if (tab === 'telas') return sources.screens
  if (tab === 'janelas') return sources.windows
  return sources.cameras
}

// Tela "Criar sala": monta a sala e começa a transmissão com as escolhas feitas.
export function CriarScreen({ onCreated, onCancel }: CriarScreenProps): ReactNode {
  const snap = useSnapshot()

  const [sources, setSources] = useState<Sources>(SOURCES_VAZIO)
  const [modo, setModo] = useState<RoomMode>('local')
  const [nome, setNome] = useState(`Sala de ${snap.me.name}`)
  const [acesso, setAcesso] = useState<Acesso>('aberta')
  const [senha, setSenha] = useState('')
  const [tab, setTab] = useState<FonteTab>('telas')
  const [selected, setSelected] = useState<string | null>(null)
  const [cameraCorner, setCameraCorner] = useState(false)
  const [sound, setSound] = useState<SomModo>('none')
  const [marcados, setMarcados] = useState<string[]>([])
  const [preset, setPreset] = useState<Preset>('720p30')
  const [encoder, setEncoder] = useState('auto')
  const [adaptavel, setAdaptavel] = useState(true)
  const [busy, setBusy] = useState(false)
  const [erro, setErro] = useState<string | null>(null)

  // Recarrega as fontes; se a selecionada sumiu, escolhe a primeira da aba atual.
  const carregar = useCallback(async (): Promise<void> => {
    try {
      const s = await api.sourcesList()
      setSources(s)
      setSelected((atual) => {
        const lista = listaDa(s, tab)
        if (atual !== null && [...s.screens, ...s.windows, ...s.cameras].some((f) => f.id === atual)) return atual
        return lista[0]?.id ?? null
      })
    } catch (e) {
      setErro(String(e))
    }
  }, [tab])

  useEffect(() => {
    void carregar()
    // Só na montagem: trocar de aba não deve recarregar as fontes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  useEffect(() => {
    let vivo = true
    api
      .settingsGet()
      .then((cfg) => {
        if (vivo) setPreset(cfg.video.default_preset)
      })
      .catch(() => {
        // Sem configuração: fica com a qualidade padrão.
      })
    return () => {
      vivo = false
    }
  }, [])

  // Trocar de aba escolhe o primeiro item da nova aba.
  function trocarAba(nova: FonteTab): void {
    setTab(nova)
    setSelected(listaDa(sources, nova)[0]?.id ?? null)
  }

  const lista = listaDa(sources, tab)
  const indice = lista.findIndex((f) => f.id === selected)
  const fonte = indice >= 0 ? lista[indice].name : ''
  const nomesApps = sources.apps.filter((a) => marcados.includes(a.id)).map((a) => a.name)

  async function iniciar(): Promise<void> {
    if (acesso === 'senha' && senha.trim() === '') {
      setErro('Informe a senha')
      return
    }
    if (selected === null) return

    setErro(null)
    setBusy(true)
    try {
      await api.roomCreate({ name: nome, mode: modo, password: acesso === 'senha' ? senha : null })
      await api.shareStart({ source: selected, audio: sound === 'apps' ? marcados : sound, preset, encoder })
      onCreated()
    } catch (e) {
      setErro(String(e))
    } finally {
      setBusy(false)
    }
  }

  return (
    <main
      className="zp-dots"
      style={{ display: 'flex', flexWrap: 'wrap', gap: 28, padding: '30px 32px 32px', alignItems: 'flex-start' }}
    >
      <div style={{ flex: '999 1 560px', minWidth: 0, display: 'flex', flexDirection: 'column', gap: 22 }}>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
          <span
            style={{
              fontFamily: 'var(--font-mono)',
              fontSize: 11,
              letterSpacing: '0.14em',
              textTransform: 'uppercase',
              color: 'var(--verde)',
            }}
          >
            Nova sala
          </span>
          <h1 style={{ margin: 0, fontFamily: 'var(--font-titulo)', fontWeight: 700, fontSize: 32, letterSpacing: '-0.02em' }}>
            Bora transmitir?
          </h1>
          <p style={{ margin: 0, color: 'var(--texto-2)', fontSize: 15 }}>{modoHint(modo)}</p>
        </div>

        <CriarModo modo={modo} onModo={setModo} />
        <CriarNomeAcesso nome={nome} onNome={setNome} acesso={acesso} onAcesso={setAcesso} senha={senha} onSenha={setSenha} />
        <CriarFontes
          sources={sources}
          tab={tab}
          onTab={trocarAba}
          selected={selected}
          onSelect={setSelected}
          cameraCorner={cameraCorner}
          onCameraCorner={setCameraCorner}
          onRefresh={() => void carregar()}
        />

        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 18 }}>
          <CriarSom apps={sources.apps} sound={sound} onSound={setSound} marcados={marcados} onMarcados={setMarcados} />
          <CriarQualidade
            encoders={sources.encoders}
            preset={preset}
            onPreset={setPreset}
            encoder={encoder}
            onEncoder={setEncoder}
            adaptavel={adaptavel}
            onAdaptavel={setAdaptavel}
          />
        </div>

        {erro !== null && (
          <p role="alert" style={{ margin: 0, color: 'var(--vermelho)', fontSize: 14 }}>
            {erro}
          </p>
        )}

        <div style={{ display: 'flex', flexWrap: 'wrap', justifyContent: 'flex-end', gap: 12, paddingTop: 4 }}>
          <button
            type="button"
            className="zp-criar-ghost"
            onClick={onCancel}
            style={{
              height: 52,
              padding: '0 22px',
              borderRadius: 14,
              border: '1px solid var(--borda-forte)',
              color: 'var(--texto-2)',
              fontWeight: 600,
              background: 'transparent',
              cursor: 'pointer',
            }}
          >
            Cancelar
          </button>
          <button
            type="button"
            className="zp-criar-cta"
            disabled={busy || selected === null}
            onClick={() => void iniciar()}
            style={{
              height: 52,
              padding: '0 26px',
              borderRadius: 14,
              background: 'var(--verde)',
              color: 'var(--verde-texto)',
              fontWeight: 700,
              fontSize: 16,
              border: 'none',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              gap: 10,
              boxShadow: '0 0 24px rgba(59,240,138,0.25)',
              cursor: 'pointer',
            }}
          >
            <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
              <path d="M13.6 2 4.5 14h6.6l-1.9 8 9.3-12.3h-6.7z" />
            </svg>
            {busy ? 'Iniciando…' : 'Iniciar transmissão'}
          </button>
        </div>
      </div>

      <CriarResumo
        modo={modo}
        nome={nome}
        acesso={acesso}
        fonte={fonte}
        cena={cenaDaFonte(tab, Math.max(indice, 0))}
        mostrarPip={cameraCorner && tab !== 'webcam'}
        som={somTexto(sound, nomesApps)}
        qualidade={qualidadeTexto(preset, adaptavel)}
        radminIp={snap.network.radmin_ip}
      />
    </main>
  )
}
