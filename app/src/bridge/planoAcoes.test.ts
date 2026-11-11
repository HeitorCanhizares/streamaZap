import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from './api'
import { clienteAtual } from './contaClient'
import {
  abrirPlanos,
  assinar,
  atualizarDepoisDePagar,
  carregarAssinatura,
  carregarConfig,
  fecharPlanos,
  pararDeEsperar,
} from './planoAcoes'
import { CONFIG_DESLIGADA } from './planoModel'
import { getPlanoEstado, resetPlano, setPlanoEstado } from './planoStore'
import type { ContaClient } from './contaTipos'

vi.mock('./contaClient', () => ({ clienteAtual: vi.fn() }))

const PRECOS = {
  espectador: { mensal: 390, anual: 3900 },
  hd: { mensal: 1490, anual: 14900 },
  fullhd: { mensal: 2990, anual: 29900 },
}
const LINHA_CONFIG = { cobranca_ativa: true, plano_padrao: 'nenhum', precos: PRECOS }

function daquiA30Dias(): string {
  return new Date(Date.now() + 30 * 86400000).toISOString()
}

function fake() {
  const resp = {
    config: { data: LINHA_CONFIG as unknown, error: null as unknown },
    assinatura: { data: null as unknown, error: null as unknown },
  }
  const from = vi.fn((tabela: string) => ({
    select: vi.fn((_c: string) => ({
      maybeSingle: async () => (tabela === 'config_publica' ? resp.config : resp.assinatura),
    })),
  }))
  const invoke = vi.fn(async (_n: string, _o?: { body?: unknown }) => ({
    data: { link: 'https://pagamento.exemplo.com/x' } as unknown,
    error: null as unknown,
  }))
  const refreshSession = vi.fn(async () => ({ data: { session: null }, error: null }))
  const cliente = { from, functions: { invoke }, auth: { refreshSession } } as unknown as ContaClient
  return { resp, from, invoke, refreshSession, cliente }
}

let f: ReturnType<typeof fake>
let abrir: ReturnType<typeof vi.spyOn>

beforeEach(() => {
  resetPlano()
  f = fake()
  vi.mocked(clienteAtual).mockReturnValue(f.cliente)
  abrir = vi.spyOn(api, 'abrirUrl').mockResolvedValue(undefined)
})

describe('carregarConfig', () => {
  it('linha completa liga a cobrança com preços', async () => {
    await carregarConfig()
    expect(getPlanoEstado().config.cobrancaAtiva).toBe(true)
    expect(getPlanoEstado().config.precos).toEqual(PRECOS)
  })

  it('erro vira CONFIG_DESLIGADA', async () => {
    await carregarConfig()
    f.resp.config = { data: null, error: { message: 'x' } }
    await carregarConfig()
    expect(getPlanoEstado().config.cobrancaAtiva).toBe(false)
    expect(getPlanoEstado().config.precos).toBeNull()
  })

  it('sem cliente vira CONFIG_DESLIGADA', async () => {
    await carregarConfig()
    expect(getPlanoEstado().config.cobrancaAtiva).toBe(true)
    vi.mocked(clienteAtual).mockReturnValue(null)
    await carregarConfig()
    expect(getPlanoEstado().config).toBe(CONFIG_DESLIGADA)
  })

  it('cobrança desligando fecha a tela e para de esperar', async () => {
    await carregarConfig()
    setPlanoEstado({ telaAberta: true, destaque: 'hd', aguardandoPagamento: true })
    f.resp.config = { data: { cobranca_ativa: false, plano_padrao: 'hd', precos: null }, error: null }
    await carregarConfig()
    const e = getPlanoEstado()
    expect(e.telaAberta).toBe(false)
    expect(e.destaque).toBeNull()
    expect(e.aguardandoPagamento).toBe(false)
  })
})

describe('abrirPlanos e fecharPlanos', () => {
  it('sem cobrança não abre', () => {
    abrirPlanos('hd')
    expect(getPlanoEstado().telaAberta).toBe(false)
  })

  it('com cobrança abre com destaque e fecha', async () => {
    await carregarConfig()
    abrirPlanos('hd')
    expect(getPlanoEstado().telaAberta).toBe(true)
    expect(getPlanoEstado().destaque).toBe('hd')
    fecharPlanos()
    expect(getPlanoEstado().telaAberta).toBe(false)
    expect(getPlanoEstado().destaque).toBeNull()
  })
})

describe('carregarAssinatura', () => {
  it('linha ativa vira Assinatura', async () => {
    const validoAte = daquiA30Dias()
    f.resp.assinatura = { data: { plano: 'hd', status: 'ativa', ciclo: 'mensal', valido_ate: validoAte }, error: null }
    await carregarAssinatura()
    expect(getPlanoEstado().assinatura).toEqual({ plano: 'hd', status: 'ativa', ciclo: 'mensal', validoAte })
  })

  it('erro vira null', async () => {
    setPlanoEstado({
      assinatura: { plano: 'hd', status: 'ativa', ciclo: 'mensal', validoAte: daquiA30Dias() },
    })
    f.resp.assinatura = { data: null, error: { message: 'x' } }
    await carregarAssinatura()
    expect(getPlanoEstado().assinatura).toBeNull()
  })

  it('aguardando pagamento com plano comprado ativo avisa', async () => {
    setPlanoEstado({ aguardandoPagamento: true, comprado: 'hd' })
    f.resp.assinatura = {
      data: { plano: 'hd', status: 'ativa', ciclo: 'anual', valido_ate: daquiA30Dias() },
      error: null,
    }
    await carregarAssinatura()
    expect(getPlanoEstado().aguardandoPagamento).toBe(false)
    expect(getPlanoEstado().info).toBe('Plano atualizado: Transmissor HD.')
  })
})

describe('assinar', () => {
  it('sem documento manda só plano e ciclo, abre o link e espera', async () => {
    await assinar('hd', 'anual', '')
    expect(f.invoke).toHaveBeenCalledWith('assinar', { body: { plano: 'hd', ciclo: 'anual' } })
    const corpo = f.invoke.mock.calls[0][1]?.body as object
    expect(Object.keys(corpo)).toEqual(['plano', 'ciclo'])
    expect(abrir).toHaveBeenCalledWith('https://pagamento.exemplo.com/x')
    expect(getPlanoEstado().aguardandoPagamento).toBe(true)
    expect(getPlanoEstado().comprado).toBe('hd')
    expect(getPlanoEstado().ocupado).toBe(false)
  })

  it('CPF formatado vira só dígitos', async () => {
    await assinar('hd', 'mensal', '123.456.789-09')
    expect(f.invoke).toHaveBeenCalledWith('assinar', {
      body: { plano: 'hd', ciclo: 'mensal', cpf_cnpj: '12345678909' },
    })
  })

  it('documento inválido avisa sem chamar a função', async () => {
    await assinar('hd', 'mensal', '123456789012')
    expect(getPlanoEstado().aviso).toBe('Informe um CPF ou CNPJ válido.')
    expect(f.invoke).not.toHaveBeenCalled()
  })

  it('erro da função mostra a mensagem do servidor', async () => {
    f.invoke.mockResolvedValueOnce({
      data: null,
      error: {
        message: 'x',
        context: { json: async () => ({ erro: 'cobranca_desligada', mensagem: 'As assinaturas ainda não estão abertas.' }) },
      },
    })
    await assinar('hd', 'mensal', '')
    expect(getPlanoEstado().aviso).toBe('As assinaturas ainda não estão abertas.')
    expect(abrir).not.toHaveBeenCalled()
    expect(getPlanoEstado().ocupado).toBe(false)
  })

  it('link que não é https avisa', async () => {
    f.invoke.mockResolvedValueOnce({ data: { link: 'http://x' }, error: null })
    await assinar('hd', 'mensal', '')
    expect(getPlanoEstado().aviso).toBe('Não foi possível abrir o pagamento.')
    expect(abrir).not.toHaveBeenCalled()
    expect(getPlanoEstado().aguardandoPagamento).toBe(false)
  })
})

describe('atualizarDepoisDePagar', () => {
  it('assinatura nova ativa avisa', async () => {
    setPlanoEstado({ aguardandoPagamento: true, comprado: 'hd' })
    f.resp.assinatura = {
      data: { plano: 'hd', status: 'ativa', ciclo: 'mensal', valido_ate: daquiA30Dias() },
      error: null,
    }
    await atualizarDepoisDePagar()
    expect(f.refreshSession).toHaveBeenCalledTimes(1)
    expect(f.from).toHaveBeenCalledWith('assinaturas')
    expect(getPlanoEstado().info).toBe('Plano atualizado: Transmissor HD.')
    expect(getPlanoEstado().ocupado).toBe(false)
  })

  it('sem assinatura nova pede para tentar de novo', async () => {
    setPlanoEstado({ aguardandoPagamento: true, comprado: 'hd' })
    await atualizarDepoisDePagar()
    expect(getPlanoEstado().info).toBe(
      'Ainda não recebemos a confirmação do pagamento. Tente de novo em alguns minutos.',
    )
    expect(getPlanoEstado().aguardandoPagamento).toBe(true)
    expect(getPlanoEstado().ocupado).toBe(false)
  })
})

describe('pararDeEsperar', () => {
  it('zera aguardandoPagamento', () => {
    setPlanoEstado({ aguardandoPagamento: true, comprado: 'hd' })
    pararDeEsperar()
    expect(getPlanoEstado().aguardandoPagamento).toBe(false)
    expect(getPlanoEstado().comprado).toBeNull()
  })
})

describe('nunca lançam', () => {
  it('cliente que joga exceção', async () => {
    const boom = () => {
      throw new Error('boom')
    }
    const ruim = {
      from: boom,
      functions: { invoke: vi.fn(async () => boom()) },
      auth: { refreshSession: vi.fn(async () => boom()) },
    } as unknown as ContaClient
    vi.mocked(clienteAtual).mockReturnValue(ruim)
    await expect(carregarConfig()).resolves.toBeUndefined()
    await expect(carregarAssinatura()).resolves.toBeUndefined()
    await expect(assinar('hd', 'mensal', '')).resolves.toBeUndefined()
    await expect(atualizarDepoisDePagar()).resolves.toBeUndefined()
    expect(getPlanoEstado().config.cobrancaAtiva).toBe(false)
    expect(getPlanoEstado().ocupado).toBe(false)
  })
})
