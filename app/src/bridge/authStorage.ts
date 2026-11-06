// Adaptador de storage do supabase-js sobre os comandos auth_store_* (nada de token no localStorage).
// Tarefa front-02.
import { api } from './api'

export interface AuthStorage {
  getItem(key: string): Promise<string | null>
  setItem(key: string, value: string): Promise<void>
  removeItem(key: string): Promise<void>
}

export const authStorage: AuthStorage = {
  getItem: (key) => api.authStoreGet(key),
  setItem: (key, value) => api.authStoreSet(key, value),
  removeItem: (key) => api.authStoreRemove(key),
}
