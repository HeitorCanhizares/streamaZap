// Cabeçalho de 24 bytes little-endian: u16 trilha, u8 camada, u8 flags(bit0 keyframe),
// u16 largura, u16 altura, u32 seq, u64 captura_us, u32 reservado; depois H.264 Annex B.
export interface VideoPacket {
  track: number; layer: number; keyframe: boolean; width: number; height: number
  seq: number; captureUs: bigint; data: Uint8Array
}
export const HEADER_LEN = 24

/** Lê o cabeçalho little-endian e devolve o payload sem copiar (view sobre o mesmo buffer). */
export function parsePacket(buf: ArrayBuffer): VideoPacket {
  if (buf.byteLength < HEADER_LEN) throw new Error('pacote curto')
  const view = new DataView(buf)
  return {
    track: view.getUint16(0, true),
    layer: view.getUint8(2),
    keyframe: (view.getUint8(3) & 1) === 1,
    width: view.getUint16(4, true),
    height: view.getUint16(6, true),
    seq: view.getUint32(8, true),
    captureUs: view.getBigUint64(12, true),
    // reservado (u32 @20) é ignorado de propósito; o payload começa em 24
    data: new Uint8Array(buf, HEADER_LEN),
  }
}

/** Extrai o codec "avc1.PPCCLL" do primeiro SPS (NAL tipo 7) de um Annex B; null se não houver. */
export function codecFromAnnexB(data: Uint8Array): string | null {
  for (let i = 0; i + 2 < data.length; i++) {
    // start code 00 00 01 (o de 4 bytes 00 00 00 01 também casa aqui, no offset +1)
    if (data[i] !== 0 || data[i + 1] !== 0 || data[i + 2] !== 1) continue
    const header = i + 3
    if (header >= data.length) return null
    if ((data[header] & 0x1f) !== 7) continue
    // SPS: profile_idc, constraint flags e level_idc vêm logo após o header da NAL
    if (header + 3 >= data.length) return null
    const hex = (b: number) => b.toString(16).padStart(2, '0').toUpperCase()
    return 'avc1.' + hex(data[header + 1]) + hex(data[header + 2]) + hex(data[header + 3])
  }
  return null
}
