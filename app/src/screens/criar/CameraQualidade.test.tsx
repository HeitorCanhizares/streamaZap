import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { CameraQualidade } from './CameraQualidade'
import type { SourceItem } from '../../bridge/types'

afterEach(cleanup)

const cam = (max_height: number, max_fps: number): SourceItem =>
  ({ id: 'camera:0', kind: 'camera', name: 'c', max_height, max_fps }) as SourceItem

const AJUDA = 'Algumas qualidades estão desligadas porque a câmera não faz.'

describe('CameraQualidade', () => {
  it('sem câmera: 4 radios habilitados e sem linha de ajuda', () => {
    render(<CameraQualidade preset="720p30" onPreset={() => {}} />)
    const radios = screen.getAllByRole('radio')
    expect(radios).toHaveLength(4)
    radios.forEach((r) => expect((r as HTMLButtonElement).disabled).toBe(false))
    expect(screen.queryByText(AJUDA)).toBeNull()
  })

  it('com câmera 720p30: só o primeiro habilita, mostra ajuda e motivo', () => {
    render(<CameraQualidade preset="720p30" onPreset={() => {}} camera={cam(720, 30)} />)
    const radios = screen.getAllByRole('radio') as HTMLButtonElement[]
    expect(radios[0].disabled).toBe(false)
    radios.slice(1).forEach((r) => expect(r.disabled).toBe(true))
    expect(screen.getByText(AJUDA)).toBeTruthy()
    expect(radios[3].title).toBe('Sua câmera não faz 1080p60')
  })

  it('clicar habilitado chama onPreset; desligado não chama', () => {
    const onPreset = vi.fn()
    render(<CameraQualidade preset="720p30" onPreset={onPreset} camera={cam(720, 60)} />)
    const radios = screen.getAllByRole('radio')
    fireEvent.click(radios[1])
    expect(onPreset).toHaveBeenCalledWith('720p60')
    onPreset.mockClear()
    fireEvent.click(radios[2])
    expect(onPreset).not.toHaveBeenCalled()
  })

  it('aria-label padrão e personalizado', () => {
    const { rerender } = render(<CameraQualidade preset="720p30" onPreset={() => {}} />)
    expect(screen.getByRole('radiogroup', { name: 'Qualidade da câmera' })).toBeTruthy()
    rerender(<CameraQualidade preset="720p30" onPreset={() => {}} rotulo="Outra" />)
    expect(screen.getByRole('radiogroup', { name: 'Outra' })).toBeTruthy()
  })

  it('o preset atual fica aria-checked', () => {
    render(<CameraQualidade preset="1080p30" onPreset={() => {}} />)
    const radios = screen.getAllByRole('radio')
    expect(radios[2].getAttribute('aria-checked')).toBe('true')
    expect(radios[0].getAttribute('aria-checked')).toBe('false')
  })
})
