// Tarefas config-02 e ui5-03
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { ConfigSobre } from './ConfigSobre'
import { mockSettings, mockSnapshot } from '../../bridge/mock'
import { api } from '../../bridge/api'
import { setSnapshot } from '../../bridge/store'
import { resetUpdateState, setUpdateState } from '../../bridge/updateStore'
import type { UpdateInfo } from '../../bridge/types'

vi.mock('../../bridge/api', () => ({
  api: {
    updateCheck: vi.fn(),
    updateDownload: vi.fn().mockResolvedValue(undefined),
    updateInstallNow: vi.fn().mockResolvedValue(undefined),
    updateInstallOnExit: vi.fn().mockResolvedValue(undefined),
  },
  isTauri: () => false,
}))

let versao = 1000
// Troca o snapshot com ou sem sala (versão sempre crescente, senão o store ignora).
function salaAberta(aberta: boolean): void {
  const s = mockSnapshot()
  setSnapshot({ ...s, room: aberta ? s.room : null, version: ++versao })
}

beforeEach(() => {
  vi.clearAllMocks()
  resetUpdateState()
  salaAberta(false)
})

afterEach(() => {
  cleanup()
})

describe('ConfigSobre', () => {
  it('renderiza o título, a versão e a logo', () => {
    render(<ConfigSobre settings={mockSettings()} onChange={vi.fn()} />)
    expect(screen.getByRole('heading', { name: 'Sobre e atualizações' })).toBeTruthy()
    // O nome fica dividido em spans (Zap em verde), então a busca compara o texto completo do elemento.
    expect(screen.getByText((_, el) => el?.textContent === 'streamaZap')).toBeTruthy()
    expect(screen.getByText('versão 3.1.2 · protocolo 3 · licença MIT')).toBeTruthy()
    expect(screen.getByAltText('Logo do streamaZap')).toBeTruthy()
  })

  it('alternar o switch de atualização preserva o canal', () => {
    const onChange = vi.fn()
    const s = { ...mockSettings(), update: { auto: true, channel: 'beta' as const } }
    render(<ConfigSobre settings={s} onChange={onChange} />)
    const sw = screen.getByRole('switch')
    expect(sw.getAttribute('aria-checked')).toBe('true')
    fireEvent.click(sw)
    expect(onChange).toHaveBeenCalledWith({ update: { auto: false, channel: 'beta' } })
  })

  it('trocar o canal chama onChange com o update.channel novo', () => {
    const onChange = vi.fn()
    render(<ConfigSobre settings={{ ...mockSettings(), update: { auto: true, channel: 'estavel' } }} onChange={onChange} />)
    fireEvent.change(screen.getByRole('combobox'), { target: { value: 'beta' } })
    expect(onChange).toHaveBeenCalledWith({ update: { auto: true, channel: 'beta' } })
  })

  it('Procurar atualização mostra a busca e depois a nova versão com botão Baixar', async () => {
    let resolve!: (i: UpdateInfo) => void
    vi.mocked(api.updateCheck).mockReturnValue(new Promise((r) => { resolve = r }))
    render(<ConfigSobre settings={mockSettings()} onChange={vi.fn()} />)
    expect(screen.getByText('Você está na versão mais nova')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Procurar atualização' }))
    expect(screen.getByText('Procurando nas Releases…')).toBeTruthy()
    await act(async () => { resolve({ disponivel: true, versao: '3.0.1', notas: null, tamanho: null }) })
    expect(screen.getByText('Nova versão 3.0.1 disponível')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Baixar' }))
    expect(api.updateDownload).toHaveBeenCalled()
  })

  it('sem atualização mostra versão mais nova', async () => {
    vi.mocked(api.updateCheck).mockResolvedValue({ disponivel: false, versao: null, notas: null, tamanho: null })
    render(<ConfigSobre settings={mockSettings()} onChange={vi.fn()} />)
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Procurar atualização' })) })
    expect(screen.getByText('Você está na versão mais nova')).toBeTruthy()
  })

  it('falha na busca mostra o erro em role=alert', async () => {
    vi.mocked(api.updateCheck).mockRejectedValue(new Error('atualização não configurada nesta build'))
    render(<ConfigSobre settings={mockSettings()} onChange={vi.fn()} />)
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Procurar atualização' })) })
    expect(screen.getByRole('alert').textContent).toBe('atualização não configurada nesta build')
  })

  it('baixando mostra porcentagem e barra', () => {
    setUpdateState({ fase: 'baixando', progresso: 40, versao: '3.0.1', erro: null })
    render(<ConfigSobre settings={mockSettings()} onChange={vi.fn()} />)
    expect(screen.getByText(/40%/)).toBeTruthy()
    expect(screen.getByRole('progressbar').getAttribute('aria-valuenow')).toBe('40')
    expect((screen.getByRole('button', { name: 'Procurar atualização' }) as HTMLButtonElement).disabled).toBe(true)
  })

  it('pronta: Reiniciar agora fora de sala chama updateInstallNow', () => {
    salaAberta(false)
    setUpdateState({ fase: 'pronta', progresso: 100, versao: '3.0.1', erro: null })
    render(<ConfigSobre settings={mockSettings()} onChange={vi.fn()} />)
    expect(screen.getByText('Atualização 3.0.1 pronta — instala ao fechar')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Reiniciar agora' }))
    expect(api.updateInstallNow).toHaveBeenCalled()
  })

  it('pronta em sala: Reiniciar agora fica desabilitado', () => {
    salaAberta(true)
    setUpdateState({ fase: 'pronta', progresso: 100, versao: '3.0.1', erro: null })
    render(<ConfigSobre settings={mockSettings()} onChange={vi.fn()} />)
    const b = screen.getByRole('button', { name: 'Reiniciar agora' }) as HTMLButtonElement
    expect(b.disabled).toBe(true)
    expect(b.title).toBe('Saia da sala para reiniciar')
  })

  it('Instalar ao fechar chama a api e troca o selo', async () => {
    salaAberta(false)
    setUpdateState({ fase: 'pronta', progresso: 100, versao: '3.0.1', erro: null })
    render(<ConfigSobre settings={mockSettings()} onChange={vi.fn()} />)
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Instalar ao fechar' })) })
    expect(api.updateInstallOnExit).toHaveBeenCalled()
    expect(screen.getByText('Vai instalar ao fechar o app')).toBeTruthy()
  })
})
