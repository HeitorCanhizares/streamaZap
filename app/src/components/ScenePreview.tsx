// Tarefas previa-01..06, previa-08
import type { ReactNode } from 'react'
import { SceneGame } from './scenes/SceneGame'
import { SceneRace } from './scenes/SceneRace'
import { SceneMovie } from './scenes/SceneMovie'
import { SceneCode } from './scenes/SceneCode'
import { SceneDesktop } from './scenes/SceneDesktop'
import { SceneCam } from './scenes/SceneCam'

export type SceneName = 'game' | 'race' | 'movie' | 'code' | 'desktop' | 'cama' | 'camb' | 'camc' | 'camd'
export interface ScenePreviewProps { scene: SceneName }

// Miniatura 16:9 (componente-previa.html 14-146). aria-hidden: decorativa.
export function ScenePreview({ scene }: ScenePreviewProps): ReactNode {
  return (
    <div style={{ width: '100%', aspectRatio: '16 / 9', position: 'relative', overflow: 'hidden', background: '#0A100E', fontFamily: "'Space Grotesk', system-ui, sans-serif", color: '#E6F4EC' }}>
      <svg viewBox="0 0 160 90" preserveAspectRatio="xMidYMid slice" style={{ display: 'block', width: '100%', height: '100%' }} aria-hidden="true" data-scene={scene}>
        {scene === 'game' && <SceneGame />}
        {scene === 'race' && <SceneRace />}
        {scene === 'movie' && <SceneMovie />}
        {scene === 'code' && <SceneCode />}
        {scene === 'desktop' && <SceneDesktop />}
        {(scene === 'cama' || scene === 'camb' || scene === 'camc' || scene === 'camd') && <SceneCam scene={scene} />}
      </svg>
    </div>
  )
}
