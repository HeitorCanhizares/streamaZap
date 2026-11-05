// Lugar onde o canvas de uma transmissão aparece. O canvas é do useAssinaturas e só muda de pai no
// DOM: trocar a transmissão de lugar não reinicia o vídeo.
import { useLayoutEffect, useRef, type CSSProperties, type ReactNode } from 'react'

export interface CanvasSlotProps { canvas: HTMLCanvasElement; label?: string; style?: CSSProperties }

export function CanvasSlot({ canvas, label, style }: CanvasSlotProps): ReactNode {
  const ref = useRef<HTMLDivElement | null>(null)
  useLayoutEffect(() => {
    const el = ref.current
    if (!el) return
    el.appendChild(canvas)
    return () => {
      // Se outro lugar já pegou o canvas, não tira de lá.
      if (canvas.parentNode === el) el.removeChild(canvas)
    }
  }, [canvas])
  return <div ref={ref} role="img" aria-label={label} style={{ position: 'absolute', inset: 0, ...style }} />
}
