// Wrappers tipados de invoke (tarefa ui-08). Fora do Tauri delegam ao mock.
import type * as Core from '@tauri-apps/api/core'
import { mockSettings, mockSources } from './mock'
import type { CreateRoom, JoinTarget, Settings, ShareRequest, Sources, TileId } from './types'

export const isTauri = (): boolean => '__TAURI_INTERNALS__' in globalThis

// Import dinamico: o modulo do Tauri so e carregado dentro do app, para o
// build no navegador (dev/teste) nao quebrar ao resolver os internals.
const core = (): Promise<typeof Core> => import('@tauri-apps/api/core')

// Erros do invoke chegam como string do lado Rust; viram Error com a mesma mensagem.
async function call<T = void>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await core()
  try {
    return args === undefined ? await invoke<T>(cmd) : await invoke<T>(cmd, args)
  } catch (e) {
    throw e instanceof Error ? e : new Error(typeof e === 'string' ? e : String(e))
  }
}

// Estado em memoria do mock: settingsSet nunca muta o objeto anterior, substitui.
let mockState: Settings = mockSettings()

const isPlain = (v: unknown): v is Record<string, unknown> =>
  typeof v === 'object' && v !== null && !Array.isArray(v)

// Merge superficial por secao: cada secao do patch e mesclada com a atual, um nivel so.
function mergeSecoes(atual: Settings, patch: Partial<Settings>): Settings {
  const out: Record<string, unknown> = { ...atual }
  for (const [k, v] of Object.entries(patch)) {
    if (v === undefined) continue
    const antes = out[k]
    out[k] = isPlain(v) && isPlain(antes) ? { ...antes, ...v } : v
  }
  return out as unknown as Settings
}

export const api = {
  settingsGet: async (): Promise<Settings> => {
    if (!isTauri()) return structuredClone(mockState)
    return call<Settings>('settings_get')
  },

  settingsSet: async (patch: Partial<Settings>): Promise<Settings> => {
    if (!isTauri()) {
      mockState = mergeSecoes(mockState, patch)
      return structuredClone(mockState)
    }
    return call<Settings>('settings_set', { patch })
  },

  sourcesList: async (): Promise<Sources> => {
    if (!isTauri()) return mockSources()
    return call<Sources>('sources_list')
  },

  roomCreate: async (req: CreateRoom): Promise<void> => {
    if (!isTauri()) return
    await call('room_create', { req })
  },

  roomJoin: async (target: JoinTarget, password?: string): Promise<void> => {
    if (!isTauri()) return
    await call('room_join', { target, password: password ?? null })
  },

  roomLeave: async (): Promise<void> => {
    if (!isTauri()) return
    await call('room_leave')
  },

  shareStart: async (req: ShareRequest): Promise<void> => {
    if (!isTauri()) return
    await call('share_start', { req })
  },

  shareStop: async (): Promise<void> => {
    if (!isTauri()) return
    await call('share_stop')
  },

  cameraSet: async (on: boolean): Promise<void> => {
    if (!isTauri()) return
    await call('camera_set', { on })
  },

  watch: async (
    trackId: string,
    tile: TileId,
    size: [number, number],
    maxLayer: number | null,
    onPacket: (b: ArrayBuffer) => void,
  ): Promise<void> => {
    if (!isTauri()) return
    const { Channel } = await core()
    // Pacotes binarios chegam pelo canal; o Rust envia cada um como ArrayBuffer.
    const channel = new Channel<ArrayBuffer>()
    channel.onmessage = onPacket
    await call('watch', { trackId, tile, size, maxLayer, channel })
  },

  unwatch: async (tile: TileId): Promise<void> => {
    if (!isTauri()) return
    await call('unwatch', { tile })
  },

  setVolume: async (participantId: string, volume: number, muted: boolean): Promise<void> => {
    if (!isTauri()) return
    await call('set_volume', { participantId, volume, muted })
  },

  setSmoothing: async (auto: boolean, ms: number): Promise<void> => {
    if (!isTauri()) return
    await call('set_smoothing', { auto, ms })
  },

  chatSend: async (text: string): Promise<void> => {
    if (!isTauri()) return
    await call('chat_send', { text })
  },
}
