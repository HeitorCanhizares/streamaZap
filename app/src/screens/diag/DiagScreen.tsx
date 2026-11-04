// Tela Diagnóstico (docs/design/5-diagnostico.html), composta pelos blocos da pasta.
import { useEffect, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import './diag.css'
import { useSnapshot } from '../../bridge/store'
import { montarChecks, calcularVeredito, montarRelatorio } from './diagModel'
import { Cabecalho } from './Cabecalho'
import { Resumo } from './Resumo'
import { CardCheck } from './CardCheck'
import { ComoLer } from './ComoLer'

export function DiagScreen(): ReactNode {
  const snap = useSnapshot()
  const [verificando, setVerificando] = useState(false)
  const [copiado, setCopiado] = useState(false)
  const timerVerif = useRef<ReturnType<typeof setTimeout> | null>(null)
  const timerCopia = useRef<ReturnType<typeof setTimeout> | null>(null)

  // Limpa os timers pendentes ao desmontar.
  useEffect(
    () => () => {
      if (timerVerif.current) clearTimeout(timerVerif.current)
      if (timerCopia.current) clearTimeout(timerCopia.current)
    },
    [],
  )

  const checks = montarChecks(snap)
  const semRede = snap.network.interfaces.length === 0
  const veredito = calcularVeredito(checks, verificando, semRede)

  const onRecheck = (): void => {
    setVerificando(true)
    if (timerVerif.current) clearTimeout(timerVerif.current)
    timerVerif.current = setTimeout(() => setVerificando(false), 1500)
  }

  const onCopy = async (): Promise<void> => {
    try {
      await navigator.clipboard.writeText(montarRelatorio(checks, veredito))
    } catch {
      // sem clipboard ou permissão negada: segue só com o feedback visual
    }
    setCopiado(true)
    if (timerCopia.current) clearTimeout(timerCopia.current)
    timerCopia.current = setTimeout(() => setCopiado(false), 1800)
  }

  return (
    <div
      className="zp-dots"
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: 20,
        padding: '28px 32px 36px',
        flex: '999 1 600px',
        minWidth: 0,
      }}
    >
      <Cabecalho copyLabel={copiado ? 'Copiado!' : 'Copiar relatório'} onRecheck={onRecheck} onCopy={() => void onCopy()} />
      <Resumo checks={checks} veredito={veredito} verificando={verificando} />
      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 20, alignItems: 'stretch' }}>
        {checks.map((c) => (
          <CardCheck key={c.id} check={c} verificando={verificando} />
        ))}
        <ComoLer />
      </div>
    </div>
  )
}
