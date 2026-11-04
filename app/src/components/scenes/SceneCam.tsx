// Tarefa previa-06
import type { ReactNode } from 'react'

export type CamScene = 'cama' | 'camb' | 'camc' | 'camd'

// Paleta [fundo, figura, lâmpada] de cada variante (componente-previa.html, linhas 152-157).
export const CAM_PALETA: Record<CamScene, readonly [string, string, string]> = {
  cama: ['#1E3A2E', '#3B6B55', '#3BF08A'],
  camb: ['#3A2A1E', '#6B4F36', '#FFB547'],
  camc: ['#1E2C3F', '#3E5878', '#5CD6FF'],
  camd: ['#2E2340', '#57457A', '#FF8FB1'],
}

// Conteúdo SVG da cena de câmera (componente-previa.html, linhas 136-143).
export function SceneCam({ scene }: { scene: CamScene }): ReactNode {
  const [fundo, figura, lampada] = CAM_PALETA[scene]
  return (
    <>
      <rect width="160" height="90" fill={fundo} />
      <rect x="0" y="22" width="160" height="2" fill={figura} opacity={0.5} />
      <rect x="18" y="10" width="10" height="12" fill={figura} opacity={0.6} />
      <rect x="30" y="14" width="6" height="8" fill={figura} opacity={0.45} />
      <circle cx="136" cy="18" r="6" fill={lampada} opacity={0.8} />
      <rect x="135" y="24" width="2" height="20" fill={figura} opacity={0.6} />
      <circle cx="80" cy="40" r="15" fill={figura} />
      <path d="M46 90c0-21 15-34 34-34s34 13 34 34z" fill={figura} />
    </>
  )
}
