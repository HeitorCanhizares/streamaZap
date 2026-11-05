// Painel "Transmissão" dentro da sala: escolher ou trocar a fonte (tela, janela, webcam ou só o
// som), o som, a qualidade e a câmera, sem sair da sala. Usa as mesmas seções de Criar sala.
import { useEffect, useState, type CSSProperties, type ReactNode } from 'react'
import { api } from '../../bridge/api'
import type { Preset, ShareView, SourceItem, Sources } from '../../bridge/types'
import { CriarFontes } from '../criar/CriarFontes'
import { CriarQualidade } from '../criar/CriarQualidade'
import { CriarSom } from '../criar/CriarSom'
import { abaDaFonte, erroDoPedido, listaDaAba, montarPedido, somDoPedido, type FonteTab, type SomModo } from '../criar/criarModel'

export interface SalaTransmissaoProps {
  /** Transmissão atual (null = ainda não estou transmitindo). */
  share: ShareView | null
  camOn: boolean
  onToggleCamera: () => void
  /** Câmera trocada com ela ligada: o pai desliga e liga de novo. */
  onCameraTrocada: () => void
  onClose: () => void
}

const SOURCES_VAZIO: Sources = { screens: [], windows: [], cameras: [], apps: [], encoders: [] }

const botao = (primario: boolean): CSSProperties => ({
  height: 48, padding: '0 22px', borderRadius: 14, fontWeight: 700, fontSize: 15, cursor: 'pointer', whiteSpace: 'nowrap',
  border: primario ? 'none' : '1px solid var(--borda-forte)',
  background: primario ? 'var(--verde)' : 'transparent',
  color: primario ? 'var(--verde-texto)' : 'var(--texto-2)',
})

export function SalaTransmissao({ share, camOn, onToggleCamera, onCameraTrocada, onClose }: SalaTransmissaoProps): ReactNode {
  const pedido = share?.request
  const inicial = somDoPedido(pedido?.audio ?? 'none')
  const [sources, setSources] = useState<Sources>(SOURCES_VAZIO)
  const [tab, setTab] = useState<FonteTab>(pedido ? abaDaFonte(pedido.source) : 'telas')
  const [selected, setSelected] = useState<string | null>(pedido && pedido.source !== 'none' ? pedido.source : null)
  const [sound, setSound] = useState<SomModo>(inicial.sound)
  const [marcados, setMarcados] = useState<string[]>(inicial.marcados)
  const [preset, setPreset] = useState<Preset>(pedido?.preset ?? share?.preset ?? '720p30')
  const [encoder, setEncoder] = useState(pedido?.encoder ?? 'auto')
  const [adaptavel, setAdaptavel] = useState(true)
  const [cameraId, setCameraId] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [erro, setErro] = useState<string | null>(null)

  const carregar = async (): Promise<void> => {
    try {
      const s = await api.sourcesList()
      setSources(s)
      // Sem fonte escolhida (ou ela sumiu): a primeira da aba.
      setSelected((atual) => {
        const todas: SourceItem[] = [...s.screens, ...s.windows, ...s.cameras]
        if (atual !== null && todas.some((f) => f.id === atual)) return atual
        return listaDaAba(s, tab)[0]?.id ?? null
      })
    } catch (e) {
      setErro(String(e))
    }
  }

  useEffect(() => {
    void carregar()
    let vivo = true
    api
      .settingsGet()
      .then((cfg) => {
        if (!vivo) return
        setCameraId(cfg.video.camera_id)
        if (!pedido) setPreset(cfg.video.default_preset)
      })
      .catch(() => {})
    return () => {
      vivo = false
    }
    // Só ao abrir o painel.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // Esc fecha o painel.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [onClose])

  function trocarAba(nova: FonteTab): void {
    setTab(nova)
    setSelected(listaDaAba(sources, nova)[0]?.id ?? null)
    if (nova === 'audio' && sound === 'none') setSound('all')
  }

  async function trocarCamera(id: string): Promise<void> {
    setCameraId(id)
    try {
      const cfg = await api.settingsGet()
      await api.settingsSet({ video: { ...cfg.video, camera_id: id } })
      if (camOn) onCameraTrocada()
    } catch (e) {
      setErro(String(e))
    }
  }

  async function aplicar(): Promise<void> {
    const falta = erroDoPedido(tab, selected, sound, marcados)
    const novo = montarPedido(tab, selected, sound, marcados, preset, encoder)
    if (falta !== null || novo === null) {
      setErro(falta ?? 'Escolha o que compartilhar.')
      return
    }
    setErro(null)
    setBusy(true)
    try {
      if (share) await api.shareUpdate(novo)
      else await api.shareStart(novo)
      onClose()
    } catch (e) {
      setErro(String(e))
    } finally {
      setBusy(false)
    }
  }

  async function parar(): Promise<void> {
    setBusy(true)
    try {
      await api.shareStop()
      onClose()
    } catch (e) {
      setErro(String(e))
    } finally {
      setBusy(false)
    }
  }

  const cameraAtual = cameraId && sources.cameras.some((c) => c.id === cameraId) ? cameraId : (sources.cameras[0]?.id ?? '')

  return (
    <div
      role="presentation"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose()
      }}
      style={{ position: 'fixed', inset: 0, zIndex: 50, background: 'rgba(3,8,6,0.72)', display: 'flex', alignItems: 'center', justifyContent: 'center', padding: 24 }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="tx-titulo"
        style={{ width: 'min(1040px, 100%)', maxHeight: '100%', overflow: 'auto', display: 'flex', flexDirection: 'column', gap: 18, padding: 24, borderRadius: 22, background: 'var(--bg)', border: '1px solid var(--borda-forte)', boxShadow: '0 24px 80px rgba(0,0,0,0.5)' }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 4, minWidth: 0, flex: 1 }}>
            <h2 id="tx-titulo" style={{ margin: 0, fontFamily: 'var(--font-titulo)', fontSize: 22, fontWeight: 700 }}>
              {share ? 'Sua transmissão' : 'Começar a transmitir'}
            </h2>
            <span style={{ fontSize: 13, color: 'var(--texto-2)' }}>
              {share ? 'Troque a fonte, o som ou a qualidade sem sair da sala.' : 'Escolha o que os outros vão ver e ouvir.'}
            </span>
          </div>
          <button
            type="button"
            className="zp-x"
            onClick={onClose}
            aria-label="Fechar"
            style={{ flex: '0 0 40px', width: 40, height: 40, borderRadius: 12, border: '1px solid var(--borda-forte)', background: 'transparent', color: 'var(--texto)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true"><path d="M6 6l12 12M18 6 6 18" /></svg>
          </button>
        </div>

        <CriarFontes
          sources={sources}
          tab={tab}
          onTab={trocarAba}
          selected={selected}
          onSelect={setSelected}
          cameraCorner={false}
          onCameraCorner={() => {}}
          mostrarCanto={false}
          onRefresh={() => void carregar()}
          numero="01"
        />

        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 18 }}>
          <CriarSom apps={sources.apps} sound={sound} onSound={setSound} marcados={marcados} onMarcados={setMarcados} numero="02" />
          <CriarQualidade
            encoders={sources.encoders}
            preset={preset}
            onPreset={setPreset}
            encoder={encoder}
            onEncoder={setEncoder}
            adaptavel={adaptavel}
            onAdaptavel={setAdaptavel}
            numero="03"
          />
        </div>

        <section aria-label="Câmera" style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 14, padding: '14px 18px', borderRadius: 18, background: 'var(--superficie)', border: '1px solid var(--borda)' }}>
          <button
            type="button"
            role="switch"
            aria-checked={camOn}
            onClick={onToggleCamera}
            style={{ display: 'flex', alignItems: 'center', gap: 12, padding: 0, border: 0, background: 'transparent', color: 'var(--texto)', textAlign: 'left', cursor: 'pointer' }}
          >
            <span style={{ flex: '0 0 44px', width: 44, height: 26, borderRadius: 999, padding: 3, boxSizing: 'border-box', display: 'flex', justifyContent: camOn ? 'flex-end' : 'flex-start', background: camOn ? 'var(--verde)' : 'var(--borda-forte)' }}>
              <span style={{ width: 20, height: 20, borderRadius: '50%', background: camOn ? 'var(--verde-texto)' : 'var(--texto-2)' }} />
            </span>
            <span style={{ fontWeight: 600 }}>Minha câmera</span>
          </button>
          {sources.cameras.length > 0 ? (
            <span style={{ display: 'flex', alignItems: 'center', gap: 10, flex: '1 1 260px', minWidth: 0 }}>
              <label htmlFor="tx-camera" style={{ fontSize: 13, color: 'var(--texto-2)', whiteSpace: 'nowrap' }}>Dispositivo</label>
              <select
                id="tx-camera"
                value={cameraAtual}
                onChange={(e) => void trocarCamera(e.target.value)}
                style={{ flex: 1, minWidth: 0, height: 40, padding: '0 10px', borderRadius: 10, border: '1px solid var(--borda-forte)', background: 'var(--bg)', color: 'var(--texto)' }}
              >
                {sources.cameras.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
              </select>
            </span>
          ) : (
            <span style={{ fontSize: 13, color: 'var(--texto-3)' }}>Nenhuma câmera encontrada</span>
          )}
        </section>

        {erro !== null && (
          <p role="alert" style={{ margin: 0, color: 'var(--vermelho)', fontSize: 14 }}>{erro}</p>
        )}

        <div style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 12 }}>
          {share && (
            <button type="button" disabled={busy} onClick={() => void parar()} style={{ ...botao(false), borderColor: 'var(--vermelho)', color: 'var(--vermelho)' }}>
              Parar transmissão
            </button>
          )}
          <span style={{ flex: 1 }} />
          <button type="button" onClick={onClose} style={botao(false)}>Cancelar</button>
          <button type="button" className="zp-criar-cta" disabled={busy} onClick={() => void aplicar()} style={botao(true)}>
            {busy ? 'Aplicando…' : share ? 'Aplicar' : 'Começar a transmitir'}
          </button>
        </div>
      </div>
    </div>
  )
}
