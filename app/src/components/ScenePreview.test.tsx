// Tarefa previa-08
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { ScenePreview, type SceneName } from './ScenePreview'

afterEach(() => cleanup())

const CENAS: SceneName[] = ['game', 'race', 'movie', 'code', 'desktop', 'cama', 'camb', 'camc', 'camd']

describe('ScenePreview', () => {
  it.each(CENAS)('renderiza a cena %s sem lançar e com o svg decorativo', (scene) => {
    const { container } = render(<ScenePreview scene={scene} />)
    const svg = container.querySelector('svg')
    expect(svg).not.toBeNull()
    expect(svg?.getAttribute('viewBox')).toBe('0 0 160 90')
    expect(svg?.getAttribute('aria-hidden')).toBe('true')
    expect(svg?.getAttribute('data-scene')).toBe(scene)
    expect((svg?.children.length ?? 0)).toBeGreaterThanOrEqual(5)
  })

  it('o wrapper tem aspect-ratio 16 / 9 e fundo #0A100E', () => {
    const { container } = render(<ScenePreview scene="game" />)
    const wrapper = container.firstElementChild as HTMLElement
    const estilo = wrapper.getAttribute('style') ?? ''
    expect(estilo).toMatch(/aspect-ratio:\s*16\s*\/\s*9/)
    expect(estilo).toMatch(/background:\s*(#0A100E|rgb\(10,\s*16,\s*14\))/i)
  })

  it('mostra o contador 24/90 na cena game', () => {
    render(<ScenePreview scene="game" />)
    expect(screen.getByText('24/90')).toBeTruthy()
  })

  it('mostra a velocidade 212 na cena race', () => {
    render(<ScenePreview scene="race" />)
    expect(screen.getByText('212')).toBeTruthy()
  })

  it('mostra a fala "Você viu aquilo" na cena movie', () => {
    const { container } = render(<ScenePreview scene="movie" />)
    const texto = container.querySelector('text')
    expect(texto?.textContent).toContain('Você viu aquilo')
  })

  it('mostra o comando "$ python main.py" na cena code', () => {
    render(<ScenePreview scene="code" />)
    expect(screen.getByText('$ python main.py')).toBeTruthy()
  })
})
