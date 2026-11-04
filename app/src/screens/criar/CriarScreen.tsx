// Tarefa ui-18
import { useEffect, useState, type ReactNode } from 'react'
import { api } from '../../bridge/api'
import { useSnapshot } from '../../bridge/store'
import type { Preset, SourceItem, Sources } from '../../bridge/types'
import { Segmented } from '../../components/Segmented'
import { TextField } from '../../components/TextField'
import { CriarFontes, type FonteTab } from './CriarFontes'
import { CriarOpcoes, type SomModo } from './CriarOpcoes'
import { CriarResumo } from './CriarResumo'
import { Button } from '../../components/Button'

export interface CriarScreenProps { onCreated: () => void; onCancel: () => void }

const SOURCES_VAZIO: Sources = { screens: [], windows: [], cameras: [], apps: [], encoders: [] }
const PRESET_PADRAO: Preset = '720p30'

// Lista de fontes de cada aba, na mesma ordem usada pela grade de CriarFontes.
function listaDa(sources: Sources, tab: FonteTab): SourceItem[] {
  if (tab === 'telas') return sources.screens
  if (tab === 'janelas') return sources.windows
  return sources.cameras
}

// Rótulo legível do som escolhido no resumo.
function rotuloSom(sound: SomModo, apps: string[]): string {
  if (sound === 'none') return 'Sem som'
  if (sound === 'all') return 'Todo o PC'
  return `${apps.length} apps`
}

// Tela "Criar sala": monta a sala local e começa a transmissão com as escolhas feitas.
export function CriarScreen({ onCreated, onCancel }: CriarScreenProps): ReactNode {
  const snap = useSnapshot()

  const [sources, setSources] = useState<Sources>(SOURCES_VAZIO)
  const [nomeSala, setNomeSala] = useState(`Sala de ${snap.me.name}`)
  const [acesso, setAcesso] = useState<'aberta' | 'senha'>('aberta')
  const [senha, setSenha] = useState('')

  const [tab, setTab] = useState<FonteTab>('telas')
  const [selected, setSelected] = useState<string | null>(null)
  const [cameraCorner, setCameraCorner] = useState(false)

  const [sound, setSound] = useState<SomModo>('none')
  const [apps, setApps] = useState<string[]>([])
  const [preset, setPreset] = useState<Preset>(PRESET_PADRAO)
  const [encoder, setEncoder] = useState('auto')

  const [busy, setBusy] = useState(false)
  const [erro, setErro] = useState<string | null>(null)

  // Carrega as fontes e a qualidade padrão; sem fonte escolhida, pré-seleciona a primeira da aba.
  useEffect(() => {
    let vivo = true
    api
      .sourcesList()
      .then((s) => {
        if (!vivo) return
        setSources(s)
        setSelected((atual) => atual ?? (listaDa(s, 'telas')[0]?.id ?? null))
      })
      .catch((e: unknown) => {
        if (vivo) setErro(String(e))
      })
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

  // Trocar de aba escolhe o primeiro item da nova aba, para a seleção nunca apontar para outra aba.
  function trocarAba(nova: FonteTab): void {
    setTab(nova)
    setSelected(listaDa(sources, nova)[0]?.id ?? null)
  }

  const todas = [...sources.screens, ...sources.windows, ...sources.cameras]
  const sourceLabel = todas.find((f) => f.id === selected)?.name ?? ''

  async function iniciar(): Promise<void> {
    if (acesso === 'senha' && senha.trim() === '') {
      setErro('Informe a senha')
      return
    }
    if (selected === null) return

    setErro(null)
    setBusy(true)
    try {
      await api.roomCreate({ name: nomeSala, mode: 'local', password: acesso === 'senha' ? senha : null })
      await api.shareStart({ source: selected, audio: sound === 'apps' ? apps : sound, preset, encoder })
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
          <span className="zp-label" style={{ color: 'var(--verde)' }}>Nova sala</span>
          <h1 style={{ margin: 0, fontFamily: 'var(--font-titulo)', fontWeight: 700, fontSize: 32, letterSpacing: '-0.02em' }}>
            Bora transmitir?
          </h1>
        </div>

        <section className="zp-card" style={{ display: 'flex', flexDirection: 'column', gap: 12, padding: 20 }}>
          <h2 style={{ margin: 0, fontSize: 14, fontWeight: 600 }}>Sala</h2>
          <Segmented
            options={[
              { value: 'local', label: 'Local' },
              { value: 'online', label: 'Online · em breve', disabled: true },
            ]}
            value="local"
            onChange={() => {}}
          />
          <TextField value={nomeSala} onChange={setNomeSala} placeholder="Nome da sala" />
          <Segmented
            options={[
              { value: 'aberta', label: 'Aberta' },
              { value: 'senha', label: 'Com senha' },
            ]}
            value={acesso}
            onChange={(v) => setAcesso(v as 'aberta' | 'senha')}
          />
          {acesso === 'senha' && (
            <TextField type="password" value={senha} onChange={setSenha} placeholder="Senha da sala" />
          )}
        </section>

        <CriarFontes
          sources={sources}
          tab={tab}
          onTab={trocarAba}
          selected={selected}
          onSelect={setSelected}
          cameraCorner={cameraCorner}
          onCameraCorner={setCameraCorner}
        />

        <CriarOpcoes
          sources={sources}
          sound={sound}
          onSound={setSound}
          apps={apps}
          onApps={setApps}
          preset={preset}
          onPreset={setPreset}
          encoder={encoder}
          onEncoder={setEncoder}
        />

        {erro !== null && (
          <p role="alert" style={{ margin: 0, color: 'var(--vermelho)', fontSize: 14 }}>
            {erro}
          </p>
        )}

        <div>
          <Button variant="ghost" onClick={onCancel}>Cancelar</Button>
        </div>
      </div>

      <CriarResumo
        roomName={nomeSala}
        sourceLabel={sourceLabel}
        sound={rotuloSom(sound, apps)}
        preset={preset}
        encoder={encoder}
        busy={busy}
        onStart={() => void iniciar()}
      />
    </main>
  )
}
