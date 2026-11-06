import { afterEach, describe, expect, it, vi } from 'vitest'
import { api } from './api'
import { authStorage } from './authStorage'

// Fora do Tauri (ambiente de teste) o api usa o Map em memória do mock: nada vai para o localStorage.
afterEach(async () => {
  vi.restoreAllMocks()
  // Limpa o Map do mock entre os testes.
  await authStorage.removeItem('sb-x-auth-token')
  await authStorage.removeItem('k')
  localStorage.clear()
})

describe('authStorage delega ao api.authStore*', () => {
  it('getItem chama authStoreGet com a mesma chave e devolve o valor', async () => {
    const spy = vi.spyOn(api, 'authStoreGet').mockResolvedValue('token-x')
    await expect(authStorage.getItem('sb-x-auth-token')).resolves.toBe('token-x')
    expect(spy).toHaveBeenCalledWith('sb-x-auth-token')
  })

  it('getItem devolve null quando o api devolve null', async () => {
    const spy = vi.spyOn(api, 'authStoreGet').mockResolvedValue(null)
    await expect(authStorage.getItem('sb-x-auth-token')).resolves.toBeNull()
    expect(spy).toHaveBeenCalledWith('sb-x-auth-token')
  })

  it('setItem chama authStoreSet com chave e valor', async () => {
    const spy = vi.spyOn(api, 'authStoreSet').mockResolvedValue(undefined)
    await authStorage.setItem('k', 'v')
    expect(spy).toHaveBeenCalledWith('k', 'v')
  })

  it('removeItem chama authStoreRemove com a chave', async () => {
    const spy = vi.spyOn(api, 'authStoreRemove').mockResolvedValue(undefined)
    await authStorage.removeItem('k')
    expect(spy).toHaveBeenCalledWith('k')
  })
})

describe('authStorage com o mock em memória (fora do Tauri)', () => {
  it('ida e volta: setItem, getItem, removeItem, getItem', async () => {
    await authStorage.setItem('k', 'v')
    await expect(authStorage.getItem('k')).resolves.toBe('v')
    await authStorage.removeItem('k')
    await expect(authStorage.getItem('k')).resolves.toBeNull()
  })

  it('não toca o localStorage', async () => {
    localStorage.clear()
    await authStorage.setItem('k', 'v')
    expect(localStorage.length).toBe(0)
  })
})
