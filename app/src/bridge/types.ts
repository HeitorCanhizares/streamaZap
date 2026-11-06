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
// camera_layout: só trilha 'camera'. Ausente = canto inferior direito (app antigo ou servidor sem o campo).
export interface TrackView { id: string; owner_id: string; kind: TrackKind; title: string; number: number; camera_layout?: CameraLayout }
export interface ChatLine { id: string; author_id: string; author: string; color: string; text: string; ts: number }
// code: codigo para convite (ex.: 'ZAP-7K2Q'); ausente na sala Local sem codigo.
// me_id: meu id de participante nesta sala (no Online vem do servidor, diferente do install_id).
// owner_id: id do dono da sala (pode tirar qualquer música). music: fila de música (ausente = a sala
// não tem: Online, ou o dono tem um app antigo).
export interface RoomView { id: string; name: string; mode: RoomMode; code?: string; participants: Participant[]; tracks: TrackView[]; chat: ChatLine[]; unread: number; me_id?: string; owner_id?: string; music?: MusicView | null }
// Música da sala (docs/planos/musica-da-sala.md). position_ms vale no instante at_ms (ms desde 1970).
export interface MusicItemView { id: string; video_id: string; title: string; channel?: string; duration_ms?: number; live: boolean; added_by: string; added_by_name: string }
export interface MusicNowView { item: MusicItemView; position_ms: number; at_ms: number; votes: string[]; needed: number }
export interface MusicView { queue: MusicItemView[]; current?: MusicNowView | null }
// O que o player descobriu sobre um item (musica.info).
export interface MusicReport { item_id: string; duration_ms?: number; live?: boolean; title?: string; ended?: boolean; failed?: boolean }
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
  /** Conta e plano da sessão Online (o que o servidor pede: conta, senha). Ausente = sem servidor. */
  account?: AccountView
}
/** Como o app faz login: `conta` do /v1/info do servidor (espelha zap-core online::account). */
export interface AccountProviderView { provider: string; url: string; key: string }
/** Plano da sessão (resposta do POST /v1/sessoes): id no fio e limites; só o id importa para a tela. */
export interface PlanView { id: 'nenhum' | 'espectador' | 'hd' | 'fullhd'; pode_entrar: boolean }
/** `Snapshot.account`: ausente `account_provider` = o servidor não confere contas (sem login). */
export interface AccountView {
  account_provider?: AccountProviderView
  requires_account: boolean
  requires_password: boolean
  account_id?: string
  plan?: PlanView
  /** 'senha_servidor' | 'conta_necessaria' quando o último login falhou por isso. */
  login_error?: string
}
export interface ServerStatus { state: 'off' | 'connecting' | 'online' | 'error'; name?: string; ping_ms?: number; message?: string }
export interface ServerTestResult { ok: boolean; name: string | null; ping_ms: number | null; code: string | null; message: string }
// Jeito da câmera para quem assiste (espelha zap_proto::CameraLayout): no canto da tela de quem
// transmite ou solta, numa célula própria. JSON: {"modo":"canto","canto":"inf_dir"} | {"modo":"solta"}.
export type Corner = 'sup_esq' | 'sup_dir' | 'inf_esq' | 'inf_dir'
export type CameraLayout = { modo: 'canto'; canto: Corner } | { modo: 'solta' }
/** Quando instalar a atualização baixada: perguntar, ao fechar o app ou sozinho (fora de sala). */
export type InstallMode = 'ask' | 'on_exit' | 'auto'
export type SceneKind = 'game' | 'race' | 'movie' | 'code' | 'desktop' | 'cama' | 'camb' | 'camc' | 'camd'

export interface Settings {
  name: string; color: string; install_id: string; open_in: RoomMode
  // enabled: modo Online ligado. Desligado, o app não mostra nada do Online nem abre conexão com o servidor.
  server?: { enabled: boolean; address: string; password: string; use_relay: boolean }
  local: { interface: string | null; port: number; announce: boolean }
  video: { camera_id: string | null; mirror_preview: boolean; default_preset: Preset; camera_preset: Preset; camera_layout: CameraLayout; encoder?: string; smoothing_auto: boolean; smoothing_ms: number }
  sound: { initial_volume: number; message_sound: boolean; notify_room_open: boolean; mute_when_minimized: boolean }
  update: { check: boolean; download: boolean; install: InstallMode; channel: 'estavel' | 'beta' }
}
// sound: só nos apps. playing = tocando agora; ready = já abriu o som (quieto); silent = só tem janela.
export type AppSound = 'playing' | 'ready' | 'silent'
// max_height/max_fps: só nas câmeras, o melhor modo nativo (ausentes = não se sabe: não desligar nada).
export interface SourceItem { id: string; kind: 'screen' | 'window' | 'camera' | 'app'; name: string; sound?: AppSound; max_height?: number; max_fps?: number }
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
/** Payload de `zap://update-aviso`: `segundos` até instalar sozinho (30 ao abrir); 0 = fechar o aviso. */
export interface UpdateAviso { versao: string; segundos: number }
/** Payload de `zap://update-aplicada` e resposta de `update_applied_take`: "Atualizado para a vX". */
export interface UpdateAplicada { versao: string; anterior: string; notas: string | null }
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
