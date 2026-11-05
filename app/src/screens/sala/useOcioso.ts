// Controles da chamada que somem com o mouse parado: aparecem ao mexer o mouse no vídeo (ou
// apertar uma tecla) e somem depois de `espera` ms sem movimento, a não ser que o mouse esteja em
// cima deles ou o foco do teclado esteja neles.
import { useCallback, useEffect, useRef, useState } from 'react'

export interface Ocioso {
  /** Mostrar os controles agora. */
  visivel: boolean
  /** Mexeu o mouse no vídeo (ou apertou tecla): mostra e reinicia a contagem. */
  acordar: () => void
  /** O mouse saiu do vídeo: some logo. */
  sair: () => void
  /** Mouse em cima dos controles ou foco neles: ficam visíveis. */
  prender: (preso: boolean) => void
}

export function useOcioso(espera = 2500): Ocioso {
  const [mexeu, setMexeu] = useState(true)
  const [preso, setPreso] = useState(false)
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)

  const agendar = useCallback((ms: number) => {
    if (timer.current !== null) clearTimeout(timer.current)
    timer.current = setTimeout(() => setMexeu(false), ms)
  }, [])

  const acordar = useCallback(() => {
    setMexeu(true)
    agendar(espera)
  }, [agendar, espera])

  const sair = useCallback(() => agendar(600), [agendar])

  useEffect(() => {
    agendar(espera)
    return () => {
      if (timer.current !== null) clearTimeout(timer.current)
    }
  }, [agendar, espera])

  return { visivel: mexeu || preso, acordar, sair, prender: setPreso }
}
