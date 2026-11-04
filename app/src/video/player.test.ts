import { describe, it, expect, vi, beforeEach } from 'vitest'
import { VideoPlayer, type DecoderInit } from './player'
import { HEADER_LEN } from './packet'

// SPS com profile 0x64, constraints 0x00, level 0x28 -> avc1.640028
const SPS = [0, 0, 0, 1, 0x67, 0x64, 0x00, 0x28, 0xac]

function pacote(keyframe: boolean, layer = 0, payload: number[] = SPS): ArrayBuffer {
  const buf = new ArrayBuffer(HEADER_LEN + payload.length)
  const v = new DataView(buf)
  v.setUint8(2, layer)
  v.setUint8(3, keyframe ? 1 : 0)
  v.setUint16(4, 1280, true)
  v.setUint16(6, 720, true)
  v.setBigUint64(12, 1000n, true)
  new Uint8Array(buf, HEADER_LEN).set(payload)
  return buf
}

class FakeDecoder {
  configure = vi.fn()
  decode = vi.fn()
  close = vi.fn()
}

function frame() {
  return { displayWidth: 640, displayHeight: 360, close: vi.fn() }
}

function montar() {
  const dec = new FakeDecoder()
  let init!: DecoderInit
  const fila: Array<() => void> = []
  const canvas = document.createElement('canvas')
  const ctx = { drawImage: vi.fn() }
  vi.spyOn(canvas, 'getContext').mockReturnValue(ctx as unknown as CanvasRenderingContext2D)
  const player = new VideoPlayer(canvas, {
    decoderFactory: (i) => { init = i; return dec },
    raf: (cb) => { fila.push(cb); return fila.length },
    caf: vi.fn(),
  })
  const saida = (f: ReturnType<typeof frame>) => init.output(f as unknown as VideoFrame)
  const rodarRaf = () => { const cbs = fila.splice(0); cbs.forEach((c) => c()) }
  return { player, dec, ctx, saida, rodarRaf }
}

beforeEach(() => {
  ;(globalThis as unknown as Record<string, unknown>).EncodedVideoChunk = class {
    constructor(public init: unknown) {}
  }
})

describe('VideoPlayer', () => {
  it('descarta delta antes do primeiro keyframe', () => {
    const { player, dec } = montar()
    player.push(pacote(false))
    expect(player.droppedFrames).toBe(1)
    expect(dec.decode).not.toHaveBeenCalled()
  })

  it('keyframe configura o decoder e o delta seguinte e decodificado', () => {
    const { player, dec } = montar()
    player.push(pacote(true))
    expect(dec.configure).toHaveBeenCalledWith(expect.objectContaining({
      codec: 'avc1.640028', hardwareAcceleration: 'prefer-hardware', optimizeForLatency: true,
    }))
    player.push(pacote(false))
    expect(dec.decode).toHaveBeenCalledTimes(2)
  })

  it('mudanca de camada reconfigura; keyframe repetido nao', () => {
    const { player, dec } = montar()
    player.push(pacote(true, 0))
    player.push(pacote(true, 0))
    expect(dec.configure).toHaveBeenCalledTimes(1)
    player.push(pacote(true, 1))
    expect(dec.configure).toHaveBeenCalledTimes(2)
  })

  it('dois frames antes do raf: so o ultimo e desenhado', () => {
    const { player, saida, rodarRaf, ctx } = montar()
    const a = frame(), b = frame()
    saida(a); saida(b)
    expect(a.close).toHaveBeenCalled()
    expect(player.droppedFrames).toBe(1)
    rodarRaf()
    expect(ctx.drawImage).toHaveBeenCalledTimes(1)
    expect(ctx.drawImage).toHaveBeenCalledWith(b, 0, 0)
    expect(player.drawnFrames).toBe(1)
    expect(b.close).toHaveBeenCalled()
  })

  it('decode que lanca descarta deltas ate novo keyframe', () => {
    const { player, dec } = montar()
    player.push(pacote(true))
    dec.decode.mockImplementationOnce(() => { throw new Error('x') })
    player.push(pacote(false))
    const antes = dec.decode.mock.calls.length
    player.push(pacote(false))
    expect(dec.decode.mock.calls.length).toBe(antes)
    expect(player.droppedFrames).toBe(1)
    player.push(pacote(true))
    expect(dec.configure).toHaveBeenCalledTimes(2)
  })

  it('close fecha decoder e frame pendente', () => {
    const { player, dec, saida } = montar()
    const f = frame()
    saida(f)
    player.close()
    expect(f.close).toHaveBeenCalled()
    expect(dec.close).toHaveBeenCalled()
    player.push(pacote(true))
    expect(dec.configure).not.toHaveBeenCalled()
  })
})
