// Hook da prévia em baixa resolução de uma sala Online na lista. Tarefa ui-online-02.
import type { RefCallback } from 'react'

/**
 * Devolve o ref do canvas da prévia. Com `codigo` e o canvas montados (e dentro do Tauri),
 * cria um VideoPlayer, chama api.roomPreview e, ao trocar de código ou desmontar,
 * api.roomPreviewStop + player.close(). Fora do Tauri não faz nada.
 */
export function useRoomPreview(codigo: string | null): RefCallback<HTMLCanvasElement> {
  void codigo
  throw new Error('tarefa ui-online-02')
}
