// Tarefa criar-07
import { useEffect, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import type { RoomMode, SceneKind } from '../../bridge/types'
import { ScenePreview } from '../../components/ScenePreview'
import { VideoFrame } from '../../components/VideoFrame'
import { acessoTexto, modoTexto, seguroTexto, TEXTO_SEM_TRANSMITIR, type Acesso } from './criarModel'
import { conviteTexto, type BarraServidor } from '../../bridge/serverModel'

export interface CriarResumoProps {
  modo: RoomMode
  nome: string
  acesso: Acesso
  fonte: string
  cena: SceneKind
  mostrarPip: boolean
  som: string
  qualidade: string
  radminIp: string | null
  /** Código da sala no modo online; null enquanto a sala não foi criada no servidor. */
  codigo?: string | null
  /** Status do servidor para a frase de apoio do modo online. */
  servidor?: BarraServidor | null
  /** "Nada por enquanto" (B1): o resumo diz "Você entra na sala sem transmitir" e esconde som e vídeo. */
  semTransmitir?: boolean
}

// Endereço da rede local ainda não vem do backend.
const IP_LOCAL = '192.168.0.12:47800'
// Antes da sala existir no servidor, o código aparece mascarado.
const CODIGO_PENDENTE = 'ZAP-····'

const MONO = "'JetBrains Mono', monospace"

function Linha({ rotulo, children, estilo }: { rotulo: string; children: ReactNode; estilo?: React.CSSProperties }): ReactNode {
  return (
    <>
      <dt style={{ color: 'var(--texto-3)' }}>{rotulo}</dt>
      <dd style={{ margin: 0, ...estilo }}>{children}</dd>
    </>
  )
}

function Endereco({ valor, rotulo }: { valor: string; rotulo: string }): ReactNode {
  return (
    <span style={{ display: 'flex', justifyContent: 'space-between', gap: 10, fontFamily: MONO, fontSize: 13 }}>
      <span>{valor}</span>
      <span style={{ color: 'var(--texto-3)' }}>{rotulo}</span>
    </span>
  )
}

// Painel direito: prévia, ficha da sala e código/endereços (2-criar-sala.html 200-239).
export function CriarResumo({ modo, nome, acesso, fonte, cena, mostrarPip, som, qualidade, radminIp, codigo = null, servidor = null, semTransmitir = false }: CriarResumoProps): ReactNode {
  const [copiado, setCopiado] = useState(false)
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)
  useEffect(() => () => { if (timer.current) clearTimeout(timer.current) }, [])

  const online = modo === 'online'
  const enderecos = [...(radminIp ? [`${radminIp}:47800`] : []), IP_LOCAL]

  function copiar(): void {
    let texto: string
    if (online) {
      if (!codigo) return
      texto = conviteTexto(codigo, nome)
    } else {
      texto = enderecos.join('\n')
    }
    try {
      void navigator.clipboard?.writeText(texto)?.catch(() => undefined)
    } catch {
      // Sem permissão de área de transferência: só segue o feedback visual.
    }
    setCopiado(true)
    if (timer.current) clearTimeout(timer.current)
    timer.current = setTimeout(() => setCopiado(false), 1500)
  }

  return (
    <aside
      aria-label="Prévia da transmissão"
      style={{ flex: '1 1 380px', maxWidth: 480, minWidth: 0, display: 'flex', flexDirection: 'column', gap: 16, padding: 20, borderRadius: 20, background: 'var(--superficie)', border: '1px solid var(--borda)' }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <span className="zp-blink" style={{ width: 7, height: 7, borderRadius: '50%', background: 'var(--vermelho)' }} />
        <h2 style={{ margin: 0, fontSize: 14, fontWeight: 600 }}>É isso que a galera vai ver</h2>
      </div>

      {semTransmitir ? (
        // Sem transmissão: a prévia vira um bloco vazio do mesmo tamanho.
        <VideoFrame corners={false} radius={14} style={{ background: '#0A100E', border: '1px solid var(--borda)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
          <span style={{ fontSize: 13, color: 'var(--texto-2)' }}>Sem transmissão por enquanto</span>
        </VideoFrame>
      ) : (
        <VideoFrame corners radius={14}>
          <ScenePreview scene={cena} />
          {mostrarPip && (
            <span style={{ position: 'absolute', right: 16, bottom: 16, width: '30%', borderRadius: 10, overflow: 'hidden', border: '2px solid var(--bg)', boxShadow: '0 0 0 1px var(--verde)' }}>
              <ScenePreview scene="camc" />
            </span>
          )}
        </VideoFrame>
      )}

      <dl style={{ margin: 0, display: 'grid', gridTemplateColumns: 'auto 1fr', gap: '10px 16px', fontSize: 13 }}>
        <Linha rotulo="Modo">{modoTexto(modo)}</Linha>
        <Linha rotulo="Sala" estilo={{ fontWeight: 600 }}>{nome.trim() === '' ? 'Sua sala' : nome}</Linha>
        <Linha rotulo="Acesso">{acessoTexto(acesso)}</Linha>
        {!semTransmitir && (
          <>
            <Linha rotulo="Fonte">{fonte}</Linha>
            <Linha rotulo="Som">{som}</Linha>
            <Linha rotulo="Vídeo" estilo={{ fontFamily: MONO, fontSize: 12.5 }}>{qualidade}</Linha>
          </>
        )}
      </dl>
      {semTransmitir && (
        <span style={{ fontSize: 13, color: 'var(--texto)' }}>{TEXTO_SEM_TRANSMITIR}</span>
      )}

      <div style={{ display: 'flex', flexDirection: 'column', gap: 8, padding: 14, borderRadius: 14, background: 'var(--bg)', border: '1px solid var(--borda)' }}>
        {online ? (
          <>
            <span style={{ fontFamily: MONO, fontSize: 11, letterSpacing: '0.14em', textTransform: 'uppercase', color: 'var(--texto-3)' }}>Código da sala</span>
            <span style={{ fontFamily: MONO, fontSize: 28, fontWeight: 600, letterSpacing: '0.12em', color: codigo ? 'var(--verde)' : 'var(--texto-3)' }}>{codigo || CODIGO_PENDENTE}</span>
            <span style={{ fontSize: 12.5, color: 'var(--texto-2)', lineHeight: 1.4 }}>
              Quem tiver o app entra com esse código, de qualquer lugar.{servidor ? ` ${servidor.texto}.` : ''}
            </span>
          </>
        ) : (
          <>
            <span style={{ fontFamily: MONO, fontSize: 11, letterSpacing: '0.14em', textTransform: 'uppercase', color: 'var(--texto-3)' }}>Seus endereços</span>
            {radminIp !== null && <Endereco valor={`${radminIp}:47800`} rotulo="Radmin" />}
            <Endereco valor={IP_LOCAL} rotulo="rede local" />
          </>
        )}
        <button
          type="button"
          className="zp-btn-ghost"
          onClick={copiar}
          disabled={online && !codigo}
          style={{ marginTop: 6, height: 44, borderRadius: 12, border: '1px solid var(--borda-forte)', background: 'transparent', color: 'var(--texto)', fontWeight: 600, display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8, cursor: 'pointer' }}
        >
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" aria-hidden="true"><rect x="8" y="8" width="12" height="12" rx="2" /><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2" /></svg>
          {copiado ? 'Copiado!' : online && !codigo ? 'Disponível ao criar a sala' : 'Copiar convite'}
        </button>
      </div>

      <span style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 12.5, color: 'var(--texto-2)' }}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#3BF08A" strokeWidth="1.8" strokeLinejoin="round" aria-hidden="true"><path d="M12 3 5 6v5c0 5 3 8.5 7 10 4-1.5 7-5 7-10V6z" /><path d="m9 12 2 2 4-4" strokeLinecap="round" /></svg>
        {seguroTexto(modo)}
      </span>
    </aside>
  )
}
