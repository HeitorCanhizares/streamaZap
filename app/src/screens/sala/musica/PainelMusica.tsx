// Aba "Música" do painel da sala: colar um link do YouTube, o que está tocando (com o voto para
// pular) e a fila. Quem pôs um item (e o dono da sala) pode tirá-lo; o atual, essas pessoas pulam
// direto. O player fica na tela da sala (BlocoMusica), não aqui.
import { useState, type CSSProperties, type FormEvent, type ReactNode } from 'react'
import { api } from '../../../bridge/api'
import type { MusicItemView, RoomView } from '../../../bridge/types'
import { duracaoTexto, miniatura } from './youtube'

export interface PainelMusicaProps {
  room: RoomView
  meId: string
}

const MONO = "'JetBrains Mono', monospace"
const BOTAO: CSSProperties = {
  height: 34, padding: '0 12px', borderRadius: 10, border: '1px solid #2A443C', background: 'transparent', color: '#E6F4EC',
  fontSize: 12.5, fontWeight: 600, cursor: 'pointer', whiteSpace: 'nowrap',
}
const TEXTO_2: CSSProperties = { fontSize: 12, color: '#8BA498' }

/** "Pedido inválido: esse vídeo…" → "Esse vídeo…". */
export function textoDoErro(e: unknown): string {
  const bruto = e instanceof Error ? e.message : String(e)
  const sem = bruto.replace(/^(Pedido inválido|Não suportado):\s*/, '')
  return sem.charAt(0).toUpperCase() + sem.slice(1)
}

function Linha({ item, podeTirar, onTirar }: { item: MusicItemView; podeTirar: boolean; onTirar: () => void }): ReactNode {
  return (
    <li style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '8px 0', borderTop: '1px solid #1F332D' }}>
      <img src={miniatura(item.video_id)} alt="" width={64} height={36} style={{ flex: '0 0 64px', borderRadius: 6, objectFit: 'cover', background: '#0A100E' }} />
      <span style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column' }}>
        <span style={{ fontSize: 13, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{item.title}</span>
        <span style={{ ...TEXTO_2, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
          {[duracaoTexto(item.duration_ms, item.live), `pôs: ${item.added_by_name}`].filter(Boolean).join(' · ')}
        </span>
      </span>
      {podeTirar && (
        <button type="button" className="zp-x" onClick={onTirar} aria-label={`Tirar ${item.title} da fila`} title="Tirar da fila"
          style={{ ...BOTAO, width: 32, height: 32, padding: 0, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true"><path d="M6 6l12 12M18 6 6 18" /></svg>
        </button>
      )}
    </li>
  )
}

export function PainelMusica({ room, meId }: PainelMusicaProps): ReactNode {
  const [link, setLink] = useState('')
  const [ocupado, setOcupado] = useState(false)
  const [erro, setErro] = useState<string | null>(null)
  const music = room.music

  if (!music) {
    return (
      <div style={{ flex: 1, padding: 20, ...TEXTO_2, fontSize: 13, lineHeight: 1.5 }}>
        {room.mode === 'online'
          ? 'A música da sala ainda não funciona nas salas Online. Por enquanto, use uma sala Local.'
          : 'Quem criou a sala precisa atualizar o streamaZap para a música funcionar.'}
      </div>
    )
  }

  const souDono = !!room.owner_id && room.owner_id === meId
  const pode = (it: MusicItemView): boolean => souDono || it.added_by === meId
  const atual = music.current ?? null
  const votei = !!atual && atual.votes.includes(meId)

  async function fazer(acao: () => Promise<void>): Promise<void> {
    setErro(null)
    try {
      await acao()
    } catch (e) {
      setErro(textoDoErro(e))
    }
  }

  async function adicionar(e: FormEvent): Promise<void> {
    e.preventDefault()
    const texto = link.trim()
    if (!texto || ocupado) return
    setOcupado(true)
    setErro(null)
    try {
      await api.musicAdd(texto)
      setLink('')
    } catch (er) {
      setErro(textoDoErro(er))
    } finally {
      setOcupado(false)
    }
  }

  return (
    <div style={{ flex: 1, minHeight: 0, overflowY: 'auto', display: 'flex', flexDirection: 'column', gap: 14, padding: 16 }}>
      <form onSubmit={(e) => void adicionar(e)} style={{ display: 'flex', gap: 8 }}>
        <label htmlFor="musica-link" style={{ position: 'absolute', width: 1, height: 1, overflow: 'hidden', clip: 'rect(0 0 0 0)' }}>Link do YouTube</label>
        <input
          id="musica-link"
          value={link}
          onChange={(e) => setLink(e.target.value)}
          placeholder="Cole um link do YouTube"
          autoComplete="off"
          style={{ flex: 1, minWidth: 0, height: 38, padding: '0 12px', borderRadius: 10, border: '1px solid #2A443C', background: '#0A100E', color: '#E6F4EC', fontSize: 13 }}
        />
        <button type="submit" disabled={ocupado || !link.trim()} style={{ ...BOTAO, height: 38, border: 'none', background: '#3BF08A', color: '#062014', opacity: ocupado || !link.trim() ? 0.6 : 1 }}>
          {ocupado ? 'Pondo…' : 'Pôr na fila'}
        </button>
      </form>
      {erro && <p role="alert" style={{ margin: 0, fontSize: 12.5, color: '#FF8592' }}>{erro}</p>}

      <section aria-label="Tocando agora" style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
        <h3 style={{ margin: 0, fontSize: 11, fontFamily: MONO, letterSpacing: '0.08em', color: '#3BF08A' }}>TOCANDO AGORA</h3>
        {atual ? (
          <>
            <div style={{ display: 'flex', gap: 10, alignItems: 'center' }}>
              <img src={miniatura(atual.item.video_id)} alt="" width={96} height={54} style={{ flex: '0 0 96px', borderRadius: 8, objectFit: 'cover', background: '#0A100E' }} />
              <span style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', gap: 2 }}>
                <span style={{ fontSize: 13.5, fontWeight: 600, overflow: 'hidden', textOverflow: 'ellipsis', display: '-webkit-box', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical' }}>{atual.item.title}</span>
                <span style={TEXTO_2}>
                  {[atual.item.live ? 'AO VIVO' : '', atual.item.channel ?? '', `pôs: ${atual.item.added_by_name}`].filter(Boolean).join(' · ')}
                </span>
              </span>
            </div>
            <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
              <button
                type="button"
                aria-pressed={votei}
                onClick={() => void fazer(() => api.musicVote(atual.item.id, !votei))}
                style={{ ...BOTAO, borderColor: votei ? '#3BF08A' : '#2A443C', color: votei ? '#3BF08A' : '#E6F4EC' }}
              >
                {`${votei ? 'Votei para pular' : 'Votar para pular'} · ${atual.votes.length}/${atual.needed}`}
              </button>
              {pode(atual.item) && (
                <button type="button" onClick={() => void fazer(() => api.musicRemove(atual.item.id))} style={BOTAO}>
                  Pular agora
                </button>
              )}
            </div>
            <span style={TEXTO_2}>Pula quando metade da sala votar.</span>
          </>
        ) : (
          <span style={{ ...TEXTO_2, fontSize: 13 }}>Nada tocando. Cole o link de uma música, vídeo ou live do YouTube.</span>
        )}
      </section>

      <section aria-label="Fila" style={{ display: 'flex', flexDirection: 'column' }}>
        <h3 style={{ margin: '0 0 6px', fontSize: 11, fontFamily: MONO, letterSpacing: '0.08em', color: '#8BA498' }}>
          {`PRÓXIMAS · ${music.queue.length}`}
        </h3>
        {music.queue.length === 0 ? (
          <span style={TEXTO_2}>A fila está vazia.</span>
        ) : (
          <ol style={{ listStyle: 'none', margin: 0, padding: 0 }}>
            {music.queue.map((it) => (
              <Linha key={it.id} item={it} podeTirar={pode(it)} onTirar={() => void fazer(() => api.musicRemove(it.id))} />
            ))}
          </ol>
        )}
      </section>

      <span style={{ ...TEXTO_2, marginTop: 'auto' }}>
        Cada um toca no próprio app, pelo player do YouTube. Quem pegar um anúncio volta para onde a sala está quando ele acabar.
      </span>
    </div>
  )
}
