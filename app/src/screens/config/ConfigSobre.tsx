// Seção Sobre e atualizações da tela de configurações (porte fiel de docs/design/4-configuracoes.html, linhas 99-118).
import { useState } from 'react'
import type { CSSProperties, ReactNode } from 'react'
import { Select } from '../../components/Select'
import { api } from '../../bridge/api'
import { useSnapshot } from '../../bridge/store'
import { useUpdateState } from '../../bridge/updateStore'
import type { UpdateInfo } from '../../bridge/types'
import type { ConfigSecaoProps } from './ConfigPerfil'

// Versão exibida na seção. Constante local até o app ler a versão do pacote.
const VERSAO = 'v3.1.1'

const CANAIS = [
  { value: 'estavel', label: 'Estável' },
  { value: 'beta', label: 'Beta' },
]

// Interruptor do design: trilho verde quando ligado, bolinha à direita.
function trilho(on: boolean): CSSProperties {
  return {
    flex: '0 0 44px', width: 44, height: 26, borderRadius: 999, padding: 3, display: 'flex',
    justifyContent: on ? 'flex-end' : 'flex-start', background: on ? '#3BF08A' : '#2A443C',
  }
}

function bolinha(on: boolean): CSSProperties {
  return { width: 20, height: 20, borderRadius: '50%', background: on ? '#03140A' : '#A3BDB1' }
}

export function ConfigSobre({ settings, onChange }: ConfigSecaoProps): ReactNode {
  const up = useUpdateState()
  const emRoom = useSnapshot().room !== null
  const [checando, setChecando] = useState(false)
  const [info, setInfo] = useState<UpdateInfo | null>(null)
  const [erroLocal, setErroLocal] = useState<string | null>(null)
  const [aoFechar, setAoFechar] = useState(false)
  const auto = settings.update.auto
  const fase = up.fase

  function msg(e: unknown): string {
    return e instanceof Error ? e.message : String(e)
  }

  async function procurarAtualizacao(): Promise<void> {
    setChecando(true)
    setErroLocal(null)
    try {
      setInfo(await api.updateCheck())
    } catch (e) {
      setErroLocal(msg(e))
    } finally {
      setChecando(false)
    }
  }

  async function executar(fn: () => Promise<void>): Promise<void> {
    setErroLocal(null)
    try {
      await fn()
    } catch (e) {
      setErroLocal(msg(e))
    }
  }

  async function instalarAoFechar(): Promise<void> {
    await executar(async () => {
      await api.updateInstallOnExit()
      setAoFechar(true)
    })
  }

  const erro = fase === 'erro' ? (up.erro ?? 'Falha na atualização') : erroLocal
  const selo: CSSProperties = {
    minHeight: 30, padding: '0 12px', borderRadius: 999, background: '#16241F', color: '#A3BDB1',
    fontSize: 12.5, display: 'flex', alignItems: 'center',
  }
  const seloVerde: CSSProperties = { ...selo, background: 'rgba(59,240,138,0.12)', color: '#3BF08A', fontWeight: 600 }
  const botao: CSSProperties = {
    height: 44, padding: '0 16px', borderRadius: 12, border: '1px solid #2A443C',
    background: 'transparent', color: '#E6F4EC', fontWeight: 600, cursor: 'pointer',
  }

  let estado: ReactNode
  if (checando) {
    estado = <span role="status" style={selo}>Procurando nas Releases…</span>
  } else if (fase === 'baixando') {
    estado = (
      <>
        <span role="status" style={selo}>{`Baixando ${up.versao ?? ''}… ${up.progresso}%`}</span>
        <div style={{ width: '100%', height: 6, borderRadius: 999, background: '#16241F', overflow: 'hidden' }}>
          <div
            role="progressbar" aria-valuenow={up.progresso} aria-valuemin={0} aria-valuemax={100}
            style={{ width: `${up.progresso}%`, height: '100%', background: '#3BF08A' }}
          />
        </div>
      </>
    )
  } else if (fase === 'pronta') {
    estado = (
      <>
        <span style={seloVerde}>
          {aoFechar ? 'Vai instalar ao fechar o app' : `Atualização ${up.versao ?? ''} pronta — instala ao fechar`}
        </span>
        <div style={{ display: 'flex', flexWrap: 'wrap', justifyContent: 'center', gap: 10 }}>
          <button
            type="button" className="zp-ghost" style={botao} disabled={emRoom}
            title={emRoom ? 'Saia da sala para reiniciar' : undefined}
            onClick={() => void executar(() => api.updateInstallNow())}
          >
            Reiniciar agora
          </button>
          <button type="button" className="zp-ghost" style={botao} onClick={() => void instalarAoFechar()}>
            Instalar ao fechar
          </button>
        </div>
        {erroLocal && <span role="alert" style={{ ...selo, color: '#FF8592' }}>{erroLocal}</span>}
      </>
    )
  } else if (fase === 'instalando') {
    estado = <span style={selo}>Instalando…</span>
  } else if (erro) {
    estado = <span role="alert" style={{ ...selo, color: '#FF8592' }}>{erro}</span>
  } else if (info?.disponivel && fase === 'nada') {
    estado = (
      <>
        <span style={seloVerde}>{`Nova versão ${info.versao ?? ''} disponível`}</span>
        <button type="button" className="zp-ghost" style={botao} onClick={() => void executar(() => api.updateDownload())}>
          Baixar
        </button>
      </>
    )
  } else {
    estado = <span style={seloVerde}>Você está na versão mais nova</span>
  }

  return (
    <section
      aria-labelledby="h-sobre"
      style={{
        flex: '1 1 380px', minWidth: 0, display: 'flex', flexDirection: 'column', alignItems: 'center',
        gap: 14, padding: '26px 22px 22px', borderRadius: 18, background: '#0F1916',
        border: '1px solid #1F332D', textAlign: 'center',
      }}
    >
      <h2 id="h-sobre" style={{ position: 'absolute', width: 1, height: 1, overflow: 'hidden', clip: 'rect(0 0 0 0)' }}>
        Sobre e atualizações
      </h2>
      <img
        src="/icone.png"
        alt="Logo do streamaZap"
        style={{
          width: 136, height: 136, borderRadius: 34, display: 'block',
          boxShadow: '0 0 0 1px #2A443C, 0 0 48px rgba(59,240,138,0.4)',
        }}
      />
      <span style={{ fontFamily: "'Unbounded', sans-serif", fontWeight: 700, fontSize: 22 }}>
        <span>streama</span>
        <span style={{ color: '#3BF08A' }}>Zap</span>
      </span>
      <span style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: 12, color: '#A3BDB1' }}>
        {`versão ${VERSAO.replace(/^v/, '')} · protocolo 3 · licença MIT`}
      </span>

      {estado}

      <div style={{ display: 'flex', flexWrap: 'wrap', justifyContent: 'center', gap: 10, width: '100%' }}>
        <button
          type="button"
          className="zp-ghost"
          onClick={() => void procurarAtualizacao()}
          disabled={checando || fase === 'baixando'}
          style={botao}
        >
          Procurar atualização
        </button>
        <a
          href="https://github.com/HeitorCanhizares/streamaZap"
          className="zp-ghost"
          style={{
            height: 44, padding: '0 16px', borderRadius: 12, border: '1px solid #2A443C', color: '#E6F4EC',
            fontWeight: 600, textDecoration: 'none', display: 'flex', alignItems: 'center', gap: 8,
          }}
        >
          <svg
            width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8}
            strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"
          >
            <path d="M9 18l-6-6 6-6M15 6l6 6-6 6" />
          </svg>
          Código no GitHub
        </a>
      </div>

      <button
        type="button"
        role="switch"
        aria-checked={auto}
        onClick={() => onChange({ update: { ...settings.update, auto: !auto } })}
        style={{
          width: '100%', display: 'flex', alignItems: 'center', gap: 12, marginTop: 4, padding: '12px 14px',
          borderRadius: 14, border: '1px solid #1F332D', background: '#0A100E', textAlign: 'left', cursor: 'pointer',
        }}
      >
        <span style={trilho(auto)}>
          <span style={bolinha(auto)} />
        </span>
        <span style={{ fontSize: 13, color: '#A3BDB1', lineHeight: 1.4 }}>
          <b style={{ color: '#E6F4EC', fontWeight: 600 }}>Atualizar sozinho ao abrir</b> — baixa, confere e reabre já atualizado.
        </span>
      </button>

      <div style={{ width: '100%', display: 'flex', flexDirection: 'column', gap: 6, textAlign: 'left' }}>
        <span style={{ fontSize: 12.5, color: '#A3BDB1' }}>Canal de atualização</span>
        <Select
          options={CANAIS}
          value={settings.update.channel}
          onChange={(v) => onChange({ update: { ...settings.update, channel: v as 'estavel' | 'beta' } })}
        />
      </div>
    </section>
  )
}
