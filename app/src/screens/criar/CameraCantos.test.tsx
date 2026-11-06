import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { CameraCantos } from './CameraCantos'

afterEach(() => cleanup())

describe('CameraCantos', () => {
  it('renderiza 4 radios e marca o canto ativo', () => {
    render(<CameraCantos canto="sup_dir" onCanto={() => {}} />)
    expect(screen.getAllByRole('radio')).toHaveLength(4)
    for (const nome of ['Superior esquerdo', 'Inferior esquerdo', 'Inferior direito']) {
      expect(screen.getByRole('radio', { name: nome }).getAttribute('aria-checked')).toBe('false')
    }
    expect(screen.getByRole('radio', { name: 'Superior direito' }).getAttribute('aria-checked')).toBe('true')
  })

  it('clicar chama onCanto com o id', () => {
    const onCanto = vi.fn()
    render(<CameraCantos canto="sup_dir" onCanto={onCanto} />)
    fireEvent.click(screen.getByRole('radio', { name: 'Inferior esquerdo' }))
    expect(onCanto).toHaveBeenCalledWith('inf_esq')
  })

  it('desabilitado não chama onCanto', () => {
    const onCanto = vi.fn()
    render(<CameraCantos canto="sup_dir" onCanto={onCanto} disabled />)
    const b = screen.getByRole('radio', { name: 'Inferior esquerdo' })
    expect((b as HTMLButtonElement).disabled).toBe(true)
    fireEvent.click(b)
    expect(onCanto).not.toHaveBeenCalled()
  })
})
