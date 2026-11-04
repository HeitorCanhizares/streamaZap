import { describe, expect, it } from 'vitest'
import { pingTone } from './pingColor'

describe('pingTone', () => {
  it('faixas de ping', () => {
    expect(pingTone(0)).toBe('green')
    expect(pingTone(59)).toBe('green')
    expect(pingTone(60)).toBe('yellow')
    expect(pingTone(99)).toBe('yellow')
    expect(pingTone(100)).toBe('red')
    expect(pingTone(500)).toBe('red')
  })

  it('NaN vira vermelho', () => {
    expect(pingTone(NaN)).toBe('red')
  })
})
