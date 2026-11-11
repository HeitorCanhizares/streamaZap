// Sincronização de planos: ao entrar, a cada 1 h e no foco enquanto espera o pagamento. Tarefa bridge-03.
import { carregarConfig, carregarAssinatura, atualizarDepoisDePagar } from './planoAcoes'
import { getPlanoEstado, setPlanoEstado } from './planoStore'

export const INTERVALO_PLANOS_MS = 3_600_000
/** Foco seguido de foco: no máximo uma atualização a cada 10 s. */
export const ESPERA_FOCO_MS = 10_000
export interface PlanosSync { aoEntrar(): void; aoSair(): void; parar(): void }

/** Dispara sem esperar; falha de uma chamada nunca derruba o timer nem vira rejeição solta. */
function disparar(f: () => Promise<void>): void {
  try {
    void Promise.resolve(f()).catch(() => undefined)
  } catch {
    /* ignora */
  }
}

export function iniciarPlanos(): PlanosSync {
  let comSessao = false
  let ultimoFoco = -Infinity

  disparar(carregarConfig)

  const timer = setInterval(() => {
    disparar(carregarConfig)
    if (comSessao) disparar(carregarAssinatura)
  }, INTERVALO_PLANOS_MS)

  // Ao voltar do navegador de pagamento, confere a assinatura (sem martelar o servidor).
  const aoFocar = (): void => {
    if (!getPlanoEstado().aguardandoPagamento) return
    const agora = Date.now()
    if (agora - ultimoFoco < ESPERA_FOCO_MS) return
    ultimoFoco = agora
    disparar(atualizarDepoisDePagar)
  }
  window.addEventListener('focus', aoFocar)

  return {
    aoEntrar() {
      comSessao = true
      disparar(carregarAssinatura)
    },
    aoSair() {
      comSessao = false
      setPlanoEstado({ assinatura: null, aguardandoPagamento: false, comprado: null, telaAberta: false, destaque: null })
    },
    parar() {
      clearInterval(timer)
      window.removeEventListener('focus', aoFocar)
    },
  }
}
