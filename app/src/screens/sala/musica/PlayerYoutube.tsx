// O player do YouTube da música da sala: um iframe do embed oficial, controlado por postMessage
// (sem o script do YouTube na página do app). A cada segundo confere a posição da sala e alcança
// ela (quem pegou anúncio pula para onde a sala está). Conta ao núcleo a duração, se é live, o
// título, o fim e os erros.
//
// Termos do YouTube: nada do app por cima do player (nem botões, nem avisos). Quem usa o player
// põe os controles e o aviso de anúncio (onTravado) fora da área dele.
import { useCallback, useEffect, useMemo, useRef, type ReactNode } from 'react'
import type { MusicNowView, MusicReport } from '../../../bridge/types'
import { ESTADO, TITULO_PADRAO, decidir, lerMensagem, posicaoDaSala, relatorio, srcDoPlayer, type InfoPlayer } from './youtube'

const ORIGEM_YT = 'https://www.youtube.com'
/** O tempo do vídeo andou pelo menos isso desde o último segundo: está tocando de verdade. */
const ANDOU_S = 0.3

export interface PlayerYoutubeProps {
  atual: MusicNowView
  /** Volume do player (0..100) já com o volume geral da sala aplicado. */
  volume: number
  mudo: boolean
  onReport: (r: MusicReport) => void
  /** Tocando sem o tempo andar (anúncio ou carregando): o pai mostra o aviso fora do player. */
  onTravado?: (travado: boolean) => void
}

export function PlayerYoutube({ atual, volume, mudo, onReport, onTravado }: PlayerYoutubeProps): ReactNode {
  const item = atual.item
  const ref = useRef<HTMLIFrameElement | null>(null)
  const info = useRef<InfoPlayer>({})
  const ultimoTempo = useRef<number | null>(null)
  const contado = useRef<{ duracaoMs?: number; aoVivo?: boolean; titulo?: string; terminou?: boolean; erro?: boolean }>({})
  const travado = useRef(false)
  // Últimas props para os callbacks de mensagem e do relógio, sem refazer os listeners.
  const props = useRef({ atual, volume, mudo, onReport, onTravado })
  props.current = { atual, volume, mudo, onReport, onTravado }

  // O endereço muda só com o item: já começa na posição da sala (em vídeo; live vai ao vivo).
  const src = useMemo(
    () => srcDoPlayer(item.video_id, item.live ? null : posicaoDaSala(props.current.atual, Date.now()), window.location.origin),
    [item.id, item.video_id, item.live],
  )

  const comando = useCallback((func: string, args: unknown[] = []) => {
    ref.current?.contentWindow?.postMessage(JSON.stringify({ event: 'command', func, args, id: 'zap', channel: 'widget' }), ORIGEM_YT)
  }, [])

  const aplicarVolume = useCallback(() => {
    comando('setVolume', [Math.round(props.current.volume)])
    comando(props.current.mudo ? 'mute' : 'unMute')
  }, [comando])

  // Item novo: esquece o que sabia do anterior.
  useEffect(() => {
    info.current = {}
    contado.current = {}
    ultimoTempo.current = null
    if (travado.current) {
      travado.current = false
      props.current.onTravado?.(false)
    }
  }, [item.id])

  useEffect(() => {
    aplicarVolume()
  }, [volume, mudo, aplicarVolume])

  // Player saindo (fechado ou sem música) no meio de um anúncio: o aviso não pode ficar preso.
  useEffect(
    () => () => {
      if (travado.current) {
        travado.current = false
        props.current.onTravado?.(false)
      }
    },
    [],
  )

  // Mensagens do player: junta o estado e conta ao núcleo o que descobriu.
  useEffect(() => {
    const onMsg = (e: MessageEvent) => {
      if (!ref.current || e.source !== ref.current.contentWindow) return
      const novo = lerMensagem(e.data)
      if (!novo) return
      info.current = { ...info.current, ...novo }
      const { item: it } = props.current.atual
      const report = props.current.onReport
      if (novo.pronto) aplicarVolume()
      if (novo.erro !== undefined && !contado.current.erro) {
        contado.current.erro = true
        report({ item_id: it.id, failed: true })
      }
      if (novo.estado === ESTADO.terminou && !contado.current.terminou) {
        contado.current.terminou = true
        report({ item_id: it.id, ended: true })
      }
      const r = relatorio(it.video_id, it.title === TITULO_PADRAO, info.current, contado.current)
      if (r) {
        contado.current = {
          ...contado.current,
          duracaoMs: r.duration_ms ?? contado.current.duracaoMs,
          aoVivo: r.live ?? contado.current.aoVivo,
          titulo: r.title ?? contado.current.titulo,
        }
        report({ item_id: it.id, ...r })
      }
    }
    window.addEventListener('message', onMsg)
    return () => window.removeEventListener('message', onMsg)
  }, [aplicarVolume])

  // A cada segundo: alcança a sala.
  useEffect(() => {
    const id = window.setInterval(() => {
      const a = props.current.atual
      const i = info.current
      const t = i.tempo
      // Sem posição anterior (primeira amostra) não dá para dizer que travou: conta como andando.
      const andou = t !== undefined && (ultimoTempo.current === null || t - ultimoTempo.current >= ANDOU_S)
      if (t !== undefined) ultimoTempo.current = t
      const d = decidir({ live: a.item.live || i.aoVivo === true, posSalaMs: posicaoDaSala(a, Date.now()), info: i, tempoAndou: andou })
      for (const c of d.comandos) comando(c.func, c.args)
      if (d.travado !== travado.current) {
        travado.current = d.travado
        props.current.onTravado?.(d.travado)
      }
    }, 1000)
    return () => window.clearInterval(id)
  }, [comando])

  const onLoad = () => {
    // Pede ao player para mandar o estado dele (infoDelivery) daqui para a frente.
    ref.current?.contentWindow?.postMessage(JSON.stringify({ event: 'listening', id: 'zap', channel: 'widget' }), ORIGEM_YT)
  }

  return (
    <iframe
      ref={ref}
      key={item.id}
      src={src}
      title={`Player do YouTube: ${item.title}`}
      onLoad={onLoad}
      allow="autoplay; encrypted-media; picture-in-picture"
      referrerPolicy="strict-origin-when-cross-origin"
      style={{ position: 'absolute', inset: 0, width: '100%', height: '100%', border: 0, background: '#000' }}
    />
  )
}
