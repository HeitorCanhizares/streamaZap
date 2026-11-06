import { act, cleanup, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from './api'
import { mockSettings } from './mock'
import {
  carregarOnlineEnabled,
  getOnlineEnabled,
  resetOnlineEnabled,
  setOnlineEnabled,
  useOnlineEnabled,
} from './onlineStore'

beforeEach(() => {
  resetOnlineEnabled()
})

afterEach(() => {
  cleanup()
  resetOnlineEnabled()
  vi.restoreAllMocks()
})

describe('store do modo Online', () => {
  it('fora do Tauri o valor inicial é true', () => {
    const { result } = renderHook(() => useOnlineEnabled())
    expect(result.current).toBe(true)
    expect(getOnlineEnabled()).toBe(true)
  })

  it('setOnlineEnabled(false) re-renderiza o hook com false', () => {
    const { result } = renderHook(() => useOnlineEnabled())
    act(() => {
      setOnlineEnabled(false)
    })
    expect(result.current).toBe(false)
    expect(getOnlineEnabled()).toBe(false)
  })

  it('setOnlineEnabled com o mesmo valor não re-renderiza', () => {
    let renders = 0
    renderHook(() => {
      renders++
      return useOnlineEnabled()
    })
    const antes = renders
    act(() => {
      setOnlineEnabled(true)
    })
    expect(renders).toBe(antes)
  })

  it('carregarOnlineEnabled aplica server.enabled false', async () => {
    vi.spyOn(api, 'settingsGet').mockResolvedValue({
      ...mockSettings(),
      server: { enabled: false, address: '', password: '', use_relay: true },
    })
    const { result } = renderHook(() => useOnlineEnabled())
    await act(async () => {
      await carregarOnlineEnabled()
    })
    expect(result.current).toBe(false)
  })

  it('carregarOnlineEnabled com server ausente aplica false', async () => {
    const semServer = { ...mockSettings() } as Record<string, unknown>
    delete semServer.server
    vi.spyOn(api, 'settingsGet').mockResolvedValue(semServer as unknown as ReturnType<typeof mockSettings>)
    await act(async () => {
      await carregarOnlineEnabled()
    })
    expect(getOnlineEnabled()).toBe(false)
  })

  it('carregarOnlineEnabled com enabled true aplica true', async () => {
    setOnlineEnabled(false)
    vi.spyOn(api, 'settingsGet').mockResolvedValue({
      ...mockSettings(),
      server: { enabled: true, address: '', password: '', use_relay: true },
    })
    await act(async () => {
      await carregarOnlineEnabled()
    })
    expect(getOnlineEnabled()).toBe(true)
  })

  it('carregarOnlineEnabled com settingsGet rejeitando mantém o valor e não lança', async () => {
    setOnlineEnabled(false)
    vi.spyOn(api, 'settingsGet').mockRejectedValue(new Error('falhou'))
    await expect(carregarOnlineEnabled()).resolves.toBeUndefined()
    expect(getOnlineEnabled()).toBe(false)
  })

  it('resetOnlineEnabled volta a true', () => {
    setOnlineEnabled(false)
    resetOnlineEnabled()
    expect(getOnlineEnabled()).toBe(true)
  })
})
