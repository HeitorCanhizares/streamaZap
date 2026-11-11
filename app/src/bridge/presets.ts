// Medidas dos 4 presets de qualidade (altura e fps). Não são limites de plano.
import type { Preset } from './types'

export const MEDIDAS_PRESET: Record<Preset, { altura: number; fps: number }> = {
  '720p30': { altura: 720, fps: 30 },
  '720p60': { altura: 720, fps: 60 },
  '1080p30': { altura: 1080, fps: 30 },
  '1080p60': { altura: 1080, fps: 60 },
}

/** Do menor para o maior. */
export const PRESETS_ORDEM: Preset[] = ['720p30', '720p60', '1080p30', '1080p60']
