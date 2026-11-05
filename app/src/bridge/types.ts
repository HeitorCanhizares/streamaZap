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
// kind 'audio': o som de quem compartilha (aparece sozinho quando a pessoa transmite só o som).
export type TrackKind = 'screen' | 'window' | 'camera' | 'audio'
export interface TrackView { id: string; owner_id: string; kind: TrackKind; title: string; number: number }
export interface ChatLine { id: string; author_id: string; author: string; color: string; text: string; ts: number }
// code: codigo para convite (ex.: 'ZAP-7K2Q'); ausente na sala Local sem codigo.
// me_id: meu id de participante nesta sala (no Online vem do servidor, diferente do install_id).
export interface RoomView { id: string; name: string; mode: RoomMode; code?: string; participants: Participant[]; tracks: TrackView[]; chat: ChatLine[]; unread: number; me_id?: string }
export interface TileView { tile: TileId; track_id: string; layer: number; width: number; height: number; fps: number; kbps: number; ping_ms: number; smoothing_ms: number; reason: string }
/** Onde a cor é convertida para NV12: Video Processor da GPU ou reserva na CPU. */
export type ConverterView = 'gpu' | 'cpu'
export interface LayerConversion { layer: number; converter: ConverterView }
// request: o pedido em vigor (a sala abre o seletor com ele). source 'none' = só o som.
export interface ShareView { source: string; audio: string; preset: Preset; encoder: string; layers: number[]; viewers_per_layer: number[]; encode_ms: number; conversion: LayerConversion[]; request?: ShareRequest }
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
  /** Protocolo maior visto na rede (amigo/servidor mais novo): oferecer atualizar. */
  newer_protocol?: number
  /** Estado do servidor Online (barra); ausente = não configurado. */
  server?: ServerStatus
}
export interface ServerStatus { state: 'off' | 'connecting' | 'online' | 'error'; name?: string; ping_ms?: number; message?: string }
export interface ServerTestResult { ok: boolean; name: string | null; ping_ms: number | null; code: string | null; message: string }
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
// source: id da fonte (screen:N, window:H, camera:ID) ou SO_AUDIO ('none') para transmitir só o som.
export interface ShareRequest { source: string; audio: 'none' | 'all' | string[]; preset: Preset; encoder: string }
export const SO_AUDIO = 'none'
export type JoinTarget = { room_id: string } | { ip: string; port: number } | { code: string }

// Miniatura de fonte (source_thumbnail): RGBA 8 bits, linhas de cima para baixo.
export interface Thumbnail { width: number; height: number; rgba: Uint8ClampedArray }

// --- M5: atualização e diagnóstico (espelham app/src-tauri/src/updater.rs e zap-core/src/diag.rs) ---
export type UpdatePhase = 'nada' | 'baixando' | 'pronta' | 'instalando' | 'erro'
/** Payload do evento `zap://update`. */
export interface UpdateState { fase: UpdatePhase; progresso: number; versao: string | null; erro: string | null }
/** Resposta de `update_check`. */
export interface UpdateInfo { disponivel: boolean; versao: string | null; notas: string | null; tamanho: number | null }

export type DiagSeverity = 'info' | 'warn' | 'error'
export interface DiagProblem { severity: DiagSeverity; code: string; message: string }
export interface DiagInterface { name: string; ip: string; broadcast?: string; radmin: boolean }
export interface DiagFriend { name: string; version: string; ip: string; hears_you: boolean; same_version: boolean }
export interface DiagRoom { id: string; name: string; host: string; participants: number; has_password: boolean; addr?: string; version_warn?: string }
export interface DiagCurrentRoom { name: string; mode: RoomMode; participants: number; code?: string }
export interface DiagOtherVersion { ip: string; protocol: number; newer: boolean }
export interface FirewallRule { nome: string; direcao: string; acao: string; perfis: number; ativa: boolean }
export interface FirewallStatus {
  suportado: boolean; bloqueado: boolean; liberado: boolean
  /** [dominio, privado, publico] */
  bloquear_tudo_por_perfil: [boolean, boolean, boolean]
  categoria_radmin: string | null
  regras: FirewallRule[]
  erro: string | null
}
export interface FirewallResult { ok: boolean; mensagem: string; removidas: number; perfis_liberados: string[] }
/** Placa de vídeo; sem Video Processor a conversão de cor vai pela CPU. */
export interface DiagVideoAdapter { name: string; video_processor: boolean }
export interface DiagVideoShare { encoder: string; conversion: LayerConversion[] }
/** Placas relevantes (vazio fora do Windows) e, transmitindo, o codificador e o conversor por camada. */
export interface DiagVideo { adapters: DiagVideoAdapter[]; sharing?: DiagVideoShare }
export interface DiagReport {
  app_version: string; protocol: number
  network: { interfaces: DiagInterface[]; radmin_ip?: string }
  vpns: string[]; antivirus: string[]
  firewall: FirewallStatus
  friends: DiagFriend[]; rooms: DiagRoom[]
  current_room?: DiagCurrentRoom
  other_versions: DiagOtherVersion[]
  newer_protocol?: number
  video: DiagVideo
  problems: DiagProblem[]
}
