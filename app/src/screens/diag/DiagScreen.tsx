// Tarefa ui-27: tela Diagnóstico (docs/design/5-diagnostico.html).
// Dados reais vêm do snapshot. Servidor e Firewall ainda não existem (M4/M5): ficam como "em breve".
import type { ReactNode } from 'react'
import { Badge } from '../../components/Badge'
import type { BadgeProps } from '../../components/Badge'
import { Button } from '../../components/Button'
import { Card } from '../../components/Card'
import { useSnapshot } from '../../bridge/store'
import type { Snapshot } from '../../bridge/types'

type Estado = 'ok' | 'atencao' | 'erro' | 'vazio'
export type Veredito = 'ok' | 'atencao' | 'sem_rede'

const TOM: Record<Estado, BadgeProps['tone']> = { ok: 'green', atencao: 'yellow', erro: 'red', vazio: 'neutral' }
const COR: Record<Estado, string> = { ok: '#3BF08A', atencao: '#FFB547', erro: '#FF8592', vazio: '#7E978B' }
const ROTULO: Record<Estado, string> = { ok: 'ok', atencao: 'atenção', erro: 'erro', vazio: 'vazio' }

// Sem interface: 'Sem rede'. Algum amigo não recebe: 'Atenção'. Senão: 'Tudo certo'.
export function calcularVeredito(snap: Snapshot): Veredito {
  if (snap.network.interfaces.length === 0) return 'sem_rede'
  if (snap.peers.some((p) => !p.receives_you)) return 'atencao'
  return 'ok'
}

const VEREDITO: Record<Veredito, { titulo: string; sub: string; cor: string }> = {
  ok: { titulo: 'Tudo certo', sub: 'Seu PC recebe e envia sem bloqueio.', cor: COR.ok },
  atencao: {
    titulo: 'Atenção',
    sub: 'Dá pra entrar nas salas, mas alguns amigos podem não te ver. Veja os itens em amarelo.',
    cor: COR.atencao,
  },
  sem_rede: {
    titulo: 'Sem rede',
    sub: 'Nenhuma interface de rede foi detectada. Confira a conexão do PC.',
    cor: COR.erro,
  },
}

function Linha({ estado, children, sub, badge }: { estado: Estado; children: ReactNode; sub?: string; badge?: string }): ReactNode {
  return (
    <li style={{ display: 'flex', alignItems: 'flex-start', gap: 10 }}>
      <span
        style={{
          flex: '0 0 8px',
          width: 8,
          height: 8,
          marginTop: 7,
          borderRadius: '50%',
          background: COR[estado],
        }}
      />
      <span style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', gap: 2 }}>
        <span style={{ lineHeight: 1.4 }}>{children}</span>
        {sub ? <span style={{ fontSize: 12.5, color: '#8BA498', lineHeight: 1.4 }}>{sub}</span> : null}
      </span>
      {badge ? <Badge tone={TOM[estado]}>{badge}</Badge> : null}
    </li>
  )
}

// Cartão de checagem: título, selo de estado e corpo (linhas de detalhe).
function CardCheck({ titulo, estado, rotulo, children }: { titulo: string; estado: Estado; rotulo: string; children: ReactNode }): ReactNode {
  return (
    <div style={{ flex: '1 1 440px', minWidth: 0, display: 'flex' }}>
      <Card>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 14, padding: '20px 22px' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
            <h2 style={{ margin: 0, flex: 1, fontSize: 16, fontWeight: 700 }}>{titulo}</h2>
            <Badge tone={TOM[estado]}>{rotulo}</Badge>
          </div>
          <ul style={{ listStyle: 'none', margin: 0, padding: 0, display: 'flex', flexDirection: 'column', gap: 10 }}>
            {children}
          </ul>
        </div>
      </Card>
    </div>
  )
}

export function DiagScreen(): ReactNode {
  // Só re-lê o snapshot: o botão "Testar de novo" ainda não tem ação neste marco.
  const snap = useSnapshot()
  const veredito = calcularVeredito(snap)
  const v = VEREDITO[veredito]

  // Rede: sem interface é erro; sem Radmin é atenção.
  const redeEstado: Estado =
    snap.network.interfaces.length === 0 ? 'erro' : snap.network.radmin_ip ? 'ok' : 'atencao'

  const naoRecebe = snap.peers.filter((p) => !p.receives_you).length
  const amigosEstado: Estado = snap.peers.length === 0 ? 'vazio' : naoRecebe > 0 ? 'atencao' : 'ok'

  const salasEstado: Estado = snap.rooms.length > 0 ? 'ok' : 'vazio'

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 20, padding: '28px 32px 36px' }}>
      <div style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'flex-end', gap: 14 }}>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
          <span
            style={{
              fontFamily: "'JetBrains Mono', monospace",
              fontSize: 11,
              letterSpacing: '0.14em',
              textTransform: 'uppercase',
              color: '#3BF08A',
            }}
          >
            Diagnóstico
          </span>
          <h1 style={{ margin: 0, fontFamily: "'Unbounded', sans-serif", fontWeight: 700, fontSize: 32, letterSpacing: '-0.02em' }}>
            Tá tudo conectando?
          </h1>
        </div>
        <span style={{ flex: 1 }} />
        <Button variant="ghost">Testar de novo</Button>
      </div>

      <section
        aria-label="Resumo"
        style={{
          display: 'flex',
          flexWrap: 'wrap',
          alignItems: 'center',
          gap: 24,
          padding: '22px 24px',
          borderRadius: 20,
          background: '#0F1916',
          border: '1px solid #1F332D',
        }}
      >
        <span style={{ position: 'relative', width: 104, height: 104, flex: '0 0 104px' }}>
          <span style={{ position: 'absolute', inset: 0, borderRadius: 30, border: `2px solid ${v.cor}` }} />
        </span>
        <div style={{ flex: '1 1 320px', minWidth: 0, display: 'flex', flexDirection: 'column', gap: 8 }}>
          <h2 style={{ margin: 0, fontFamily: "'Unbounded', sans-serif", fontWeight: 700, fontSize: 24, letterSpacing: '-0.01em' }}>
            {v.titulo}
          </h2>
          <span style={{ color: '#A3BDB1' }}>{v.sub}</span>
        </div>
      </section>

      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 20, alignItems: 'stretch' }}>
        <CardCheck titulo="Rede" estado={redeEstado} rotulo={ROTULO[redeEstado]}>
          {snap.network.interfaces.length === 0 ? (
            <Linha estado="erro">Nenhuma interface detectada</Linha>
          ) : (
            <Linha estado="ok">Interfaces: {snap.network.interfaces.join(', ')}</Linha>
          )}
          {snap.network.radmin_ip ? (
            <Linha estado="ok">Radmin · {snap.network.radmin_ip}</Linha>
          ) : (
            <Linha estado="atencao" sub="O modo Online continua funcionando sem ele.">
              Radmin: não detectado
            </Linha>
          )}
        </CardCheck>

        <CardCheck titulo="Amigos na rede" estado={amigosEstado} rotulo={ROTULO[amigosEstado]}>
          {snap.peers.length === 0 ? (
            <Linha estado="vazio">Nenhum amigo com o app aberto agora.</Linha>
          ) : (
            snap.peers.map((p) => (
              <Linha
                key={p.ip}
                estado={p.receives_you ? 'ok' : 'atencao'}
                sub={p.receives_you ? undefined : 'Não recebe nada do seu PC — o bloqueio está no PC dele.'}
                badge={p.receives_you ? 'recebe você' : 'não recebe'}
              >
                <b>{p.name}</b> · <span style={{ fontFamily: "'JetBrains Mono', monospace" }}>{p.ip}</span>
              </Linha>
            ))
          )}
        </CardCheck>

        <CardCheck titulo="Salas" estado={salasEstado} rotulo={ROTULO[salasEstado]}>
          {snap.rooms.length === 0 ? (
            <Linha estado="vazio">Nenhuma sala descoberta ainda.</Linha>
          ) : (
            <>
              <Linha estado="ok">{snap.rooms.length} salas descobertas</Linha>
              {snap.rooms.map((r) => (
                <Linha key={r.room_id} estado="ok" sub={`${r.mode === 'online' ? 'Online' : 'Local'} · ${r.room_id}`}>
                  {r.name} — {r.host}
                </Linha>
              ))}
            </>
          )}
        </CardCheck>

        <CardCheck titulo="Servidor" estado="vazio" rotulo="em breve">
          <Linha estado="vazio">Disponível em breve</Linha>
        </CardCheck>

        <CardCheck titulo="Firewall" estado="vazio" rotulo="em breve">
          <Linha estado="vazio">Disponível em breve</Linha>
        </CardCheck>

        <section
          aria-labelledby="h-ler"
          style={{
            flex: '1 1 440px',
            minWidth: 0,
            display: 'flex',
            flexDirection: 'column',
            gap: 12,
            padding: '20px 22px',
            borderRadius: 18,
            background: 'rgba(92,214,255,0.06)',
            border: '1px solid rgba(92,214,255,0.25)',
          }}
        >
          <h2 id="h-ler" style={{ margin: 0, fontSize: 16, fontWeight: 700 }}>
            Como ler isso
          </h2>
          <p style={{ margin: 0, color: '#C6D9CF', lineHeight: 1.55 }}>
            Se a sala de alguém aparece mas dá <b style={{ color: '#E6F4EC' }}>tempo esgotado</b>, e do lado do nome dele está
            “não recebe nada do seu PC”, o bloqueio está no PC <b style={{ color: '#E6F4EC' }}>dele</b>.
          </p>
          <p style={{ margin: 0, color: '#C6D9CF', lineHeight: 1.55 }}>
            Teste rápido no PC bloqueado: desligue o Firewall do Windows por 1 minuto. Funcionou? É o firewall. Não funcionou?
            É antivírus ou o Radmin — veja se a conexão no Radmin está “direta”.
          </p>
          <p style={{ margin: 0, color: '#C6D9CF', lineHeight: 1.55 }}>
            No modo <b style={{ color: '#E6F4EC' }}>Online</b> nada disso importa tanto: o servidor faz a ponte quando a conexão
            direta falha.
          </p>
        </section>
      </div>
    </div>
  )
}
