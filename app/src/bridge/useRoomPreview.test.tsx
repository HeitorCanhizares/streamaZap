import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render } from '@testing-library/react'
import { useRoomPreview } from './useRoomPreview'

const m = vi.hoisted(() => ({
  tauri: true,
  start: vi.fn(),
  stop: vi.fn(),
  push: vi.fn(),
  close: vi.fn(),
}))

vi.mock('./api', () => ({
  isTauri: () => m.tauri,
  api: {
    roomPreview: (...a: unknown[]) => m.start(...a),
    roomPreviewStop: (...a: unknown[]) => m.stop(...a),
  },
}))
vi.mock('../video/player', () => ({
  VideoPlayer: class {
    push = m.push
    close = m.close
  },
}))

function Prev({ codigo }: { codigo: string | null }) {
  const ref = useRoomPreview(codigo)
  return <canvas ref={ref} />
}

beforeEach(() => {
  m.tauri = true
  m.start.mockReset().mockResolvedValue(undefined)
  m.stop.mockReset().mockResolvedValue(undefined)
  m.push.mockReset()
  m.close.mockReset()
})

describe('useRoomPreview', () => {
  it('fora do Tauri não chama roomPreview', () => {
    m.tauri = false
    render(<Prev codigo="ZAP-7K2Q" />)
    expect(m.start).not.toHaveBeenCalled()
  })

  it('no Tauri inicia a prévia e repassa pacotes ao player', () => {
    render(<Prev codigo="ZAP-7K2Q" />)
    expect(m.start).toHaveBeenCalledTimes(1)
    const [cod, tile, fn] = m.start.mock.calls[0]
    expect(cod).toBe('ZAP-7K2Q')
    expect(tile).toBeGreaterThanOrEqual(1_000_000)
    const buf = new ArrayBuffer(4)
    fn(buf)
    expect(m.push).toHaveBeenCalledWith(buf)
  })

  it('desmontar para a prévia com o mesmo tile e fecha o player', () => {
    const { unmount } = render(<Prev codigo="ZAP-7K2Q" />)
    const tile = m.start.mock.calls[0][1]
    unmount()
    expect(m.stop).toHaveBeenCalledWith(tile)
    expect(m.close).toHaveBeenCalled()
  })

  it('código null não faz nada', () => {
    render(<Prev codigo={null} />)
    expect(m.start).not.toHaveBeenCalled()
  })

  it('trocar de código para o antigo e inicia o novo com outro tile', () => {
    const { rerender } = render(<Prev codigo="ZAP-AAAA" />)
    const t1 = m.start.mock.calls[0][1]
    rerender(<Prev codigo="ZAP-BBBB" />)
    expect(m.stop).toHaveBeenCalledWith(t1)
    expect(m.start).toHaveBeenCalledTimes(2)
    expect(m.start.mock.calls[1][0]).toBe('ZAP-BBBB')
    expect(m.start.mock.calls[1][1]).not.toBe(t1)
  })

  it('rejeição de roomPreview não quebra a renderização', async () => {
    m.start.mockRejectedValue(new Error('falhou'))
    m.stop.mockRejectedValue(new Error('falhou'))
    const { unmount } = render(<Prev codigo="ZAP-7K2Q" />)
    await Promise.resolve()
    expect(() => unmount()).not.toThrow()
  })
})
