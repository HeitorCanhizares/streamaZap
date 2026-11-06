import { describe, expect, it } from 'vitest'
import snap from './fixtures/snapshot_exemplo.json'
import { mockSnapshot } from './mock'
import type { Snapshot } from './types'

// Chaves de topo do Snapshot (espelho de bridge/types.ts).
const CHAVES_DO_SNAPSHOT = ['version', 'me', 'network', 'rooms', 'peers', 'room', 'tiles', 'share', 'errors']

describe('fixture snapshot_exemplo.json', () => {
  it('tem exatamente as chaves de topo do Snapshot', () => {
    expect(Object.keys(snap).sort()).toEqual([...CHAVES_DO_SNAPSHOT].sort())
  })

  it('atribui a Snapshot sem erro de tipo', () => {
    const s: Snapshot = snap as Snapshot
    expect(s.version).toBe(1)
  })

  it('sobrevive a round-trip JSON', () => {
    expect(JSON.parse(JSON.stringify(snap))).toEqual(snap)
  })

  it('mockSnapshot tem as mesmas chaves de topo', () => {
    // `server` e `account` sao opcionais (ausentes quando nao configurados); so o mock os traz.
    const { server: _server, account: _account, ...resto } = mockSnapshot()
    expect(Object.keys(resto).sort()).toEqual(Object.keys(snap).sort())
  })
})
