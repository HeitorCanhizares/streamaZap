// Tarefa shell-03
import type { ReactNode } from 'react'
import { pingTone } from '../../pingColor'
import type { PingTone } from '../../pingColor'
import type { SalaMetricsProps } from './types'

// Métricas da sala (ping, suavização, qualidade e bitrate) na barra de título.
// Porte fiel de docs/design/3-sala.html (linhas 46-50).

const COR_TOM: Record<PingTone, string> = {
  green: 'var(--verde)',
  yellow: 'var(--amarelo)',
  red: 'var(--vermelho-texto)',
}

export function SalaMetrics({ tile, share }: SalaMetricsProps): ReactNode {
  // Sem tile o ping ainda não chegou: mostra travessão apagado, sem " ms".
  const ping = tile ? Math.round(tile.ping_ms) : null
  const corPing = ping === null ? 'var(--texto-3)' : COR_TOM[pingTone(ping)]

  // A ponte ainda não informa o servidor, então 'sa-east' é fixo.
  const textoSuavizacao = tile ? `suav. ${tile.smoothing_ms} ms auto` : 'suav. —'

  let textoQualidade: string
  if (tile) {
    // Vírgula decimal, como no design de referência (6,2 Mbps).
    const mbps = (tile.kbps / 1000).toFixed(1).replace('.', ',')
    textoQualidade = `${tile.height}p${Math.round(tile.fps)} · ${mbps} Mbps`
  } else if (share) {
    textoQualidade = `${share.preset}`
  } else {
    textoQualidade = '—'
  }

  return (
    <span
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 14,
        fontFamily: 'var(--font-mono)',
        fontSize: 12,
        color: 'var(--texto-2)',
      }}
    >
      <span>
        {`via servidor sa-east · `}
        <span style={{ color: corPing }}>{ping === null ? '—' : `${ping} ms`}</span>
      </span>
      <span>{textoSuavizacao}</span>
      <span>{textoQualidade}</span>
    </span>
  )
}
