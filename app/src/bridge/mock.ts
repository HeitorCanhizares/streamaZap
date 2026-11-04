// Dados do design para uso fora do Tauri (tarefa ui-07).
// Cada função monta objetos NOVOS a cada chamada: nada aqui é compartilhado,
// então quem mutar o retorno não altera o mock das próximas chamadas.
import type { Settings, Snapshot, Sources } from './types'

export const mockSnapshot = (): Snapshot => ({
  version: 1,
  me: { name: 'Você', color: '#3BF08A', install_id: 'mock-install' },
  network: { radmin_ip: '26.14.7.88', interfaces: ['Radmin VPN', 'Ethernet'] },
  rooms: [
    {
      room_id: 'zap-7k2q',
      name: 'Sexta do Rush',
      host: 'Marcão',
      participants: 3,
      has_password: false,
      mode: 'online',
      scene: 'game',
    },
    {
      room_id: 'zap-3m9x',
      name: 'Filminho de domingo',
      host: 'Ju',
      participants: 2,
      has_password: true,
      mode: 'online',
      scene: 'movie',
    },
    {
      room_id: 'zap-b5tf',
      name: 'Live de Python',
      host: 'Rafa',
      participants: 1,
      has_password: false,
      mode: 'online',
      scene: 'code',
    },
  ],
  peers: [
    { name: 'Marcão', version: '3.0.0', ip: '26.201.4.12', receives_you: true },
    { name: 'Ju', version: '3.0.0', ip: '26.88.19.5', receives_you: true },
    { name: 'Rafa', version: '3.0.0', ip: '26.3.140.77', receives_you: false },
    { name: 'Dudu', version: '3.0.0', ip: '26.57.210.9', receives_you: true },
  ],
  room: {
    id: 'zap-7k2q',
    name: 'Sexta do Rush',
    mode: 'online',
    participants: [
      { id: 'p-marcao', name: 'Marcão', color: '#FFB547', ping_ms: 24, sharing: true, camera: false },
      { id: 'p-ju', name: 'Ju', color: '#7CF2C0', ping_ms: 58, sharing: false, camera: true },
      { id: 'p-rafa', name: 'Rafa', color: '#5CD6FF', ping_ms: 92, sharing: false, camera: false },
      { id: 'p-dudu', name: 'Dudu', color: '#FF9E5C', ping_ms: 140, sharing: true, camera: false },
    ],
    tracks: [
      { id: 'trk-1', owner_id: 'p-marcao', kind: 'screen', title: 'Tela inteira', number: 1 },
      { id: 'trk-2', owner_id: 'p-dudu', kind: 'window', title: 'cs2.exe', number: 1 },
      { id: 'trk-3', owner_id: 'p-ju', kind: 'camera', title: 'Webcam', number: 1 },
    ],
    chat: [
      { id: 'c1', author_id: 'p-ju', author: 'Ju', color: '#7CF2C0', text: 'cheguei! tá liso aqui, 38 ms', ts: 1767312600000 },
      { id: 'c2', author_id: 'p-rafa', author: 'Rafa', color: '#5CD6FF', text: 'o áudio do jogo tá vindo junto', ts: 1767312660000 },
      { id: 'c3', author_id: 'p-marcao', author: 'Marcão', color: '#FFB547', text: 'tá sim, só o cs2.exe. o spot tá limpo', ts: 1767312720000 },
      { id: 'c4', author_id: 'p-dudu', author: 'Dudu', color: '#FF9E5C', text: 'abri o mapa, clica no meu quad', ts: 1767312780000 },
    ],
    unread: 0,
  },
  tiles: [
    {
      tile: 0,
      track_id: 'trk-1',
      layer: 2,
      width: 1280,
      height: 720,
      fps: 60,
      kbps: 5200,
      ping_ms: 24,
      smoothing_ms: 200,
      reason: 'banda ok',
    },
  ],
  share: null,
  errors: [],
})

export const mockSettings = (): Settings => ({
  name: 'Você',
  color: '#3BF08A',
  install_id: 'mock-install',
  open_in: 'local',
  local: { interface: null, port: 47800, announce: true },
  video: {
    camera_id: null,
    mirror_preview: true,
    default_preset: '720p30',
    smoothing_auto: true,
    smoothing_ms: 200,
  },
  sound: {
    initial_volume: 0.8,
    message_sound: true,
    notify_room_open: true,
    mute_when_minimized: true,
  },
  update: { auto: true, channel: 'estavel' },
})

export const mockSources = (): Sources => ({
  screens: [
    { id: 'screen:0', kind: 'screen', name: 'Tela 1 (1920x1080)' },
    { id: 'screen:1', kind: 'screen', name: 'Tela 2 (2560x1440)' },
  ],
  windows: [
    { id: 'window:cs2', kind: 'window', name: 'cs2.exe — Counter-Strike 2' },
    { id: 'window:chrome', kind: 'window', name: 'Chrome — Filminho' },
    { id: 'window:vscode', kind: 'window', name: 'VS Code — live de Python' },
  ],
  cameras: [{ id: 'camera:0', kind: 'camera', name: 'Webcam (Logitech C920)' }],
  apps: [
    { id: 'app:discord', kind: 'app', name: 'Discord' },
    { id: 'app:chrome', kind: 'app', name: 'Chrome' },
    { id: 'app:jogo', kind: 'app', name: 'Jogo' },
  ],
  encoders: ['Automatico', 'NVENC', 'x264'],
})
