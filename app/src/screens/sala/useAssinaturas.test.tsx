import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render } from '@testing-library/react'

const players = vi.hoisted(() => ({ criados: 0, fechados: 0 }))
vi.mock('../../video/player', () => ({
  VideoPlayer: class {
    constructor() { players.criados++ }
    push() {}
    close() { players.fechados++ }
  },
}))

import { api } from '../../bridge/api'
import type { Stream } from './salaModel'
import { useAssinaturas, type PedidoStream } from './useAssinaturas'

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
  players.criados = 0
  players.fechados = 0
})

const tela: Stream = { trackId: 'a', ownerId: 'p', kind: 'screen', tile: 11 }
const som: Stream = { trackId: 'b', ownerId: 'q', kind: 'audio', tile: 12 }

let canvasDe: ((id: string) => HTMLCanvasElement) | null = null
function Teste({ pedidos, celulas = 1 }: { pedidos: PedidoStream[]; celulas?: number }) {
  canvasDe = useAssinaturas(pedidos, celulas)
  return null
}

describe('useAssinaturas', () => {
  it('assina cada transmissão sozinha: vídeo com player, som sem', () => {
    const watch = vi.spyOn(api, 'watch').mockResolvedValue(undefined)
    render(<Teste pedidos={[{ stream: tela, lugar: 'principal' }, { stream: som, lugar: 'faixa' }]} />)
    expect(watch).toHaveBeenCalledWith('a', 11, [1920, 1080], null, expect.any(Function))
    expect(watch).toHaveBeenCalledWith('b', 12, [320, 180], null, expect.any(Function))
    expect(players.criados).toBe(1)
  })

  it('mudar de lugar só pede outro tamanho no mesmo tile, com o mesmo canvas', () => {
    const watch = vi.spyOn(api, 'watch').mockResolvedValue(undefined)
    const unwatch = vi.spyOn(api, 'unwatch').mockResolvedValue(undefined)
    const { rerender } = render(<Teste pedidos={[{ stream: tela, lugar: 'principal' }]} />)
    const antes = canvasDe?.('a')
    rerender(<Teste pedidos={[{ stream: tela, lugar: 'faixa' }]} />)
    expect(watch).toHaveBeenLastCalledWith('a', 11, [320, 180], 2, expect.any(Function))
    expect(unwatch).not.toHaveBeenCalled()
    expect(players.criados).toBe(1)
    expect(canvasDe?.('a')).toBe(antes)
  })

  it('o mesmo pedido não repete o watch', () => {
    const watch = vi.spyOn(api, 'watch').mockResolvedValue(undefined)
    const { rerender } = render(<Teste pedidos={[{ stream: tela, lugar: 'faixa' }]} />)
    rerender(<Teste pedidos={[{ stream: tela, lugar: 'faixa' }]} />)
    expect(watch).toHaveBeenCalledTimes(1)
  })

  it('fechar (sair da lista) faz unwatch e fecha o player; desmontar solta o resto', () => {
    vi.spyOn(api, 'watch').mockResolvedValue(undefined)
    const unwatch = vi.spyOn(api, 'unwatch').mockResolvedValue(undefined)
    const { rerender, unmount } = render(<Teste pedidos={[{ stream: tela, lugar: 'faixa' }, { stream: som, lugar: 'faixa' }]} />)
    rerender(<Teste pedidos={[{ stream: som, lugar: 'faixa' }]} />)
    expect(unwatch).toHaveBeenCalledWith(11)
    expect(players.fechados).toBe(1)
    unmount()
    expect(unwatch).toHaveBeenCalledWith(12)
  })
})
