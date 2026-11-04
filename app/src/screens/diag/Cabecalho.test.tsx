// Tarefa diag-02
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { Cabecalho } from './Cabecalho'

afterEach(() => {
  cleanup()
})

describe('Cabecalho', () => {
  it('renderiza_textos', () => {
    render(<Cabecalho copyLabel="Copiar relatório" onRecheck={() => {}} onCopy={() => {}} />)
    expect(screen.getByText('Diagnóstico')).toBeTruthy()
    expect(screen.getByText('Tá tudo conectando?')).toBeTruthy()
    expect(screen.getByText('Verificar de novo')).toBeTruthy()
    expect(screen.getByText('Copiar relatório')).toBeTruthy()
  })

  it('copyLabel_muda', () => {
    render(<Cabecalho copyLabel="Copiado!" onRecheck={() => {}} onCopy={() => {}} />)
    expect(screen.getByText('Copiado!')).toBeTruthy()
  })

  it('cliques', () => {
    const onRecheck = vi.fn()
    const onCopy = vi.fn()
    render(<Cabecalho copyLabel="Copiar relatório" onRecheck={onRecheck} onCopy={onCopy} />)
    fireEvent.click(screen.getByText('Verificar de novo'))
    fireEvent.click(screen.getByText('Copiar relatório'))
    expect(onRecheck).toHaveBeenCalledTimes(1)
    expect(onCopy).toHaveBeenCalledTimes(1)
  })

  it('icones', () => {
    render(<Cabecalho copyLabel="Copiar relatório" onRecheck={() => {}} onCopy={() => {}} />)
    const recheck = screen.getByText('Verificar de novo').closest('button')
    const copy = screen.getByText('Copiar relatório').closest('button')
    expect(recheck?.querySelectorAll('svg')).toHaveLength(1)
    expect(copy?.querySelectorAll('svg')).toHaveLength(1)
    expect(recheck?.querySelector('svg')?.querySelectorAll('path')).toHaveLength(2)
  })
})
