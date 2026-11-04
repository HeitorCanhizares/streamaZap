// Decodifica a resposta bruta de source_thumbnail (tarefa ui-ligacao-03).
import type { Thumbnail } from './types'

const CABECALHO = 8
const DIM_MAX = 8192

/** Formato: u32 largura LE, u32 altura LE, depois largura*altura*4 bytes RGBA. Invalido = null. */
export function decodeThumbnail(buf: ArrayBuffer): Thumbnail | null {
  if (buf.byteLength < CABECALHO) return null

  // DataView com little-endian explicito: o formato da resposta nao depende da plataforma.
  const view = new DataView(buf)
  const width = view.getUint32(0, true)
  const height = view.getUint32(4, true)

  if (width === 0 || height === 0) return null
  if (width > DIM_MAX || height > DIM_MAX) return null

  // Conta em number (nao em u32) para nao estourar; o tamanho tem que bater exato.
  const esperado = width * height * 4
  if (buf.byteLength !== CABECALHO + esperado) return null

  return {
    width,
    height,
    rgba: new Uint8ClampedArray(buf, CABECALHO, esperado),
  }
}
