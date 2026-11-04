// VideoPlayer por tile (tarefa ui-11): decodifica H.264 via WebCodecs e desenha no canvas.
import { parsePacket, codecFromAnnexB } from './packet'

/** Subconjunto do VideoDecoder que usamos (facilita injetar um falso nos testes). */
export interface DecoderLike {
  configure(config: VideoDecoderConfig): void
  decode(chunk: EncodedVideoChunk): void
  close(): void
}
export interface DecoderInit {
  output: (f: VideoFrame) => void
  error: (e: Error) => void
}
export interface PlayerOptions {
  decoderFactory?: (init: DecoderInit) => DecoderLike
  raf?: (cb: () => void) => number
  caf?: (id: number) => void
}

export class VideoPlayer {
  droppedFrames = 0
  drawnFrames = 0
  private decoder: DecoderLike
  private ctx: CanvasRenderingContext2D | null
  private raf: (cb: () => void) => number
  private caf: (id: number) => void
  private needKey = true
  private configKey = ''
  private pending: VideoFrame | null = null
  private rafId: number | null = null
  private closed = false

  constructor(private canvas: HTMLCanvasElement, opcoes: PlayerOptions = {}) {
    this.raf = opcoes.raf ?? ((cb) => requestAnimationFrame(cb))
    this.caf = opcoes.caf ?? ((id) => cancelAnimationFrame(id))
    this.ctx = canvas.getContext('2d')
    const init: DecoderInit = {
      output: (f) => this.onOutput(f),
      // erro do decoder: espera novo keyframe e reconfigura
      error: () => this.resetar(),
    }
    this.decoder = opcoes.decoderFactory ? opcoes.decoderFactory(init) : new VideoDecoder(init)
  }

  private resetar(): void {
    this.needKey = true
    this.configKey = ''
  }

  private onOutput(frame: VideoFrame): void {
    if (this.closed) { frame.close(); return }
    // só o frame mais recente interessa: o anterior ainda pendente é descartado
    if (this.pending) { this.pending.close(); this.droppedFrames++ }
    this.pending = frame
    if (this.rafId === null) this.rafId = this.raf(() => this.desenhar())
  }

  private desenhar(): void {
    this.rafId = null
    const frame = this.pending
    if (!frame) return
    this.pending = null
    if (this.canvas.width !== frame.displayWidth) this.canvas.width = frame.displayWidth
    if (this.canvas.height !== frame.displayHeight) this.canvas.height = frame.displayHeight
    this.ctx?.drawImage(frame, 0, 0)
    frame.close()
    this.drawnFrames++
  }

  push(buf: ArrayBuffer): void {
    if (this.closed) return
    let pkt
    try { pkt = parsePacket(buf) } catch { return }
    if (pkt.keyframe) {
      const codec = codecFromAnnexB(pkt.data)
      if (codec === null) return
      const chave = `${codec}|${pkt.width}x${pkt.height}|${pkt.layer}`
      if (chave !== this.configKey) {
        try {
          this.decoder.configure({
            codec,
            codedWidth: pkt.width,
            codedHeight: pkt.height,
            hardwareAcceleration: 'prefer-hardware',
            optimizeForLatency: true,
          })
        } catch { this.resetar(); return }
        this.configKey = chave
        this.needKey = false
      }
    } else if (this.needKey) {
      this.droppedFrames++
      return
    }
    try {
      this.decoder.decode(new EncodedVideoChunk({
        type: pkt.keyframe ? 'key' : 'delta',
        timestamp: Number(pkt.captureUs),
        data: pkt.data,
      }))
    } catch {
      this.resetar()
    }
  }

  close(): void {
    if (this.closed) return
    this.closed = true
    if (this.rafId !== null) { this.caf(this.rafId); this.rafId = null }
    if (this.pending) { this.pending.close(); this.pending = null }
    try { this.decoder.close() } catch { /* já fechado */ }
  }
}
