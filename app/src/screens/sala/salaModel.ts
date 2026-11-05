// Funcoes puras da tela da sala (tarefa ui-ligacao-06).
import type { NetworkStatus, Participant, RoomView, TileView, TrackView } from '../../bridge/types'

/** Ids de tile reservados para o "Voce" (a propria tela e a propria camera, camada 2). */
export const LOCAL_SCREEN_TILE = 9000
export const LOCAL_CAMERA_TILE = 9001

/** 'ONLINE · ZAP-7K2Q' (modo online com codigo), 'LOCAL' (modo local; com codigo: 'LOCAL · <codigo>'). */
export function roomChip(room: RoomView): string {
  // Modo online sempre aparece como ONLINE; o codigo so entra junto se existir.
  const base = room.mode === 'online' ? 'ONLINE' : 'LOCAL'
  return room.code ? `${base} · ${room.code}` : base
}

/** Rotulo do tipo da trilha: screen 'Tela', window 'Janela', camera 'Webcam', audio 'Só o som'. */
export function kindLabel(kind: TrackView['kind']): string {
  switch (kind) {
    case 'screen':
      return 'Tela'
    case 'window':
      return 'Janela'
    case 'camera':
      return 'Webcam'
    case 'audio':
      return 'Só o som'
  }
}

/** Quem estou assistindo no tile principal: { name, kind } ou null sem tile/trilha/dono. */
export function watching(room: RoomView, main: Pick<TileView, 'track_id'> | null): { name: string; kind: string } | null {
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

// ---------------------------------------------------------------------------------------------
// Várias transmissões na sala: cada uma vai na tela principal ou na faixa de baixo (ou fica
// fechada, sem receber).

/** Onde a transmissão aparece. 'fechada' = sem receber nada (economiza banda). */
export type Lugar = 'principal' | 'faixa' | 'fechada'

/** Uma transmissão na sala: vídeo (tela, janela, câmera) ou só o som. As minhas (`mine`) são só
 *  vídeo e chegam pela prévia local do núcleo, sem rede. */
export interface Stream {
  trackId: string
  ownerId: string
  kind: TrackView['kind']
  /** Tile fixo desta trilha enquanto o app estiver aberto (o núcleo assina por tile). */
  tile: number
  /** Minha transmissão (a tela ou a câmera que eu estou mandando). */
  mine?: boolean
}

// Tiles das transmissões: abaixo dos reservados do "Você" (9000) e da prévia da lista (1_000_000).
const tilesPorTrilha = new Map<string, number>()
let proximoTile = 1

/** Tile estável da trilha: a mesma trilha sempre usa o mesmo tile. */
export function tileDaTrilha(trackId: string): number {
  let t = tilesPorTrilha.get(trackId)
  if (t === undefined) {
    t = proximoTile++
    if (proximoTile >= LOCAL_SCREEN_TILE) proximoTile = 1
    tilesPorTrilha.set(trackId, t)
  }
  return t
}

const ehVideo = (k: TrackView['kind']): boolean => k !== 'audio'
// Tela e janela antes da câmera; depois a ordem de chegada (número da trilha).
const ordemTipo = (k: TrackView['kind']): number => (k === 'camera' ? 1 : 0)

/** Transmissões da sala: os vídeos dos outros (tela/janela antes da câmera), o som de quem só manda
 *  som (o som de quem tem vídeo vem junto com o vídeo) e, por último, os meus vídeos. */
export function streamsDaSala(room: RoomView, meId: string): Stream[] {
  const porTipo = (a: TrackView, b: TrackView): number => ordemTipo(a.kind) - ordemTipo(b.kind) || a.number - b.number
  const dosOutros = room.tracks.filter((t) => t.owner_id !== meId)
  const comVideo = new Set(dosOutros.filter((t) => ehVideo(t.kind)).map((t) => t.owner_id))
  const videos = dosOutros.filter((t) => ehVideo(t.kind)).sort(porTipo)
  const soSom = dosOutros.filter((t) => t.kind === 'audio' && !comVideo.has(t.owner_id))
  const meus = room.tracks.filter((t) => t.owner_id === meId && ehVideo(t.kind)).sort(porTipo)
  const vira = (t: TrackView, mine: boolean): Stream => ({ trackId: t.id, ownerId: t.owner_id, kind: t.kind, tile: tileDaTrilha(t.id), ...(mine ? { mine } : {}) })
  return [...videos.map((t) => vira(t, false)), ...soSom.map((t) => vira(t, false)), ...meus.map((t) => vira(t, true))]
}

/**
 * Lugar de cada transmissão. A escolha do usuário vale (o som sozinho nunca vai para a principal).
 * Sem escolha: se nada estiver na principal, a primeira tela/janela (ou câmera) dos outros vai para
 * lá; sem vídeo dos outros, a minha (quem só tem a própria transmissão vê ela grande). A câmera de
 * quem está com a tela na principal vai junto (no canto da tela); o resto, na faixa.
 */
export function lugaresDasStreams(streams: Stream[], escolhas: Record<string, Lugar>): Record<string, Lugar> {
  const out: Record<string, Lugar> = {}
  for (const s of streams) {
    const e = escolhas[s.trackId]
    if (e) out[s.trackId] = s.kind === 'audio' && e === 'principal' ? 'faixa' : e
  }
  const semEscolha = streams.filter((s) => out[s.trackId] === undefined && ehVideo(s.kind))
  const temPrincipal = (): boolean => streams.some((s) => out[s.trackId] === 'principal')
  const primeiraDe = (lista: Stream[]): Stream | undefined => lista.find((s) => s.kind !== 'camera') ?? lista[0]
  if (!temPrincipal()) {
    const primeira = primeiraDe(semEscolha.filter((s) => !s.mine)) ?? primeiraDe(semEscolha.filter((s) => s.mine))
    if (primeira) out[primeira.trackId] = 'principal'
  }
  const telaNaPrincipal = new Set(
    streams.filter((s) => s.kind !== 'camera' && s.kind !== 'audio' && out[s.trackId] === 'principal').map((s) => s.ownerId),
  )
  for (const s of streams) {
    if (out[s.trackId] !== undefined) continue
    out[s.trackId] = s.kind === 'camera' && telaNaPrincipal.has(s.ownerId) ? 'principal' : 'faixa'
  }
  return out
}

/** Uma célula da tela principal: a transmissão e, se for a tela de alguém, a câmera dele no canto. */
export interface Celula { stream: Stream; pip: Stream | null }

/** Células da principal, na ordem das transmissões. A câmera de quem também está com a tela (ou
 *  janela) na principal vira o canto dessa tela em vez de outra célula. */
export function celulasDaPrincipal(streams: Stream[], lugares: Record<string, Lugar>): Celula[] {
  const naPrincipal = streams.filter((s) => lugares[s.trackId] === 'principal' && ehVideo(s.kind))
  const usadas = new Set<string>()
  const celulas: Celula[] = []
  for (const s of naPrincipal) {
    if (s.kind === 'camera') continue
    const pip = naPrincipal.find((c) => c.kind === 'camera' && c.ownerId === s.ownerId && !usadas.has(c.trackId)) ?? null
    if (pip) usadas.add(pip.trackId)
    celulas.push({ stream: s, pip })
  }
  for (const s of naPrincipal) {
    if (s.kind === 'camera' && !usadas.has(s.trackId)) celulas.push({ stream: s, pip: null })
  }
  return celulas
}

/** O que pedir ao núcleo por lugar: tamanho em px e teto de camada (null = sem teto). A minha
 *  transmissão na principal pede o tamanho médio: o núcleo mostra a camada do meio, sem ligar a
 *  melhor só para eu me ver. */
export function pedidoDoLugar(
  lugar: 'principal' | 'faixa' | 'canto',
  celulas: number,
  mine = false,
): { size: [number, number]; maxLayer: number | null } {
  if (lugar === 'faixa') return { size: [320, 180], maxLayer: 2 }
  if (mine && lugar === 'principal') return { size: [1280, 720], maxLayer: null }
  if (lugar === 'canto') return { size: [480, 270], maxLayer: 1 }
  // Mais células dividem a tela: cada uma pede menos.
  if (celulas <= 1) return { size: [1920, 1080], maxLayer: null }
  if (celulas <= 4) return { size: [960, 540], maxLayer: null }
  return { size: [640, 360], maxLayer: 1 }
}

/** Volume que EU escolhi para uma pessoa (0..100) e se ela está muda para mim. */
export interface VolumePessoa { vol: number; muted: boolean }

/** Volume inicial: o que o núcleo guardou para a pessoa (padrão 80% e com som). */
export function volumeInicial(p: Participant | undefined): VolumePessoa {
  return { vol: Math.round((p?.volume ?? 0.8) * 100), muted: p?.muted ?? false }
}
