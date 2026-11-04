// Hook da prévia em baixa resolução de uma sala Online na lista. Tarefa ui-online-02.
import { useCallback, useEffect, useState, type RefCallback } from 'react'
import { api, isTauri } from './api'
import { VideoPlayer } from '../video/player'

// Ids de tile da prévia ficam acima dos tiles reais da sala para nunca colidirem.
let proximoTile = 1_000_000

/**
 * Devolve o ref do canvas da prévia. Com `codigo` e o canvas montados (e dentro do Tauri),
 * cria um VideoPlayer, chama api.roomPreview e, ao trocar de código ou desmontar,
 * api.roomPreviewStop + player.close(). Fora do Tauri não faz nada.
 */
export function useRoomPreview(codigo: string | null): RefCallback<HTMLCanvasElement> {
  const [canvas, setCanvas] = useState<HTMLCanvasElement | null>(null)
  const ref = useCallback((el: HTMLCanvasElement | null) => setCanvas(el), [])

  useEffect(() => {
    if (!canvas || !codigo || !isTauri()) return
    let player: VideoPlayer
    try {
      player = new VideoPlayer(canvas)
    } catch {
      return
    }
    const tile = proximoTile++
    try {
      void Promise.resolve(api.roomPreview(codigo, tile, (b) => player.push(b))).catch(() => {})
    } catch {
      // prévia é opcional: falha não pode quebrar a lista
    }
    return () => {
      try {
        void Promise.resolve(api.roomPreviewStop(tile)).catch(() => {})
      } catch {
        // idem
      }
      player.close()
    }
  }, [canvas, codigo])

  return ref
}
