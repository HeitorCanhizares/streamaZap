// Wrappers tipados de invoke (tarefa ui-08). Fora do Tauri delegam ao mock.
import type * as Core from '@tauri-apps/api/core'
import { mockDiagReport, mockDiagText } from './mockDiag'
import { mockSettings, mockSnapshot, mockSources } from './mock'
import { decodeThumbnail } from './thumbnail'
import type { CreateRoom, DiagReport, FirewallResult, UpdateInfo, JoinTarget, ServerTestResult, Settings, ShareRequest, Snapshot, Sources, Thumbnail, TileId } from './types'

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
  snapshotGet: async (): Promise<Snapshot> => {
    if (!isTauri()) return mockSnapshot()
    return call<Snapshot>('snapshot_get')
  },

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

  // Testar conexão com o servidor Online: nunca rejeita; a falha vem em ok=false.
  serverTest: async (address: string, password: string): Promise<ServerTestResult> => {
    if (!isTauri()) {
      if (address.trim() === '') {
        return { ok: false, name: null, ping_ms: null, code: 'invalido', message: 'Informe o endereço do servidor' }
      }
      return { ok: true, name: 'Servidor de teste', ping_ms: 42, code: null, message: 'Conectado' }
    }
    return call<ServerTestResult>('server_test', { address, password })
  },

  // Prévia de uma sala na lista: pacotes de vídeo chegam pelo canal, como em watch.
  roomPreview: async (code: string, tile: TileId, onPacket: (b: ArrayBuffer) => void): Promise<void> => {
    if (!isTauri()) return
    const { Channel } = await core()
    const channel = new Channel<ArrayBuffer>()
    channel.onmessage = onPacket
    await call('room_preview', { code, tile, channel })
  },

  roomPreviewStop: async (tile: TileId): Promise<void> => {
    if (!isTauri()) return
    await call('room_preview_stop', { tile })
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

  shareUpdate: async (req: ShareRequest): Promise<void> => {
    if (!isTauri()) return
    await call('share_update', { req })
  },

  chatTyping: async (typing: boolean): Promise<void> => {
    if (!isTauri()) return
    await call('chat_typing', { typing })
  },

  // Miniatura de uma fonte: o Rust devolve bytes brutos (cabecalho + RGBA), decodificados aqui.
  sourceThumbnail: async (id: string, w: number, h: number): Promise<Thumbnail | null> => {
    if (!isTauri()) return null
    const { invoke } = await core()
    let bruto: unknown
    try {
      bruto = await invoke<ArrayBuffer>('source_thumbnail', { id, w, h })
    } catch (e) {
      throw e instanceof Error ? e : new Error(typeof e === 'string' ? e : String(e))
    }
    // Conforme o transporte, a resposta pode vir como ArrayBuffer, array de numeros ou Uint8Array.
    let buffer: ArrayBuffer
    if (bruto instanceof ArrayBuffer) {
      buffer = bruto
    } else if (bruto instanceof Uint8Array) {
      buffer = bruto.slice().buffer
    } else if (Array.isArray(bruto)) {
      buffer = Uint8Array.from(bruto as number[]).buffer
    } else {
      return null
    }
    return decodeThumbnail(buffer)
  },
  // --- M5: atualização e diagnóstico (tarefa ui5-01 preenche; fora do Tauri usa o mock) ---
  // Fora do Tauri não há atualizador: nada disponível e as ações são no-op.
  updateCheck: async (): Promise<UpdateInfo> => {
    if (!isTauri()) return { disponivel: false, versao: null, notas: null, tamanho: null }
    return call<UpdateInfo>('update_check')
  },
  updateDownload: async (): Promise<void> => {
    if (!isTauri()) return
    await call('update_download')
  },
  updateInstallNow: async (): Promise<void> => {
    if (!isTauri()) return
    await call('update_install_now')
  },
  updateInstallOnExit: async (): Promise<void> => {
    if (!isTauri()) return
    await call('update_install_on_exit')
  },
  diagnosticsRun: async (): Promise<DiagReport> => {
    if (!isTauri()) return mockDiagReport()
    return call<DiagReport>('diagnostics_run')
  },
  diagnosticsText: async (): Promise<string> => {
    if (!isTauri()) return mockDiagText()
    return call<string>('diagnostics_text')
  },
  firewallFix: async (): Promise<FirewallResult> => {
    if (!isTauri()) {
      return { ok: true, mensagem: 'Firewall corrigido (exemplo).', removidas: 1, perfis_liberados: [] }
    }
    return call<FirewallResult>('firewall_fix')
  },
}
