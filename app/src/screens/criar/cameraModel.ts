// Modelo puro da câmera (plano contas-planos-camera, B2 e B4): jeito (canto/solta), cantos e
// qualidades que a câmera aguenta. Esqueleto: a tarefa front-05 preenche os corpos.
import type { CameraLayout, Corner, Preset, SourceItem } from '../../bridge/types'
import { MEDIDAS_PRESET } from '../../bridge/presets'

/** Escolha de quem transmite: câmera desligada, no canto da tela ou solta. */
export type CameraJeito = 'off' | 'canto' | 'solta'
/** O jeito de uma câmera ligada (sem o "desligada"). */
export type JeitoLigado = 'canto' | 'solta'

export interface CantoInfo { id: Corner; label: string }
/** Os 4 cantos, na ordem de leitura (de cima para baixo, da esquerda para a direita). */
export const CANTOS: CantoInfo[] = [
  { id: 'sup_esq', label: 'Superior esquerdo' },
  { id: 'sup_dir', label: 'Superior direito' },
  { id: 'inf_esq', label: 'Inferior esquerdo' },
  { id: 'inf_dir', label: 'Inferior direito' },
]

/** Canto do layout; `undefined` (app antigo) ou modo solta devolvem o padrão `inf_dir`. */
export function cantoDoLayout(layout: CameraLayout | undefined): Corner {
  return layout?.modo === 'canto' ? layout.canto : 'inf_dir'
}

/** `'solta'` só se o layout for `{modo:'solta'}`; ausente = `'canto'`. */
export function jeitoDoLayout(layout: CameraLayout | undefined): JeitoLigado {
  return layout?.modo === 'solta' ? 'solta' : 'canto'
}

/** Monta o layout: `'canto'` -> `{modo:'canto', canto}`; `'solta'` -> `{modo:'solta'}`. */
export function layoutDe(jeito: JeitoLigado, canto: Corner): CameraLayout {
  return jeito === 'canto' ? { modo: 'canto', canto } : { modo: 'solta' }
}

/** Uma qualidade da câmera na lista; `desligado` quando a câmera não faz (com o motivo). */
export interface OpcaoPreset { preset: Preset; desligado: boolean; motivo?: string }

/** Ordem de exibição das 4 qualidades. */
const PRESETS_EXIBIDOS: Preset[] = ['720p30', '720p60', '1080p30', '1080p60']

/** Ordem de busca ao ajustar: da maior para a menor. */
const PRESETS_DE_BUSCA: Preset[] = ['1080p60', '1080p30', '720p60', '720p30']

/** Preset que a câmera não faz: passa da altura máxima ou do fps máximo do item (se informados). */
function desligadoPara(preset: Preset, camera: SourceItem | undefined): boolean {
  const { altura, fps } = MEDIDAS_PRESET[preset]
  return (
    (camera?.max_height !== undefined && altura > camera.max_height) ||
    (camera?.max_fps !== undefined && fps > camera.max_fps)
  )
}

/** As 4 qualidades (`720p30`, `720p60`, `1080p30`, `1080p60`) para a câmera: desligadas as que passam
 *  de `max_height`/`max_fps` do item. Sem item ou sem os limites, nenhuma fica desligada. */
export function opcoesDaCamera(camera: SourceItem | undefined): OpcaoPreset[] {
  return PRESETS_EXIBIDOS.map((preset) => {
    if (!desligadoPara(preset, camera)) return { preset, desligado: false }
    return {
      preset,
      desligado: true,
      motivo: 'Sua câmera não faz ' + preset,
    }
  })
}

/** A maior qualidade que a câmera faz e que não passa de `preset` (o próprio, se couber). */
export function presetAjustado(preset: Preset, camera: SourceItem | undefined): Preset {
  if (!desligadoPara(preset, camera)) return preset
  const pedido = MEDIDAS_PRESET[preset]
  const achado = PRESETS_DE_BUSCA.find((p) => {
    const m = MEDIDAS_PRESET[p]
    return m.altura <= pedido.altura && m.fps <= pedido.fps && !desligadoPara(p, camera)
  })
  return achado ?? '720p30'
}

/** Nome do canto em minúsculas, para a frase do resumo. */
const NOME_CANTO: Record<Corner, string> = {
  sup_esq: 'superior esquerdo',
  sup_dir: 'superior direito',
  inf_esq: 'inferior esquerdo',
  inf_dir: 'inferior direito',
}

/** Texto do resumo de Criar sala: "Câmera: desligada", "Câmera: no canto inferior direito",
 *  "Câmera: solta". */
export function textoJeitoCamera(jeito: CameraJeito, canto: Corner): string {
  if (jeito === 'off') return 'Câmera: desligada'
  if (jeito === 'solta') return 'Câmera: solta'
  return 'Câmera: no canto ' + NOME_CANTO[canto]
}
