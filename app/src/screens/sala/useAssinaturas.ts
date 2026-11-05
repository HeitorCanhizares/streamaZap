// Assinaturas das transmissões da sala: uma por trilha, abertas sozinhas quando a trilha aparece.
// O hook é dono do canvas e do VideoPlayer de cada vídeo. Mudar de lugar (principal, faixa, canto)
// só pede outro tamanho ao núcleo (watch de novo no mesmo tile = atualizar) e o canvas muda de
// lugar no DOM (CanvasSlot): o decodificador continua, sem esperar outro keyframe. Fechar ou a
// trilha sumir = unwatch. O som de quem compartilha só o som também é uma assinatura (sem canvas).
import { useCallback, useEffect, useRef } from 'react'
import { api } from '../../bridge/api'
import { VideoPlayer } from '../../video/player'
import { pedidoDoLugar, type Stream } from './salaModel'

export type LugarVisivel = 'principal' | 'faixa' | 'canto'

/** Uma transmissão aberta e onde ela aparece agora. */
export interface PedidoStream { stream: Stream; lugar: LugarVisivel }

interface Entrada { tile: number; player: VideoPlayer | null; chave: string }

const ESTILO_CANVAS: Partial<CSSStyleDeclaration> = {
  position: 'absolute', inset: '0', width: '100%', height: '100%', objectFit: 'contain', display: 'block',
}

/**
 * Abre/atualiza/fecha as assinaturas conforme `pedidos` (as transmissões não fechadas) e devolve
 * `canvasDe(trackId)`, o canvas fixo de cada vídeo. `celulas` = quantas dividem a tela principal
 * (cada uma pede menos quando são várias).
 */
export function useAssinaturas(pedidos: PedidoStream[], celulas: number): (trackId: string) => HTMLCanvasElement {
  const canvases = useRef(new Map<string, HTMLCanvasElement>())
  const entradas = useRef(new Map<string, Entrada>())

  const canvasDe = useCallback((trackId: string): HTMLCanvasElement => {
    let c = canvases.current.get(trackId)
    if (!c) {
      c = document.createElement('canvas')
      Object.assign(c.style, ESTILO_CANVAS)
      canvases.current.set(trackId, c)
    }
    return c
  }, [])

  // Só reage quando muda o que importa (trilha, tile, lugar, divisão da principal).
  const chaveGeral = pedidos.map((p) => `${p.stream.trackId}:${p.stream.tile}:${p.lugar}`).join('|') + `#${celulas}`

  useEffect(() => {
    const mapa = entradas.current
    const vivos = new Set<string>()
    for (const { stream: s, lugar } of pedidos) {
      vivos.add(s.trackId)
      const video = s.kind !== 'audio'
      const { size, maxLayer } = video ? pedidoDoLugar(lugar, celulas, s.mine) : { size: [320, 180] as [number, number], maxLayer: null }
      const chave = `${size[0]}x${size[1]}:${maxLayer ?? '-'}`
      let e = mapa.get(s.trackId)
      if (!e) {
        let player: VideoPlayer | null = null
        if (video) {
          try {
            player = new VideoPlayer(canvasDe(s.trackId))
          } catch {
            player = null
          }
        }
        e = { tile: s.tile, player, chave: '' }
        mapa.set(s.trackId, e)
      }
      if (e.chave !== chave) {
        e.chave = chave
        const alvo = e
        void Promise.resolve(api.watch(s.trackId, s.tile, size, maxLayer, (b) => alvo.player?.push(b))).catch(() => {})
      }
    }
    for (const [id, e] of mapa) {
      if (vivos.has(id)) continue
      void Promise.resolve(api.unwatch(e.tile)).catch(() => {})
      e.player?.close()
      mapa.delete(id)
      // Reaberta depois, ganha um canvas limpo.
      canvases.current.delete(id)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [chaveGeral])

  // Saiu da sala: solta tudo.
  useEffect(() => () => {
    for (const e of entradas.current.values()) {
      void Promise.resolve(api.unwatch(e.tile)).catch(() => {})
      e.player?.close()
    }
    entradas.current.clear()
  }, [])

  return canvasDe
}
