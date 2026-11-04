// Tarefa diag-03
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import { Resumo } from './Resumo'
import type { Check, Veredito } from './diagModel'

afterEach(() => {
  cleanup()
})

const checks: Check[] = [
  { id: 'srv', title: 'Servidor online', short: 'Servidor', status: 'soon', lines: [] },
  { id: 'net', title: 'Rede', short: 'Rede', status: 'ok', lines: [] },
  { id: 'fw', title: 'Firewall do Windows', short: 'Firewall', status: 'soon', lines: [] },
  { id: 'peers', title: 'Amigos com o app aberto', short: 'Amigos', status: 'warn', lines: [] },
  { id: 'rooms', title: 'Salas encontradas', short: 'Salas', status: 'ok', lines: [] },
]

const veredito: Veredito = {
  titulo: 'Quase lá · 1 aviso',
  sub: 'Dá pra entrar nas salas, mas alguns amigos podem não te ver. Veja os itens em amarelo.',
  cor: '#FFB547',
  avisos: 1,
}

describe('Resumo', () => {
  it('mostra_titulo_e_versao', () => {
    render(<Resumo checks={checks} veredito={veredito} verificando={false} />)
    expect(screen.getByText('Quase lá · 1 aviso')).toBeTruthy()
    expect(screen.getByText('streamaZap 0.1.0 · protocolo 3 · Windows 11 64 bits')).toBeTruthy()
  })

  it('lista_os_5_selos', () => {
    render(<Resumo checks={checks} veredito={veredito} verificando={false} />)
    const itens = screen.getAllByRole('listitem')
    expect(itens).toHaveLength(5)
    expect(itens.map((li) => li.textContent)).toEqual(['Servidor', 'Rede', 'Firewall', 'Amigos', 'Salas'])
  })

  it('selo_de_amigos_e_warn', () => {
    render(<Resumo checks={checks} veredito={veredito} verificando={false} />)
    const amigos = screen.getByText('Amigos')
    expect(amigos.getAttribute('data-estado')).toBe('warn')
    expect(screen.getByText('Rede').getAttribute('data-estado')).toBe('ok')
    expect(screen.getByText('Servidor').getAttribute('data-estado')).toBe('soon')
  })

  it('verificando_marca_todos_como_checking', () => {
    render(<Resumo checks={checks} veredito={veredito} verificando={true} />)
    const itens = screen.getAllByRole('listitem')
    expect(itens).toHaveLength(5)
    for (const li of itens) {
      expect(li.getAttribute('data-estado')).toBe('checking')
    }
  })

  it('section_tem_aria_label', () => {
    render(<Resumo checks={checks} veredito={veredito} verificando={false} />)
    expect(screen.getByLabelText('Resumo').tagName).toBe('SECTION')
  })

  it('anel_tem_pulso_e_cor_do_veredito', () => {
    const { container } = render(<Resumo checks={checks} veredito={veredito} verificando={false} />)
    const anel = container.querySelector('.zp-pulse') as HTMLElement | null
    expect(anel).not.toBeNull()
    // jsdom normaliza a cor hex para rgb().
    expect(anel?.style.border).toContain('rgb(255, 181, 71)')
  })
})
