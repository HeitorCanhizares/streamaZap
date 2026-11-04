// Tarefa previa-06
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render } from '@testing-library/react'
import { SceneCam, type CamScene } from './SceneCam'

afterEach(() => cleanup())

const CASOS: { scene: CamScene; fundo: string; figura: string; lampada: string }[] = [
  { scene: 'cama', fundo: '#1E3A2E', figura: '#3B6B55', lampada: '#3BF08A' },
  { scene: 'camb', fundo: '#3A2A1E', figura: '#6B4F36', lampada: '#FFB547' },
  { scene: 'camc', fundo: '#1E2C3F', figura: '#3E5878', lampada: '#5CD6FF' },
  { scene: 'camd', fundo: '#2E2340', figura: '#57457A', lampada: '#FF8FB1' },
]

function renderCam(scene: CamScene) {
  const { container } = render(
    <svg>
      <SceneCam scene={scene} />
    </svg>,
  )
  return container
}

describe('SceneCam', () => {
  for (const caso of CASOS) {
    describe(caso.scene, () => {
      it('pinta o fundo 160x90 com a cor de fundo da variante', () => {
        const container = renderCam(caso.scene)
        const fundo = container.querySelector(`rect[fill="${caso.fundo}"]`)
        expect(fundo).not.toBeNull()
        expect(fundo?.getAttribute('width')).toBe('160')
        expect(fundo?.getAttribute('height')).toBe('90')
      })

      it('desenha a lâmpada com circle r=6 na cor da lâmpada', () => {
        const container = renderCam(caso.scene)
        const lampada = container.querySelector('circle[r="6"]')
        expect(lampada).not.toBeNull()
        expect(lampada?.getAttribute('fill')).toBe(caso.lampada)
        expect(lampada?.getAttribute('cx')).toBe('136')
        expect(lampada?.getAttribute('cy')).toBe('18')
        expect(lampada?.getAttribute('opacity')).toBe('0.8')
      })

      it('desenha a figura com a cor de figura', () => {
        const container = renderCam(caso.scene)
        const figuras = container.querySelectorAll(`[fill="${caso.figura}"]`)
        expect(figuras.length).toBe(6)
      })

      it('tem o path da silhueta com o d exato', () => {
        const container = renderCam(caso.scene)
        const path = container.querySelector('path')
        expect(path?.getAttribute('d')).toBe('M46 90c0-21 15-34 34-34s34 13 34 34z')
        expect(path?.getAttribute('fill')).toBe(caso.figura)
      })
    })
  }
})
