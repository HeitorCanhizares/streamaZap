import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'

vi.mock('../../video/player', () => ({
  VideoPlayer: class { push() {} close() {} },
}))

import { api } from '../../bridge/api'
import { mockSnapshot } from '../../bridge/mock'
import { SalaVoce } from './SalaVoce'

afterEach(() => { cleanup(); vi.restoreAllMocks() })

const room = mockSnapshot().room!

function spies() {
  const watch = vi.spyOn(api, 'watch').mockResolvedValue(undefined as never)
  const unwatch = vi.spyOn(api, 'unwatch').mockResolvedValue(undefined as never)
  return { watch, unwatch }
}

describe('SalaVoce', () => {
  it('renderiza titulo Você e os botoes Câmera e Tela', () => {
    spies()
    render(<SalaVoce room={room} meId="p-ju" camOn={false} sharing={false} />)
    expect(screen.getByText('Você')).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Câmera' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Tela' })).toBeTruthy()
  })

  it('camOn com trilha camera propria chama api.watch com trk-3 e tile 9001', () => {
    const { watch } = spies()
    render(<SalaVoce room={room} meId="p-ju" camOn={true} sharing={false} />)
    expect(watch).toHaveBeenCalledWith('trk-3', 9001, [320, 180], 2, expect.any(Function))
    expect(screen.getByText('sua câmera')).toBeTruthy()
  })

  it('camOn sem trilha propria nao chama watch', () => {
    const { watch } = spies()
    render(<SalaVoce room={room} meId="p-ninguem" camOn={true} sharing={false} />)
    expect(watch).not.toHaveBeenCalled()
  })

  it('desmontar chama api.unwatch', () => {
    const { unwatch } = spies()
    const { unmount } = render(<SalaVoce room={room} meId="p-ju" camOn={true} sharing={false} />)
    unmount()
    expect(unwatch).toHaveBeenCalledWith(9001)
  })

  it('clicar em Câmera chama onToggleCam', () => {
    spies()
    const fn = vi.fn()
    render(<SalaVoce room={room} meId="p-ju" camOn={false} sharing={false} onToggleCam={fn} />)
    fireEvent.click(screen.getByRole('button', { name: 'Câmera' }))
    expect(fn).toHaveBeenCalledTimes(1)
  })

  it('aria-pressed reflete camOn e sharing', () => {
    spies()
    render(<SalaVoce room={room} meId="p-ninguem" camOn={true} sharing={false} />)
    expect(screen.getByRole('button', { name: 'Câmera' }).getAttribute('aria-pressed')).toBe('true')
    expect(screen.getByRole('button', { name: 'Tela' }).getAttribute('aria-pressed')).toBe('false')
  })
})
