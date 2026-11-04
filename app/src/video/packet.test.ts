import { describe, it, expect } from 'vitest'
import { parsePacket, codecFromAnnexB, HEADER_LEN } from './packet'

function makePacket(flags: number, payload: number[]): ArrayBuffer {
  const buf = new ArrayBuffer(HEADER_LEN + payload.length)
  const v = new DataView(buf)
  v.setUint16(0, 7, true)          // track
  v.setUint8(2, 2)                 // layer
  v.setUint8(3, flags)             // flags
  v.setUint16(4, 1280, true)       // width
  v.setUint16(6, 720, true)        // height
  v.setUint32(8, 123456, true)     // seq
  v.setBigUint64(12, 9876543210n, true) // captureUs
  v.setUint32(20, 0, true)         // reservado
  new Uint8Array(buf, HEADER_LEN).set(payload)
  return buf
}

describe('parsePacket', () => {
  it('le todos os campos do cabeçalho e o payload', () => {
    const p = parsePacket(makePacket(1, [1, 2, 3]))
    expect(p.track).toBe(7)
    expect(p.layer).toBe(2)
    expect(p.keyframe).toBe(true)
    expect(p.width).toBe(1280)
    expect(p.height).toBe(720)
    expect(p.seq).toBe(123456)
    expect(p.captureUs).toBe(9876543210n)
    expect(typeof p.captureUs).toBe('bigint')
    expect(p.data.length).toBe(3)
    expect(Array.from(p.data)).toEqual([1, 2, 3])
  })

  it('keyframe false quando bit0 das flags é zero', () => {
    expect(parsePacket(makePacket(0, [1, 2, 3])).keyframe).toBe(false)
    expect(parsePacket(makePacket(2, [1, 2, 3])).keyframe).toBe(false)
  })

  it('lança em buffer curto', () => {
    expect(() => parsePacket(new ArrayBuffer(10))).toThrow('pacote curto')
  })
})

describe('codecFromAnnexB', () => {
  it('extrai avc1 de SPS com start code longo', () => {
    expect(codecFromAnnexB(new Uint8Array([0, 0, 0, 1, 0x67, 0x64, 0x00, 0x28, 0xAC]))).toBe('avc1.640028')
  })

  it('extrai avc1 de SPS com start code curto', () => {
    expect(codecFromAnnexB(new Uint8Array([0, 0, 1, 0x67, 0x42, 0xE0, 0x1E]))).toBe('avc1.42E01E')
  })

  it('retorna null sem SPS', () => {
    expect(codecFromAnnexB(new Uint8Array([0, 0, 0, 1, 0x65, 0x88, 0x84]))).toBeNull()
  })

  it('retorna null com SPS truncado', () => {
    expect(codecFromAnnexB(new Uint8Array([0, 0, 0, 1, 0x67, 0x64, 0x00]))).toBeNull()
    expect(codecFromAnnexB(new Uint8Array([0, 0, 1, 0x67]))).toBeNull()
  })
})
