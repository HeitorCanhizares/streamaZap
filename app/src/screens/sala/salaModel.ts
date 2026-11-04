// Funcoes puras da tela da sala (tarefa ui-ligacao-06).
import type { NetworkStatus, RoomView, TileView, TrackView } from '../../bridge/types'

/** Ids de tile reservados para o "Voce" (a propria tela e a propria camera, camada 2). */
export const LOCAL_SCREEN_TILE = 9000
export const LOCAL_CAMERA_TILE = 9001

/** 'ONLINE · ZAP-7K2Q' (modo online com codigo), 'LOCAL' (modo local; com codigo: 'LOCAL · <codigo>'). */
export function roomChip(room: RoomView): string {
  // Modo online sempre aparece como ONLINE; o codigo so entra junto se existir.
  const base = room.mode === 'online' ? 'ONLINE' : 'LOCAL'
  return room.code ? `${base} · ${room.code}` : base
}

/** Rotulo do tipo da trilha: screen 'Tela', window 'Janela', camera 'Webcam'. */
export function kindLabel(kind: TrackView['kind']): string {
  switch (kind) {
    case 'screen':
      return 'Tela'
    case 'window':
      return 'Janela'
    case 'camera':
      return 'Webcam'
  }
}

/** Quem estou assistindo no tile principal: { name, kind } ou null sem tile/trilha/dono. */
export function watching(room: RoomView, main: TileView | null): { name: string; kind: string } | null {
  if (!main) return null
  const track = room.tracks.find((t) => t.id === main.track_id)
  if (!track) return null
  const dono = room.participants.find((p) => p.id === track.owner_id)
  if (!dono) return null
  return { name: dono.name, kind: kindLabel(track.kind) }
}

/** Texto do convite copiado. Ex.: 'Entra na sala "Sexta do Rush" no streamaZap: código ZAP-7K2Q'.
 *  Sem codigo e com radmin_ip: 'Entra na sala "X" no streamaZap: <radmin_ip>'. Sem nada: so o nome. */
export function inviteText(room: RoomView, network: NetworkStatus): string {
  const base = `Entra na sala "${room.name}" no streamaZap`
  if (room.code) return `${base}: código ${room.code}`
  if (network.radmin_ip) return `${base}: ${network.radmin_ip}`
  return base
}

/** Trilhas do proprio usuario na sala: { screen: primeira screen|window, camera: primeira camera }. */
export function ownTracks(room: RoomView, meId: string): { screen: TrackView | null; camera: TrackView | null } {
  const mine = room.tracks.filter((t) => t.owner_id === meId)
  const screen = mine.find((t) => t.kind === 'screen' || t.kind === 'window') ?? null
  const camera = mine.find((t) => t.kind === 'camera') ?? null
  return { screen, camera }
}
