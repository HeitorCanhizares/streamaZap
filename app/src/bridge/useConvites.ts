// Recebe os convites de sala (`streamazap://sala/<alvo>`, validados no Rust) e entrega um por vez.
import { useCallback, useEffect, useRef, useState } from 'react'
import { api } from './api'
import { useContaEstado } from './contaStore'

/** Um convite recebido; `seq` muda a cada convite, mesmo que o alvo se repita. */
export interface ConvitePedido { alvo: string; seq: number }

/** Quanto esperar a conta carregar antes de entregar o convite mesmo assim (ms). */
export const ESPERA_CONTA_MS = 3000

/**
 * Escuta `zap://convite` e pega os que chegaram com o app fechado (`convitesPendentes`). Só entrega
 * depois que a sessão da conta saiu de 'carregando' (para a sala Online já entrar com a conta), ou
 * depois de {@link ESPERA_CONTA_MS}, para não travar quem não usa conta. Vários seguidos: vale o
 * último. `limpar` marca o atual como tratado.
 */
export function useConvites(): { pedido: ConvitePedido | null; limpar: () => void } {
  const conta = useContaEstado()
  const [fila, setFila] = useState<string | null>(null)
  const [esperouDemais, setEsperouDemais] = useState(false)
  const [pedido, setPedido] = useState<ConvitePedido | null>(null)
  const seq = useRef(0)

  useEffect(() => {
    let desfazer: (() => void) | null = null
    let cancelado = false
    void api
      .escutarConvite((alvo) => setFila(alvo))
      .then((fn) => {
        if (cancelado) fn()
        else desfazer = fn
      })
      .catch(() => undefined)
    // Sem guarda de `cancelado`: a fila do Rust esvazia na primeira leitura, e no StrictMode o
    // estado sobrevive à remontagem.
    void api
      .convitesPendentes()
      .then((alvos) => {
        const ultimo = alvos[alvos.length - 1]
        if (ultimo !== undefined) setFila(ultimo)
      })
      .catch(() => undefined)
    const t = setTimeout(() => setEsperouDemais(true), ESPERA_CONTA_MS)
    return () => {
      cancelado = true
      desfazer?.()
      clearTimeout(t)
    }
  }, [])

  const pronto = conta.sessao !== 'carregando' || esperouDemais
  useEffect(() => {
    if (fila === null || !pronto) return
    seq.current += 1
    setPedido({ alvo: fila, seq: seq.current })
    setFila(null)
  }, [fila, pronto])

  const limpar = useCallback(() => setPedido(null), [])
  return { pedido, limpar }
}
