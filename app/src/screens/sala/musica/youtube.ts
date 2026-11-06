// Player do YouTube controlado pelo postMessage do embed (enablejsapi=1), sem carregar o script
// do YouTube na página do app (ele teria acesso ao IPC do Tauri). Funções puras: o componente
// PlayerYoutube só liga isto ao iframe.
import type { MusicNowView } from '../../../bridge/types'

/** Estados do player (os mesmos números da IFrame API). */
export const ESTADO = { naoIniciado: -1, terminou: 0, tocando: 1, pausado: 2, carregando: 3, pronto: 5 } as const

/** Diferença entre o player e a sala que faz buscar a posição da sala. */
export const LIMITE_DESVIO_MS = 2000

/** O que se sabe do player, juntando as mensagens que ele manda. */
export interface InfoPlayer {
  estado?: number
  /** Posição do vídeo em segundos. */
  tempo?: number
  /** Duração em segundos (0 em live). */
  duracao?: number
  videoId?: string
  titulo?: string
  aoVivo?: boolean
  pronto?: boolean
  /** Código de erro do player (100 = removido/privado; 101 e 150 = não deixa tocar fora do YouTube). */
  erro?: number
}

/** Endereço do embed: começa tocando; em vídeo (não live) já começa na posição da sala. */
export function srcDoPlayer(videoId: string, inicioMs: number | null, origem: string): string {
  const p = new URLSearchParams({ enablejsapi: '1', autoplay: '1', playsinline: '1', rel: '0', origin: origem })
  if (inicioMs !== null && inicioMs > 0) p.set('start', String(Math.floor(inicioMs / 1000)))
  return `https://www.youtube.com/embed/${encodeURIComponent(videoId)}?${p.toString()}`
}

/** Posição da sala agora (ms): a do núcleo mais o tempo desde que ele a calculou. */
export function posicaoDaSala(atual: Pick<MusicNowView, 'position_ms' | 'at_ms'>, agoraMs: number): number {
  return atual.position_ms + Math.max(0, agoraMs - atual.at_ms)
}

/** Lê uma mensagem do iframe: o pedaço novo da InfoPlayer, ou null se não for do player. */
export function lerMensagem(data: unknown): InfoPlayer | null {
  let m: unknown = data
  if (typeof data === 'string') {
    try {
      m = JSON.parse(data)
    } catch {
      return null
    }
  }
  if (typeof m !== 'object' || m === null) return null
  const { event, info } = m as { event?: unknown; info?: unknown }
  if (event === 'onReady') return { pronto: true }
  if (event === 'onStateChange' && typeof info === 'number') return { estado: info }
  if (event === 'onError' && typeof info === 'number') return { erro: info }
  if ((event === 'infoDelivery' || event === 'initialDelivery') && typeof info === 'object' && info !== null) {
    const i = info as Record<string, unknown>
    const out: InfoPlayer = {}
    if (typeof i.playerState === 'number') out.estado = i.playerState
    if (typeof i.currentTime === 'number') out.tempo = i.currentTime
    if (typeof i.duration === 'number') out.duracao = i.duration
    const vd = i.videoData
    if (typeof vd === 'object' && vd !== null) {
      const v = vd as Record<string, unknown>
      if (typeof v.video_id === 'string') out.videoId = v.video_id
      if (typeof v.title === 'string' && v.title.trim() !== '') out.titulo = v.title.trim()
      if (typeof v.isLive === 'boolean') out.aoVivo = v.isLive
    }
    return out
  }
  return null
}

/** Um comando para o player (`func` da IFrame API). */
export interface Comando { func: 'playVideo' | 'pauseVideo' | 'seekTo'; args?: unknown[] }

export interface Decisao {
  comandos: Comando[]
  /** Tocando sem o tempo andar: anúncio ou carregando. */
  travado: boolean
}

/**
 * O que fazer a cada segundo para ficar junto da sala. A sala nunca espera ninguém:
 * - live: só garante que está tocando (no ponto ao vivo, sem buscar posição);
 * - antes de começar (posição negativa): espera parado;
 * - parado ou pausado (sem ter terminado): toca e, se estiver longe, busca a posição da sala;
 * - tocando com o tempo andando e longe da sala (> 2 s): busca a posição da sala;
 * - tocando sem o tempo andar (anúncio ou carregando): espera; quando voltar, a regra de cima
 *   alcança a sala.
 */
export function decidir(args: { live: boolean; posSalaMs: number; info: InfoPlayer; tempoAndou: boolean }): Decisao {
  const { live, posSalaMs, info, tempoAndou } = args
  const estado = info.estado ?? ESTADO.naoIniciado
  const tempoMs = (info.tempo ?? 0) * 1000
  const longe = Math.abs(tempoMs - posSalaMs) > LIMITE_DESVIO_MS
  const busca: Comando = { func: 'seekTo', args: [Math.max(0, posSalaMs) / 1000, true] }
  if (estado === ESTADO.terminou) return { comandos: [], travado: false }
  if (live) {
    const parado = estado === ESTADO.pausado || estado === ESTADO.pronto || estado === ESTADO.naoIniciado
    return { comandos: parado ? [{ func: 'playVideo' }] : [], travado: estado === ESTADO.tocando && !tempoAndou }
  }
  if (posSalaMs < 0) {
    return { comandos: estado === ESTADO.tocando ? [{ func: 'pauseVideo' }] : [], travado: false }
  }
  if (estado === ESTADO.pausado || estado === ESTADO.pronto || estado === ESTADO.naoIniciado) {
    return { comandos: longe ? [busca, { func: 'playVideo' }] : [{ func: 'playVideo' }], travado: false }
  }
  if (estado === ESTADO.tocando) {
    if (!tempoAndou) return { comandos: [], travado: true }
    return { comandos: longe ? [busca] : [], travado: false }
  }
  // Carregando: espera.
  return { comandos: [], travado: false }
}

/** O que contar ao núcleo do item (duração, live, título), ou null se nada mudou. */
export function relatorio(
  itemVideoId: string,
  itemTituloPadrao: boolean,
  info: InfoPlayer,
  jaContado: { duracaoMs?: number; aoVivo?: boolean; titulo?: string },
): { duration_ms?: number; live?: boolean; title?: string } | null {
  // Só vale o que é do vídeo da sala (durante um anúncio os dados podem ser de outro vídeo).
  if (info.videoId !== undefined && info.videoId !== itemVideoId) return null
  const out: { duration_ms?: number; live?: boolean; title?: string } = {}
  const duracaoMs = info.duracao !== undefined ? Math.round(info.duracao * 1000) : undefined
  if (duracaoMs !== undefined && duracaoMs >= 1000 && duracaoMs !== jaContado.duracaoMs) out.duration_ms = duracaoMs
  if (info.aoVivo === true && jaContado.aoVivo !== true) out.live = true
  if (itemTituloPadrao && info.titulo && info.titulo !== jaContado.titulo) out.title = info.titulo
  return Object.keys(out).length > 0 ? out : null
}

/** "3:07", "1:02:45"; live = "AO VIVO". */
export function duracaoTexto(ms: number | undefined, live: boolean): string {
  if (live) return 'AO VIVO'
  if (ms === undefined) return ''
  const total = Math.max(0, Math.round(ms / 1000))
  const h = Math.floor(total / 3600)
  const m = Math.floor((total % 3600) / 60)
  const s = total % 60
  const ss = String(s).padStart(2, '0')
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${ss}` : `${m}:${ss}`
}

/** Miniatura pública do vídeo (i.ytimg.com). */
export function miniatura(videoId: string): string {
  return `https://i.ytimg.com/vi/${encodeURIComponent(videoId)}/mqdefault.jpg`
}

/** Título que o núcleo põe enquanto ninguém soube o de verdade. */
export const TITULO_PADRAO = 'Vídeo do YouTube'
