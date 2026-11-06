// Tarefa front-09: Diagnóstico sem o card 'Servidor online' com Online desligado.
import { describe, expect, it } from 'vitest'
import { mockSnapshot } from '../../bridge/mock'
import { mockDiagReport } from '../../bridge/mockDiag'
import { montarChecks } from './diagModel'
import { montarChecksReal } from './diagReal'

const ids = (cs: { id: string }[]) => cs.map((c) => c.id)
const avisos = (cs: { status: string }[]) => cs.filter((c) => c.status === 'warn').length

describe('diagnóstico com Online desligado', () => {
  it('montarChecks sem srv', () => {
    const snap = mockSnapshot()
    expect(ids(montarChecks(snap, false))).not.toContain('srv')
    expect(montarChecks(snap, false)).toHaveLength(4)
    expect(montarChecks(snap, true)).toHaveLength(5)
    expect(ids(montarChecks(snap, true))).toContain('srv')
    expect(ids(montarChecks(snap))).toContain('srv')
  })

  it('montarChecksReal sem srv', () => {
    const snap = mockSnapshot()
    const rel = mockDiagReport()
    expect(ids(montarChecksReal(rel, snap, false))).not.toContain('srv')
    expect(montarChecksReal(rel, snap, false)).toHaveLength(5)
    expect(montarChecksReal(rel, snap, true)).toHaveLength(6)
    expect(ids(montarChecksReal(rel, snap, true))).toContain('srv')
  })

  it('servidor com erro não gera aviso com Online desligado', () => {
    const snap = { ...mockSnapshot(), server: { state: 'error' } } as unknown as ReturnType<typeof mockSnapshot>
    const rel = mockDiagReport()
    expect(avisos(montarChecksReal(rel, snap, false))).toBeLessThanOrEqual(avisos(montarChecksReal(rel, snap, true)))
    expect(avisos(montarChecks(snap, false))).toBeLessThanOrEqual(avisos(montarChecks(snap, true)))
  })
})
