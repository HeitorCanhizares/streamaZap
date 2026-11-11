// Clique numa qualidade travada. Tarefa telas-02.
import { useCallback, useState } from 'react'
import { abrirPlanos } from '../../bridge/planoAcoes'
import { useCobrancaAtiva } from '../../bridge/usePlano'
import type { PresetLock } from '../../bridge/types'

export interface CliqueTravado { aoClicar(lock: PresetLock): void; explicacao: string | null }

/** Com cobrança: abre a tela de planos já destacando o plano que destrava. Sem cobrança: só explica. */
export function useCliqueTravado(): CliqueTravado {
  const cobranca = useCobrancaAtiva()
  const [explicacao, setExplicacao] = useState<string | null>(null)
  const aoClicar = useCallback(
    (lock: PresetLock) => {
      if (cobranca) {
        setExplicacao(null)
        abrirPlanos(lock.needs === 'fullhd' ? 'fullhd' : 'hd')
      } else {
        setExplicacao('Seu plano não permite essa qualidade')
      }
    },
    [cobranca],
  )
  return { aoClicar, explicacao }
}
