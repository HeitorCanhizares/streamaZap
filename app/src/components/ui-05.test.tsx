// Tarefa ui-05
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { ScenePreview } from './ScenePreview'
import { Select } from './Select'
import { TextField } from './TextField'

afterEach(() => cleanup())

describe('Select', () => {
  it('dispara onChange com o valor escolhido', () => {
    const onChange = vi.fn()
    const { container } = render(
      <Select
        options={[{ value: 'a', label: 'Alfa' }, { value: 'b', label: 'Beta' }]}
        value="a"
        onChange={onChange}
      />,
    )
    const select = container.querySelector('select') as HTMLSelectElement
    fireEvent.change(select, { target: { value: 'b' } })
    expect(onChange).toHaveBeenCalledWith('b')
  })
})

describe('TextField', () => {
  it('dispara onChange com o texto digitado', () => {
    const onChange = vi.fn()
    render(<TextField value="" onChange={onChange} placeholder="nome" />)
    const input = screen.getByPlaceholderText('nome') as HTMLInputElement
    fireEvent.change(input, { target: { value: 'ana' } })
    expect(onChange).toHaveBeenCalledWith('ana')
  })

  it('respeita type="password" e o padrão é text', () => {
    const { container, unmount } = render(<TextField value="" onChange={() => {}} type="password" />)
    expect((container.querySelector('input') as HTMLInputElement).type).toBe('password')
    unmount()
    const { container: c2 } = render(<TextField value="" onChange={() => {}} />)
    expect((c2.querySelector('input') as HTMLInputElement).type).toBe('text')
  })
})

describe('ScenePreview', () => {
  it('renderiza um svg para cada uma das 4 cenas', () => {
    for (const scene of ['game', 'race', 'movie', 'code'] as const) {
      const { container, unmount } = render(<ScenePreview scene={scene} />)
      expect(container.querySelectorAll('svg').length).toBe(1)
      unmount()
    }
  })

  it('a cena de corrida contém o texto KM/H', () => {
    const { container } = render(<ScenePreview scene="race" />)
    const texts = Array.from(container.querySelectorAll('text')).map((t) => t.textContent)
    expect(texts).toContain('KM/H')
  })
})
