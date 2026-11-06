// Tela Diagnóstico (docs/design/5-diagnostico.html), composta pelos blocos da pasta.
import { useEffect, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import './diag.css'
import { useSnapshot } from '../../bridge/store'
import { api } from '../../bridge/api'
import { useOnlineEnabled } from '../../bridge/onlineStore'
import type { DiagReport } from '../../bridge/types'
import { calcularVeredito, montarRelatorio } from './diagModel'
import { montarChecksReal } from './diagReal'
import { Cabecalho } from './Cabecalho'
import { Resumo } from './Resumo'
import { CardCheck } from './CardCheck'
import { ComoLer } from './ComoLer'

export function DiagScreen(): ReactNode {
  const snap = useSnapshot()
  const online = useOnlineEnabled()
  const [rel, setRel] = useState<DiagReport | null>(null)
  const [verificando, setVerificando] = useState(false)
  const [copiado, setCopiado] = useState(false)
  const [erroDiag, setErroDiag] = useState<string | null>(null)
  const [corrigindo, setCorrigindo] = useState(false)
  const [resultadoFw, setResultadoFw] = useState<{ ok: boolean; mensagem: string } | null>(null)
  const timerCopia = useRef<ReturnType<typeof setTimeout> | null>(null)
  const vivo = useRef(true)

  // Roda o diagnóstico; em falha mantém o relatório anterior e guarda o erro.
  const rodar = async (): Promise<void> => {
    try {
      const r = await api.diagnosticsRun()
      if (!vivo.current) return
      setRel(r)
      setErroDiag(null)
    } catch (e) {
      if (vivo.current) setErroDiag(e instanceof Error ? e.message : String(e))
    }
  }

  useEffect(() => {
    vivo.current = true
    void rodar()
    return () => {
      vivo.current = false
      if (timerCopia.current) clearTimeout(timerCopia.current)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const checks = montarChecksReal(rel, snap, online)
  const semRede = (rel ? rel.network.interfaces.length : snap.network.interfaces.length) === 0
  const veredito = calcularVeredito(checks, verificando, semRede, online)

  const onRecheck = async (): Promise<void> => {
    setVerificando(true)
    await rodar()
    if (vivo.current) setVerificando(false)
  }

  const onCopy = async (): Promise<void> => {
    let texto: string
    try {
      texto = await api.diagnosticsText()
    } catch {
      texto = montarRelatorio(checks, veredito)
    }
    try {
      await navigator.clipboard.writeText(texto)
    } catch {
      // sem clipboard ou permissão negada: segue só com o feedback visual
    }
    setCopiado(true)
    if (timerCopia.current) clearTimeout(timerCopia.current)
    timerCopia.current = setTimeout(() => setCopiado(false), 1800)
  }

  const onFixFirewall = async (): Promise<void> => {
    setCorrigindo(true)
    setResultadoFw(null)
    try {
      const r = await api.firewallFix()
      if (vivo.current) setResultadoFw({ ok: r.ok, mensagem: r.mensagem })
    } catch (e) {
      if (vivo.current) setResultadoFw({ ok: false, mensagem: e instanceof Error ? e.message : String(e) })
    }
    await rodar()
    if (vivo.current) setCorrigindo(false)
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
      <Cabecalho copyLabel={copiado ? 'Copiado!' : 'Copiar relatório'} onRecheck={() => void onRecheck()} onCopy={() => void onCopy()} />
      {erroDiag && (
        <div role="alert" style={{ color: '#FF8592', fontSize: 13.5 }}>
          Não foi possível rodar o diagnóstico: {erroDiag}
        </div>
      )}
      {resultadoFw && (
        <div
          role={resultadoFw.ok ? 'status' : 'alert'}
          style={{ color: resultadoFw.ok ? '#3BF08A' : '#FF8592', fontSize: 13.5 }}
        >
          {resultadoFw.mensagem}
        </div>
      )}
      <Resumo checks={checks} veredito={veredito} verificando={verificando} />
      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 20, alignItems: 'stretch' }}>
        {checks.map((c) => (
          <CardCheck key={c.id} check={c} verificando={verificando} onFixFirewall={() => void onFixFirewall()} corrigindo={corrigindo} />
        ))}
        <ComoLer online={online} />
      </div>
    </div>
  )
}
