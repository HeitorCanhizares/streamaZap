// Seção Sobre e atualizações da tela de configurações (porte fiel de docs/design/4-configuracoes.html, linhas 99-118).
import { useEffect, useRef, useState } from 'react'
import type { CSSProperties, ReactNode } from 'react'
import { Select } from '../../components/Select'
import type { ConfigSecaoProps } from './ConfigPerfil'

// Versão exibida na seção. Constante local até o app ler a versão do pacote.
const VERSAO = 'v3.0.0-alpha.0'

// Tempo da verificação simulada (o design usa 1200 ms; a busca real nas Releases entra na tarefa de atualização).
const TEMPO_VERIFICACAO_MS = 1200

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
  const [checking, setChecking] = useState(false)
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)
  // Ao desmontar, cancela a verificação pendente para não atualizar estado de tela fechada.
  useEffect(() => () => {
    if (timer.current !== null) clearTimeout(timer.current)
  }, [])

  const upToDate = !checking
  const auto = settings.update.auto

  function procurarAtualizacao(): void {
    if (timer.current !== null) clearTimeout(timer.current)
    setChecking(true)
    timer.current = setTimeout(() => {
      timer.current = null
      setChecking(false)
    }, TEMPO_VERIFICACAO_MS)
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

      {upToDate && (
        <span
          style={{
            height: 30, padding: '0 12px', borderRadius: 999, background: 'rgba(59,240,138,0.12)',
            color: '#3BF08A', fontSize: 12.5, fontWeight: 600, display: 'flex', alignItems: 'center', gap: 6,
          }}
        >
          Você está na versão mais nova
        </span>
      )}
      {checking && (
        <span
          role="status"
          style={{
            height: 30, padding: '0 12px', borderRadius: 999, background: '#16241F', color: '#A3BDB1',
            fontSize: 12.5, display: 'flex', alignItems: 'center',
          }}
        >
          Procurando nas Releases…
        </span>
      )}

      <div style={{ display: 'flex', flexWrap: 'wrap', justifyContent: 'center', gap: 10, width: '100%' }}>
        <button
          type="button"
          className="zp-ghost"
          onClick={procurarAtualizacao}
          style={{
            height: 44, padding: '0 16px', borderRadius: 12, border: '1px solid #2A443C',
            background: 'transparent', color: '#E6F4EC', fontWeight: 600, cursor: 'pointer',
          }}
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
