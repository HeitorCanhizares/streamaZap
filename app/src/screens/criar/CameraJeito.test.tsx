import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { CameraJeito } from './CameraJeito'

function monta(jeito: 'off' | 'canto' | 'solta') {
  const onJeito = vi.fn()
  const onCanto = vi.fn()
  render(<CameraJeito jeito={jeito} onJeito={onJeito} canto="inf_dir" onCanto={onCanto} />)
  return { onJeito, onCanto }
}

afterEach(() => cleanup())

describe('CameraJeito', () => {
  it('mostra os 3 jeitos', () => {
    monta('off')
    const grupo = screen.getByRole('radiogroup', { name: 'Jeito da câmera' })
    expect(grupo).toBeTruthy()
    for (const nome of ['Desligada', 'No canto', 'Solta']) {
      expect(screen.getByRole('radio', { name: nome })).not.toBeNull()
    }
  })

  it('no canto mostra os 4 cantos e a dica', () => {
    const { onCanto } = monta('canto')
    expect(screen.getByRole('radiogroup', { name: 'Canto da câmera' })).not.toBeNull()
    expect(screen.getByText('A câmera fica nesse canto da sua tela.')).not.toBeNull()
    fireEvent.click(screen.getByRole('radio', { name: 'Superior esquerdo' }))
    expect(onCanto).toHaveBeenCalledWith('sup_esq')
  })

  it('solta e off não mostram os cantos', () => {
    monta('solta')
    expect(screen.queryByRole('radiogroup', { name: 'Canto da câmera' })).toBeNull()
    expect(screen.getByText('A câmera vai como uma transmissão à parte, ao lado da tela.')).not.toBeNull()
  })

  it('off não mostra cantos nem dica', () => {
    monta('off')
    expect(screen.queryByRole('radiogroup', { name: 'Canto da câmera' })).toBeNull()
    expect(screen.queryByText(/A câmera/)).toBeNull()
  })

  it('clicar nos jeitos chama onJeito', () => {
    const { onJeito } = monta('canto')
    fireEvent.click(screen.getByRole('radio', { name: 'Solta' }))
    expect(onJeito).toHaveBeenCalledWith('solta')
    fireEvent.click(screen.getByRole('radio', { name: 'Desligada' }))
    expect(onJeito).toHaveBeenCalledWith('off')
  })
})
