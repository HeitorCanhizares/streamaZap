// Espelho do Snapshot do zap-core (snake_case). Tarefa ui-05 completa e refina.
export type RoomMode = 'online' | 'local'
export type Preset = '720p30' | '720p60' | '1080p30' | '1080p60'
export type TileId = number

export interface Me { name: string; color: string; install_id: string }
export interface RoomSharer { name: string; kind: 'Tela' | 'Webcam' | 'Janela'; scene: SceneKind }
export interface RoomPerson { name: string; role: string; ping_ms: number }
// Campos opcionais alimentam a tela Salas (docs/design/1-salas.html); ausentes = valor neutro.
export interface RoomListing {
  room_id: string; name: string; host: string; participants: number; has_password: boolean; mode: RoomMode
  live_title?: string; scene?: SceneKind
  addr?: string; last_message?: string; last_time?: string; unread?: number; typing?: boolean
  ping_ms?: number; quality?: string; bitrate?: string; viewers?: number; version_warn?: string
  cam_scene?: SceneKind; sharers?: RoomSharer[]; people?: RoomPerson[]
}
export interface PeerListing { name: string; version: string; ip: string; receives_you: boolean }
// volume 0..1 e muted: o que EU escolhi para esse participante (set_volume); ausentes = 0,8 e false.
export interface Participant { id: string; name: string; color: string; ping_ms: number; sharing: boolean; camera: boolean; volume?: number; muted?: boolean }
export interface TrackView { id: string; owner_id: string; kind: 'screen' | 'window' | 'camera'; title: string; number: number }
export interface ChatLine { id: string; author_id: string; author: string; color: string; text: string; ts: number }
// code: codigo para convite (ex.: 'ZAP-7K2Q'); ausente na sala Local sem codigo.
export interface RoomView { id: string; name: string; mode: RoomMode; code?: string; participants: Participant[]; tracks: TrackView[]; chat: ChatLine[]; unread: number }
export interface TileView { tile: TileId; track_id: string; layer: number; width: number; height: number; fps: number; kbps: number; ping_ms: number; smoothing_ms: number; reason: string }
export interface ShareView { source: string; audio: string; preset: Preset; encoder: string; layers: number[]; viewers_per_layer: number[]; encode_ms: number }
export interface NetworkStatus { radmin_ip: string | null; interfaces: string[] }
export interface Snapshot {
  version: number
  me: Me
  network: NetworkStatus
  rooms: RoomListing[]
  peers: PeerListing[]
  room: RoomView | null
  tiles: TileView[]
  share: ShareView | null
  errors: string[]
}
export type SceneKind = 'game' | 'race' | 'movie' | 'code' | 'desktop' | 'cama' | 'camb' | 'camc' | 'camd'

export interface Settings {
  name: string; color: string; install_id: string; open_in: RoomMode
  server?: { address: string; password: string; use_relay: boolean }
  local: { interface: string | null; port: number; announce: boolean }
  video: { camera_id: string | null; mirror_preview: boolean; default_preset: Preset; encoder?: string; smoothing_auto: boolean; smoothing_ms: number }
  sound: { initial_volume: number; message_sound: boolean; notify_room_open: boolean; mute_when_minimized: boolean }
  update: { auto: boolean; channel: 'estavel' | 'beta' }
}
export interface SourceItem { id: string; kind: 'screen' | 'window' | 'camera' | 'app'; name: string }
export interface Sources { screens: SourceItem[]; windows: SourceItem[]; cameras: SourceItem[]; apps: SourceItem[]; encoders: string[] }
export interface CreateRoom { name: string; mode: RoomMode; password: string | null }
export interface ShareRequest { source: string; audio: 'none' | 'all' | string[]; preset: Preset; encoder: string }
export type JoinTarget = { room_id: string } | { ip: string; port: number }

// Miniatura de fonte (source_thumbnail): RGBA 8 bits, linhas de cima para baixo.
export interface Thumbnail { width: number; height: number; rgba: Uint8ClampedArray }
