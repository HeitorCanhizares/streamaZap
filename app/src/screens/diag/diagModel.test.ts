import { describe, it, expect } from 'vitest'
import { mockSnapshot } from '../../bridge/mock'
import type { Snapshot } from '../../bridge/types'
import { montarChecks, calcularVeredito, montarRelatorio } from './diagModel'

const comSnap = (f: (s: Snapshot) => void): Snapshot => {
  const s = mockSnapshot()
  f(s)
  return s
}

describe('montarChecks', () => {
  it('checks_ordem: 5 checagens na ordem srv, net, fw, peers, rooms', () => {
    const ids = montarChecks(mockSnapshot()).map((c) => c.id)
    expect(ids).toEqual(['srv', 'net', 'fw', 'peers', 'rooms'])
  })

  it('net_sem_interfaces: status warn e linha bad', () => {
    const checks = montarChecks(comSnap((s) => { s.network.interfaces = [] }))
    const net = checks.find((c) => c.id === 'net')!
    expect(net.status).toBe('warn')
    expect(net.lines).toContainEqual({ tom: 'bad', text: 'Nenhuma interface de rede detectada' })
  })

  it('net_sem_radmin: linha warn com sub', () => {
    const checks = montarChecks(comSnap((s) => { s.network.radmin_ip = null }))
    const net = checks.find((c) => c.id === 'net')!
    expect(net.status).toBe('warn')
    expect(net.lines).toContainEqual({
      tom: 'warn',
      text: 'Radmin não detectado',
      sub: 'O modo Online continua funcionando sem ele.',
    })
  })

  it('net_ok: interfaces e radmin presentes geram status ok e linha Radmin com IP', () => {
    const net = montarChecks(mockSnapshot()).find((c) => c.id === 'net')!
    expect(net.status).toBe('ok')
    expect(net.lines).toContainEqual({ tom: 'ok', text: 'Radmin VPN · 26.14.7.88' })
    expect(net.lines).toContainEqual({ tom: 'ok', text: 'Ethernet' })
  })

  it('peers_nao_recebe: status warn e badge "não recebe"', () => {
    const peers = montarChecks(mockSnapshot()).find((c) => c.id === 'peers')!
    expect(peers.status).toBe('warn')
    const rafa = peers.lines.find((l) => l.text.startsWith('Rafa'))!
    expect(rafa.tom).toBe('bad')
    expect(rafa.badge).toBe('não recebe')
    expect(rafa.text).toBe('Rafa · 26.3.140.77 · v3.0.0')
  })

  it('peers_vazio_ok: sem peers, linha info e status ok', () => {
    const peers = montarChecks(comSnap((s) => { s.peers = [] })).find((c) => c.id === 'peers')!
    expect(peers.status).toBe('ok')
    expect(peers.lines).toEqual([{ tom: 'info', text: 'Nenhum amigo com o app aberto agora.' }])
  })

  it('rooms_linhas: sub com modo e ID', () => {
    const rooms = montarChecks(mockSnapshot()).find((c) => c.id === 'rooms')!
    expect(rooms.status).toBe('ok')
    expect(rooms.lines[0]).toEqual({ tom: 'ok', text: 'Sexta do Rush — Marcão', sub: 'online · zap-7k2q' })
  })

  it('rooms_vazio: linha info e status ok', () => {
    const rooms = montarChecks(comSnap((s) => { s.rooms = [] })).find((c) => c.id === 'rooms')!
    expect(rooms.status).toBe('ok')
    expect(rooms.lines).toEqual([{ tom: 'info', text: 'Nenhuma sala descoberta ainda.' }])
  })

  it('soon_nao_conta: srv e fw ficam soon e não entram em avisos', () => {
    const checks = montarChecks(mockSnapshot())
    expect(checks.find((c) => c.id === 'srv')!.status).toBe('soon')
    expect(checks.find((c) => c.id === 'fw')!.status).toBe('soon')
    const v = calcularVeredito(checks, false, false)
    expect(v.avisos).toBe(1) // só Amigos (Rafa não recebe)
  })
})

describe('calcularVeredito', () => {
  const okChecks = () => montarChecks(comSnap((s) => {
    s.peers = s.peers.map((p) => ({ ...p, receives_you: true }))
  }))

  it('veredito_ok', () => {
    const v = calcularVeredito(okChecks(), false, false)
    expect(v).toEqual({
      titulo: 'Tudo certo pra transmitir',
      sub: 'Seu PC recebe e envia sem bloqueio.',
      cor: '#3BF08A',
      avisos: 0,
    })
  })

  it('quase_la_singular', () => {
    const checks = montarChecks(comSnap((s) => { s.peers = s.peers.map((p) => ({ ...p, receives_you: true })); s.network.radmin_ip = null }))
    const v = calcularVeredito(checks, false, false)
    expect(v.titulo).toBe('Quase lá · 1 aviso')
    expect(v.sub).toBe('Dá pra entrar nas salas, mas alguns amigos podem não te ver. Veja os itens em amarelo.')
    expect(v.cor).toBe('#FFB547')
    expect(v.avisos).toBe(1)
  })

  it('quase_la_plural', () => {
    const v = calcularVeredito(montarChecks(mockSnapshot()).map((c) => (c.id === 'rooms' ? { ...c, status: 'warn' as const } : c)), false, false)
    expect(v.titulo).toBe('Quase lá · 2 avisos')
    expect(v.avisos).toBe(2)
  })

  it('verificando: textos e cor de verificação', () => {
    const v = calcularVeredito(okChecks(), true, true)
    expect(v).toEqual({
      titulo: 'Verificando sua conexão…',
      sub: 'Olhando servidor, Radmin, firewall e quem está online.',
      cor: '#A3BDB1',
      avisos: 0,
    })
  })

  it('sem_rede: textos e cor de sem rede', () => {
    const v = calcularVeredito(okChecks(), false, true)
    expect(v).toEqual({
      titulo: 'Sem rede',
      sub: 'Nenhuma interface de rede foi detectada. Confira a conexão do PC.',
      cor: '#FF8592',
      avisos: 0,
    })
  })
})

describe('montarRelatorio', () => {
  it('relatorio_contem_titulos: cabeçalho, seções e título do veredito', () => {
    const checks = montarChecks(mockSnapshot())
    const veredito = calcularVeredito(checks, false, false)
    const texto = montarRelatorio(checks, veredito)
    const linhas = texto.split('\n')
    expect(linhas[0]).toBe('streamaZap · diagnóstico')
    expect(linhas[1]).toBe(veredito.titulo)
    expect(linhas[2]).toBe('')
    expect(texto).toContain('## Rede — Tudo certo')
    expect(texto).toContain('## Amigos com o app aberto — Atenção')
    expect(texto).toContain('## Servidor online — em breve')
    expect(texto).toContain('- Rafa · 26.3.140.77 · v3.0.0 (Não recebe nada do seu PC — o bloqueio está no PC dele.) [não recebe]')
    expect(texto).toContain('- Radmin VPN · 26.14.7.88')
  })
})
