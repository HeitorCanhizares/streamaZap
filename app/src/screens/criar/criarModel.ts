// Modelo puro da tela Criar sala (docs/design/2-criar-sala.html, script x-dc, linhas 245-355).
import type { Preset, RoomMode, SceneKind, SourceItem } from '../../bridge/types'

export type Acesso = 'aberta' | 'senha'
export type SomModo = 'none' | 'all' | 'apps'
export type FonteTab = 'telas' | 'janelas' | 'webcam'

export interface ModoInfo { value: RoomMode; label: string; hint: string; tech: string; cor: string }
export interface AcessoInfo { value: Acesso; label: string; hint: string }
export interface PresetInfo { value: Preset; res: string; fps: string; bitrate: string }

export const MODOS: ModoInfo[] = [
  {
    value: 'online',
    label: 'Online',
    hint: 'Pelo servidor streamaZap. Amigos de qualquer lugar entram com um código, sem precisar de Radmin.',
    tech: 'servidor sa-east · 42 ms',
    cor: '#3BF08A',
  },
  {
    value: 'local',
    label: 'Local',
    hint: 'Direto entre os PCs, pelo Radmin VPN ou pela rede de casa. Não passa pelo servidor.',
    tech: 'P2P · porta 47800',
    cor: '#5CD6FF',
  },
]

export const ACESSOS: AcessoInfo[] = [
  { value: 'aberta', label: 'Aberta', hint: 'Qualquer um na rede entra' },
  { value: 'senha', label: 'Com senha', hint: 'Só quem souber a senha' },
]

export const PRESETS_UI: PresetInfo[] = [
  { value: '480p30', res: '480p', fps: '30 fps', bitrate: '1,5 Mbps' },
  { value: '720p30', res: '720p', fps: '30 fps · Radmin', bitrate: '2,5 Mbps' },
  { value: '720p60', res: '720p', fps: '60 fps', bitrate: '4,0 Mbps' },
  { value: '1080p60', res: '1080p', fps: '60 fps', bitrate: '8,0 Mbps' },
]

/** Frase sob o título "Bora transmitir?" conforme o modo. */
export function modoHint(modo: RoomMode): string {
  return modo === 'online'
    ? 'A sala vai pro servidor e aparece pra todo mundo que usa o app. Pra entrar, é só mandar o código no zap.'
    : 'A sala aparece sozinha pra quem estiver na mesma rede do Radmin ou na sua rede local.'
}

/** Texto do campo "Modo" do resumo. */
export function modoTexto(modo: RoomMode): string {
  return modo === 'online' ? 'Online · via servidor' : 'Local · Radmin / rede local'
}

/** Texto da linha de segurança no rodapé do resumo. */
export function seguroTexto(modo: RoomMode): string {
  return modo === 'online'
    ? 'Conexão com o servidor autenticada'
    : 'Firewall liberado para receber conexões'
}

/** Texto do campo "Acesso" do resumo. */
export function acessoTexto(acesso: Acesso): string {
  return acesso === 'senha' ? 'Com senha' : 'Aberta para a rede'
}

/** Texto do campo "Som" do resumo; `apps` são os nomes marcados. */
export function somTexto(som: SomModo, apps: string[]): string {
  if (som === 'none') return 'Sem som'
  if (som === 'all') return 'Todo o som do PC'
  return apps.length > 0 ? 'Só ' + apps.join(', ') : 'Nenhum app marcado'
}

/** Texto do campo "Vídeo" do resumo, ex.: "720p30 · 2,5 Mbps · adaptável". */
export function qualidadeTexto(preset: Preset, adaptavel: boolean): string {
  const info = PRESETS_UI.find((p) => p.value === preset)
  const base = `${preset} · ${info ? info.bitrate : ''}`
  return adaptavel ? base + ' · adaptável' : base
}

/** Segunda linha do cartão de fonte (mono), a partir do tipo. */
export function fonteMeta(item: SourceItem): string {
  switch (item.kind) {
    case 'screen':
      return 'Tela inteira'
    case 'window':
      return 'Janela'
    case 'camera':
      return 'Webcam'
    case 'app':
      return item.name
  }
}

const CENAS_POR_ABA: Record<FonteTab, SceneKind[]> = {
  telas: ['game', 'desktop'],
  janelas: ['game', 'movie', 'code'],
  webcam: ['camc', 'cama'],
}

/** Cena usada na miniatura da fonte `indice` da aba (telas: game/desktop; janelas: game/movie/code; webcam: camc/cama). */
export function cenaDaFonte(tab: FonteTab, indice: number): SceneKind {
  const cenas = CENAS_POR_ABA[tab]
  const n = cenas.length
  return cenas[((indice % n) + n) % n]
}
