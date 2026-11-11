// Tela de planos (diálogo). Tarefa telas-05.
import { useEffect, useState, type ReactNode } from 'react'
import { Button } from '../../components/Button'
import { SeloPlano } from '../../components/SeloPlano'
import { assinar, atualizarDepoisDePagar, fecharPlanos, pararDeEsperar } from '../../bridge/planoAcoes'
import { formatarPreco, mesesDeGraca } from '../../bridge/planoModel'
import { usePlanoEstado } from '../../bridge/planoStore'
import type { Ciclo, PlanoPago, Precos } from '../../bridge/planoTipos'
import { useSnapshot } from '../../bridge/store'

const CARTOES: { id: PlanoPago; nome: string; descricao: string }[] = [
  { id: 'espectador', nome: 'Espectador', descricao: 'Entra nas salas Online e assiste a tudo o que transmitirem' },
  { id: 'hd', nome: 'Transmissor HD', descricao: 'Tudo do Espectador, e transmite em HD' },
  { id: 'fullhd', nome: 'Transmissor Full HD', descricao: 'Tudo do Espectador, e transmite em Full HD' },
]

function textoCiclo(precos: Precos, plano: PlanoPago, ciclo: Ciclo): string {
  return ciclo === 'anual' ? `${formatarPreco(precos[plano].anual)}/ano` : `${formatarPreco(precos[plano].mensal)}/mês`
}

const estiloOpcao = (ativa: boolean) =>
  ({
    padding: '6px 14px',
    borderRadius: 999,
    border: `1px solid ${ativa ? 'var(--verde)' : 'var(--borda)'}`,
    background: 'transparent',
    color: 'inherit',
    cursor: 'pointer',
  }) as const

export function PlanosModal(): ReactNode {
  const estado = usePlanoEstado()
  const snap = useSnapshot()
  const [ciclo, setCiclo] = useState<Ciclo>('anual')
  const [documento, setDocumento] = useState('')
  const aberto = estado.telaAberta && estado.config.cobrancaAtiva && estado.config.precos !== null

  useEffect(() => {
    if (!aberto) return
    const aoTeclar = (e: KeyboardEvent) => {
      if (e.key === 'Escape') fecharPlanos()
    }
    window.addEventListener('keydown', aoTeclar)
    return () => window.removeEventListener('keydown', aoTeclar)
  }, [aberto])

  const precos = estado.config.precos
  if (!aberto || precos === null) return null

  const { assinatura } = estado
  const temPago =
    assinatura !== null &&
    (assinatura.status === 'ativa' || assinatura.status === 'atrasada') &&
    new Date(assinatura.validoAte) > new Date()
  const atual = temPago ? assinatura.plano : (snap.account?.plan?.id ?? null)
  const meses = mesesDeGraca(precos, 'hd')

  return (
    <div style={{ position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.6)', zIndex: 50, display: 'grid', placeItems: 'center' }}>
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Planos do Online"
        style={{ width: 'min(860px, 92vw)', padding: 24, borderRadius: 18, background: 'var(--superficie)', border: '1px solid var(--borda)', maxHeight: '92vh', overflow: 'auto' }}
      >
        <h2>Planos do Online</h2>
        <div role="radiogroup" aria-label="Ciclo de cobrança" style={{ display: 'flex', gap: 8, alignItems: 'center', margin: '12px 0' }}>
          <button type="button" role="radio" aria-checked={ciclo === 'anual'} style={estiloOpcao(ciclo === 'anual')} onClick={() => setCiclo('anual')}>
            Anual no Pix
          </button>
          <button type="button" role="radio" aria-checked={ciclo === 'mensal'} style={estiloOpcao(ciclo === 'mensal')} onClick={() => setCiclo('mensal')}>
            Mensal no cartão
          </button>
          {ciclo === 'anual' && meses > 0 && <span style={{ color: 'var(--verde)', fontSize: 13 }}>{`${meses} meses de graça`}</span>}
        </div>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))', gap: 12 }}>
          {CARTOES.map((c) => (
            <section
              key={c.id}
              data-plano={c.id}
              data-destaque={estado.destaque === c.id ? 'true' : undefined}
              style={{ padding: 16, borderRadius: 14, display: 'flex', flexDirection: 'column', gap: 8, border: `1px solid ${estado.destaque === c.id ? 'var(--verde)' : 'var(--borda)'}` }}
            >
              <h3>{c.nome}</h3>
              {atual === c.id && (
                <div>
                  <SeloPlano plano={c.id} /> <span>Seu plano</span>
                </div>
              )}
              <p>{c.descricao}</p>
              <strong>{textoCiclo(precos, c.id, ciclo)}</strong>
              {!estado.aguardandoPagamento &&
                (atual === c.id ? (
                  <Button disabled>Plano atual</Button>
                ) : (
                  <Button disabled={estado.ocupado} onClick={() => void assinar(c.id, ciclo, documento)}>
                    {temPago ? 'Trocar para este' : 'Assinar'}
                  </Button>
                ))}
            </section>
          ))}
        </div>
        {temPago && <p>Trocar de plano cancela a assinatura atual; a nova começa sem desconto proporcional.</p>}
        <label style={{ display: 'block', marginTop: 16 }}>
          <input
            className="zp-input"
            style={{ width: 'min(320px, 100%)' }}
            inputMode="numeric"
            aria-label="CPF ou CNPJ (opcional)"
            placeholder="CPF ou CNPJ (opcional)"
            value={documento}
            onChange={(e) => setDocumento(e.target.value)}
          />
          <small style={{ display: 'block', marginTop: 6, color: 'var(--texto-3)' }}>
            Vai só para o Asaas, que faz a cobrança. Se deixar em branco, ele pede na página de pagamento.
          </small>
        </label>
        {estado.aguardandoPagamento && (
          <div style={{ marginTop: 12 }}>
            <p>Abrimos o pagamento no navegador. Depois de pagar, volte aqui.</p>
            <Button disabled={estado.ocupado} onClick={() => void atualizarDepoisDePagar()}>Já assinei, atualizar</Button>{' '}
            <Button variant="ghost" onClick={pararDeEsperar}>Fechar</Button>
          </div>
        )}
        {estado.aviso && <p role="alert" style={{ color: 'var(--vermelho)', fontSize: 14 }}>{estado.aviso}</p>}
        {estado.info && <p role="status" style={{ color: 'var(--verde)', fontSize: 14 }}>{estado.info}</p>}
        <div style={{ marginTop: 16, textAlign: 'right' }}>
          <Button variant="ghost" onClick={fecharPlanos}>Fechar</Button>
        </div>
      </div>
    </div>
  )
}
