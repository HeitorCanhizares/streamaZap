// Ações de planos (docs/planos/fase3-app-planos.md §4.4). Usam `clienteAtual()`; nunca lançam.
import { api } from './api'
import { clienteAtual } from './contaClient'
import { mensagemDaFuncao } from './contaErros'
import { nomePlano } from './contaModel'
import { CONFIG_DESLIGADA, lerAssinatura, lerConfigPublica, limparDocumento } from './planoModel'
import { getPlanoEstado, setPlanoEstado } from './planoStore'
import type { Ciclo, ConfigPublica, PlanoPago } from './planoTipos'

const AVISO_PAGAMENTO = 'Não foi possível abrir o pagamento.'

/** Aplica a config; sem cobrança, fecha a tela de planos e para de esperar pagamento. */
function aplicarConfig(config: ConfigPublica): void {
  if (config.cobrancaAtiva) {
    setPlanoEstado({ config })
  } else {
    setPlanoEstado({ config, telaAberta: false, destaque: null, aguardandoPagamento: false })
  }
}

/** Lê `config_publica`. Erro, exceção ou sem cliente → `CONFIG_DESLIGADA`. */
export async function carregarConfig(): Promise<void> {
  const cliente = clienteAtual()
  if (!cliente) {
    aplicarConfig(CONFIG_DESLIGADA)
    return
  }
  try {
    const { data, error } = await cliente.from('config_publica').select('cobranca_ativa,plano_padrao,precos').maybeSingle()
    aplicarConfig(error ? CONFIG_DESLIGADA : lerConfigPublica(data))
  } catch {
    aplicarConfig(CONFIG_DESLIGADA)
  }
}

/** Lê `assinaturas`; com `aguardandoPagamento` e plano comprado ativo, avisa. */
export async function carregarAssinatura(): Promise<void> {
  const cliente = clienteAtual()
  let assinatura = null
  if (cliente) {
    try {
      const { data, error } = await cliente.from('assinaturas').select('plano,status,ciclo,valido_ate').maybeSingle()
      assinatura = error ? null : lerAssinatura(data)
    } catch {
      assinatura = null
    }
  }
  setPlanoEstado({ assinatura })
  const estado = getPlanoEstado()
  if (
    estado.aguardandoPagamento &&
    assinatura !== null &&
    assinatura.status === 'ativa' &&
    assinatura.plano === estado.comprado &&
    new Date(assinatura.validoAte) > new Date()
  ) {
    setPlanoEstado({ aguardandoPagamento: false, info: `Plano atualizado: ${nomePlano({ id: assinatura.plano })}.` })
  }
}

/** Só com `config.cobrancaAtiva`. */
export function abrirPlanos(destaque?: PlanoPago): void {
  if (!getPlanoEstado().config.cobrancaAtiva) return
  setPlanoEstado({ telaAberta: true, destaque: destaque ?? null, aviso: null, info: null })
}

export function fecharPlanos(): void {
  setPlanoEstado({ telaAberta: false, destaque: null })
}

/** Chama a função `assinar` e abre o link. */
export async function assinar(plano: PlanoPago, ciclo: Ciclo, documento: string): Promise<void> {
  const cliente = clienteAtual()
  if (!cliente) {
    setPlanoEstado({ aviso: 'Login indisponível neste servidor.' })
    return
  }
  const doc = limparDocumento(documento)
  if (doc === null) {
    setPlanoEstado({ aviso: 'Informe um CPF ou CNPJ válido.' })
    return
  }
  setPlanoEstado({ ocupado: true, aviso: null, info: null })
  try {
    const body = doc !== '' ? { plano, ciclo, cpf_cnpj: doc } : { plano, ciclo }
    const { data, error } = await cliente.functions.invoke('assinar', { body })
    if (error) {
      setPlanoEstado({ aviso: await mensagemDaFuncao(error, 'Não foi possível assinar agora. Tente de novo.') })
      return
    }
    const link = (data as { link?: unknown } | null)?.link
    if (typeof link !== 'string' || !link.startsWith('https://')) {
      setPlanoEstado({ aviso: AVISO_PAGAMENTO })
      return
    }
    try {
      await api.abrirUrl(link)
    } catch {
      setPlanoEstado({ aviso: AVISO_PAGAMENTO })
      return
    }
    setPlanoEstado({ aguardandoPagamento: true, comprado: plano })
  } catch {
    setPlanoEstado({ aviso: 'Não foi possível assinar agora. Tente de novo.' })
  } finally {
    setPlanoEstado({ ocupado: false })
  }
}

/** `refreshSession()` + `carregarAssinatura()`. O token novo chega ao núcleo pelo `contaSync`. */
export async function atualizarDepoisDePagar(): Promise<void> {
  const cliente = clienteAtual()
  if (!cliente) return
  setPlanoEstado({ ocupado: true })
  try {
    try {
      await cliente.auth.refreshSession()
    } catch {
      // Sem renovar a sessão ainda dá para consultar a assinatura.
    }
    await carregarAssinatura()
    if (getPlanoEstado().aguardandoPagamento) {
      setPlanoEstado({ info: 'Ainda não recebemos a confirmação do pagamento. Tente de novo em alguns minutos.' })
    }
  } finally {
    setPlanoEstado({ ocupado: false })
  }
}

/** Para de esperar o pagamento. */
export function pararDeEsperar(): void {
  setPlanoEstado({ aguardandoPagamento: false, comprado: null })
}
