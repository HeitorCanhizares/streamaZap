import { describe, expect, it } from 'vitest'
import { mockDiagReport } from '../../bridge/mockDiag'
import { mockSnapshot } from '../../bridge/mock'
import { montarChecks } from './diagModel'
import { montarChecksReal } from './diagReal'
import type { Snapshot } from '../../bridge/types'

const get = (cs: ReturnType<typeof montarChecksReal>, id: string) => cs.find((c) => c.id === id)!

// O mockSnapshot não traz `server`; o teste do card srv monta o estado explicitamente.
const comServidor = (server: Snapshot['server']): Snapshot => {
  const s = mockSnapshot()
  s.server = server
  return s
}

describe('checagem de vídeo', () => {
  const video = (rel: ReturnType<typeof mockDiagReport>) => get(montarChecksReal(rel, mockSnapshot()), 'video')

  it('placa com Video Processor: ok, conversão na GPU', () => {
    const v = video(mockDiagReport())
    expect(v.status).toBe('ok')
    expect(v.lines).toEqual([{ tom: 'ok', text: 'NVIDIA GeForce GTX 1060 6GB', badge: 'conversão na GPU' }])
  })
  it('placa sem Video Processor: aviso, conversão na CPU', () => {
    const r = mockDiagReport()
    r.video = { adapters: [{ name: 'Microsoft Basic Render Driver', video_processor: false }] }
    const v = video(r)
    expect(v.status).toBe('warn')
    expect(v.lines[0].tom).toBe('warn')
    expect(v.lines[0].badge).toBe('conversão na CPU')
    expect(v.lines[0].sub).toMatch(/usa mais processador/)
  })
  it('transmitindo: linha com codificador e conversor por camada', () => {
    const r = mockDiagReport()
    r.video.sharing = {
      encoder: 'NVIDIA H.264 Encoder MFT',
      conversion: [{ layer: 0, converter: 'gpu' }, { layer: 1, converter: 'cpu' }],
    }
    const v = video(r)
    const linha = v.lines[v.lines.length - 1]
    expect(linha.text).toBe('Transmitindo · NVIDIA H.264 Encoder MFT')
    expect(linha.sub).toBe('camada 0 na GPU · camada 1 na CPU')
    expect(linha.tom).toBe('warn')
    expect(v.status).toBe('warn')
  })
  it('transmitindo sem espectador: nenhuma camada ainda', () => {
    const r = mockDiagReport()
    r.video.sharing = { encoder: 'x', conversion: [] }
    const v = video(r)
    expect(v.lines[v.lines.length - 1].sub).toBe('nenhuma camada codificando ainda')
    expect(v.status).toBe('ok')
  })
  it('sem placas (fora do Windows): informativo', () => {
    const r = mockDiagReport()
    r.video = { adapters: [] }
    const v = video(r)
    expect(v.status).toBe('ok')
    expect(v.lines).toEqual([{ tom: 'info', text: 'Nenhuma placa de vídeo encontrada' }])
  })
})

describe('montarChecksReal', () => {
  it('rel null cai no montarChecks', () => {
    const snap = mockSnapshot()
    expect(montarChecksReal(null, snap)).toEqual(montarChecks(snap))
  })
  it('ordem dos ids e rede ok', () => {
    const cs = montarChecksReal(mockDiagReport(), mockSnapshot())
    expect(cs.map((c) => c.id)).toEqual(['srv', 'net', 'fw', 'peers', 'rooms', 'video'])
    const net = get(cs, 'net')
    expect(net.lines.some((l) => l.text === 'Radmin VPN · 26.100.5.20')).toBe(true)
    expect(net.status).toBe('ok')
  })
  it('srv com servidor online: ok e linha Conectado', () => {
    const snap = comServidor({ state: 'online', name: 'Casa', ping_ms: 23.6 })
    const srv = get(montarChecksReal(mockDiagReport(), snap), 'srv')
    expect(srv.status).toBe('ok')
    expect(srv.lines[0].text).toMatch(/^Conectado/)
    expect(srv.lines[0].text).toBe('Conectado a Casa')
  })
  it('srv sem servidor: soon com Servidor não configurado', () => {
    const srv = get(montarChecksReal(mockDiagReport(), comServidor(undefined)), 'srv')
    expect(srv.status).toBe('soon')
    expect(srv.lines[0].text).toBe('Servidor não configurado')
  })
  it('firewall bloqueado', () => {
    const r = mockDiagReport()
    r.firewall.bloqueado = true
    const fw = get(montarChecksReal(r, mockSnapshot()), 'fw')
    expect(fw.status).toBe('warn')
    expect(fw.lines.some((l) => l.tom === 'bad')).toBe(true)
  })
  it('vpn deixa rede em aviso', () => {
    const r = mockDiagReport()
    r.vpns = ['NordVPN']
    const net = get(montarChecksReal(r, mockSnapshot()), 'net')
    expect(net.status).toBe('warn')
    expect(net.lines.some((l) => l.text === 'VPN ativa: NordVPN')).toBe(true)
  })
  it('amigo que não ouve', () => {
    const r = mockDiagReport()
    r.friends[0].hears_you = false
    const p = get(montarChecksReal(r, mockSnapshot()), 'peers')
    expect(p.status).toBe('warn')
    expect(p.lines[0].badge).toBe('não recebe')
  })
  it('protocolo mais novo', () => {
    const r = mockDiagReport()
    r.newer_protocol = 4
    const s = get(montarChecksReal(r, mockSnapshot()), 'rooms')
    expect(s.status).toBe('warn')
    expect(s.lines.some((l) => l.text.includes('protocolo 4'))).toBe(true)
  })
  it('firewall não suportado e com erro', () => {
    const r = mockDiagReport()
    r.firewall.suportado = false
    const a = get(montarChecksReal(r, mockSnapshot()), 'fw')
    expect(a.status).toBe('ok')
    expect(a.lines[0].tom).toBe('info')
    const r2 = mockDiagReport()
    r2.firewall.erro = 'x'
    expect(get(montarChecksReal(r2, mockSnapshot()), 'fw').status).toBe('warn')
  })
})
