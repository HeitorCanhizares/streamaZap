import { afterEach, describe, expect, it } from 'vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { AtualizadoCard } from './AtualizadoCard'
import { resetUpdateAviso, setUpdateAplicada } from '../../bridge/updateAvisoStore'

afterEach(() => {
  cleanup()
  act(() => resetUpdateAviso())
})

const notas = '- Câmera solta\n- Atualização sozinha'

describe('AtualizadoCard', () => {
  it('sem estado não renderiza', () => {
    render(<AtualizadoCard />)
    expect(screen.queryByTestId('atualizado-card')).toBeNull()
  })

  it('mostra versão, anterior e notas', () => {
    render(<AtualizadoCard />)
    act(() => setUpdateAplicada({ versao: '3.4.0', anterior: '3.3.0', notas }))
    expect(screen.getByText('Atualizado para a v3.4.0')).toBeTruthy()
    expect(screen.getByText('Antes: v3.3.0')).toBeTruthy()
    expect(screen.getByText(/Câmera solta/)).toBeTruthy()
    expect(screen.getByText(/Atualização sozinha/)).toBeTruthy()
  })

  it('sem notas não mostra o bloco', () => {
    const { container } = render(<AtualizadoCard />)
    act(() => setUpdateAplicada({ versao: '3.4.0', anterior: '3.3.0', notas: null }))
    expect(screen.queryByText(/Câmera solta/)).toBeNull()
    expect(container.querySelector('pre')).toBeNull()
  })

  it('Fechar remove o cartão', () => {
    render(<AtualizadoCard />)
    act(() => setUpdateAplicada({ versao: '3.4.0', anterior: '3.3.0', notas }))
    fireEvent.click(screen.getByLabelText('Fechar aviso de atualização'))
    expect(screen.queryByTestId('atualizado-card')).toBeNull()
  })

  it('versão com v não duplica', () => {
    render(<AtualizadoCard />)
    act(() => setUpdateAplicada({ versao: 'v3.4.0', anterior: '3.3.0', notas: null }))
    expect(screen.getByText('Atualizado para a v3.4.0')).toBeTruthy()
    expect(screen.queryByText(/vv3/)).toBeNull()
  })
})
