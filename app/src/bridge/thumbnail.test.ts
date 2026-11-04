import { describe, expect, it } from 'vitest'
import { decodeThumbnail } from './thumbnail'

/** Monta um buffer: cabecalho u32 LE (largura, altura) + bytes de pixel. */
function montar(w: number, h: number, pixels: number): ArrayBuffer {
  const buf = new ArrayBuffer(8 + pixels)
  const view = new DataView(buf)
  view.setUint32(0, w, true)
  view.setUint32(4, h, true)
  new Uint8Array(buf, 8).forEach((_, i, arr) => {
    arr[i] = i + 1
  })
  return buf
}

describe('decodeThumbnail', () => {
  it('decodifica 2x1', () => {
    const buf = montar(2, 1, 2 * 1 * 4)
    const t = decodeThumbnail(buf)
    expect(t).not.toBeNull()
    expect(t!.width).toBe(2)
    expect(t!.height).toBe(1)
    expect(t!.rgba).toBeInstanceOf(Uint8ClampedArray)
    expect(t!.rgba.length).toBe(8)
    expect(Array.from(t!.rgba)).toEqual([1, 2, 3, 4, 5, 6, 7, 8])
  })

  it('buffer menor que 8 bytes = null', () => {
    expect(decodeThumbnail(new ArrayBuffer(7))).toBeNull()
    expect(decodeThumbnail(new ArrayBuffer(0))).toBeNull()
  })

  it('largura zero = null', () => {
    expect(decodeThumbnail(montar(0, 1, 0))).toBeNull()
    expect(decodeThumbnail(montar(0, 1, 4))).toBeNull()
  })

  it('tamanho dos pixels errado = null', () => {
    // 2x1 pede 8 bytes de pixel; sobra ou falta byte.
    expect(decodeThumbnail(montar(2, 1, 7))).toBeNull()
    expect(decodeThumbnail(montar(2, 1, 9))).toBeNull()
  })

  it('dimensao acima de 8192 = null', () => {
    expect(decodeThumbnail(montar(8193, 1, 8193 * 4))).toBeNull()
    expect(decodeThumbnail(montar(1, 8193, 8193 * 4))).toBeNull()
  })
})
