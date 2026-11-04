// Tarefa diag-04
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { CardCheck } from './CardCheck'
import type { Check } from './diagModel'

afterEach(() => {
  cleanup()
})

const net: Check = {
  id: 'net', title: 'Rede', short: 'Rede', status: 'ok',
  lines: [{ tom: 'ok', text: 'Ethernet' }, { tom: 'ok', text: 'Radmin VPN · 26.1.1.1' }],
}

describe('CardCheck', () => {
  it('ok_com_linhas', () => {
    render(<CardCheck check={net} verificando={false} />)
    expect(screen.getByText('Rede')).toBeTruthy()
    expect(screen.getByText('Tudo certo')).toBeTruthy()
    expect(screen.getByText('Ethernet')).toBeTruthy()
    expect(screen.getByText('Radmin VPN · 26.1.1.1')).toBeTruthy()
  })

  it('warn_com_sub', () => {
    const c: Check = { ...net, status: 'warn', lines: [{ tom: 'warn', text: 'Radmin não detectado', sub: 'O modo Online continua funcionando sem ele.' }] }
    render(<CardCheck check={c} verificando={false} />)
    expect(screen.getByText('Atenção')).toBeTruthy()
    expect(screen.getByText('O modo Online continua funcionando sem ele.')).toBeTruthy()
  })

  it('peers_badges', () => {
    const c: Check = {
      id: 'peers', title: 'Amigos', short: 'Amigos', status: 'warn',
      lines: [
        { tom: 'ok', text: 'Ju', badge: 'recebe você' },
        { tom: 'bad', text: 'Dudu', badge: 'não recebe', sub: 'Não recebe nada do seu PC' },
      ],
    }
    render(<CardCheck check={c} verificando={false} />)
    expect(screen.getByText('recebe você')).toBeTruthy()
    expect(screen.getByText('não recebe')).toBeTruthy()
    expect(screen.getByText('Não recebe nada do seu PC')).toBeTruthy()
  })

  it('soon', () => {
    const c: Check = { id: 'srv', title: 'Servidor online', short: 'Servidor', status: 'soon', lines: [{ tom: 'info', text: 'Disponível em breve' }] }
    render(<CardCheck check={c} verificando={false} />)
    expect(screen.getByText('em breve')).toBeTruthy()
    expect(screen.getByText('Disponível em breve')).toBeTruthy()
  })

  it('verificando', () => {
    const { container } = render(<CardCheck check={net} verificando={true} />)
    expect(screen.getByText('Verificando…')).toBeTruthy()
    expect(container.querySelector('svg.zp-spin')).toBeTruthy()
  })

  it('icone_ok', () => {
    const { container } = render(<CardCheck check={net} verificando={false} />)
    expect(container.querySelector('path[d="m5 12 5 5 9-10"]')).toBeTruthy()
  })

  it('dot_ok_tem_glow', () => {
    render(<CardCheck check={net} verificando={false} />)
    const dot = screen.getAllByTestId('dot')[0] as HTMLElement
    expect(dot.style.boxShadow).toBeTruthy()
  })

  it('botao_firewall_so_em_fw_warn', () => {
    const onFix = vi.fn()
    const fw: Check = { id: 'fw', title: 'Firewall', short: 'Firewall', status: 'warn', lines: [{ tom: 'warn', text: 'x' }] }
    render(<CardCheck check={fw} verificando={false} onFixFirewall={onFix} />)
    fireEvent.click(screen.getByText('Corrigir firewall'))
    expect(onFix).toHaveBeenCalledTimes(1)
    cleanup()
    render(<CardCheck check={{ ...fw, status: 'ok' }} verificando={false} onFixFirewall={onFix} />)
    expect(screen.queryByText('Corrigir firewall')).toBeNull()
    cleanup()
    render(<CardCheck check={{ ...net, status: 'warn' }} verificando={false} onFixFirewall={onFix} />)
    expect(screen.queryByText('Corrigir firewall')).toBeNull()
  })

  it('botao_firewall_corrigindo', () => {
    const fw: Check = { id: 'fw', title: 'Firewall', short: 'Firewall', status: 'warn', lines: [] }
    render(<CardCheck check={fw} verificando={false} corrigindo />)
    const b = screen.getByText('Corrigindo…') as HTMLButtonElement
    expect(b.disabled).toBe(true)
  })
})
