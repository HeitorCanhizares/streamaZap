// Seção 'Conexão' da tela de configurações (porte fiel de docs/design/4-configuracoes.html, linhas 121-176).
import { useEffect, useState } from 'react'
import type { CSSProperties, ReactNode } from 'react'
import type { NetworkStatus, RoomMode } from '../../bridge/types'
import type { ConfigSecaoProps } from './ConfigPerfil'
import { useServerTest } from './useServerTest'
import { textoTeste } from '../../bridge/serverModel'
import { api } from '../../bridge/api'
import { useSnapshot } from '../../bridge/store'

const MODOS: Array<[RoomMode, string]> = [['online', 'Online'], ['local', 'Local']]

function segStyle(on: boolean): CSSProperties {
  return {
    height: 38, padding: '0 16px', borderRadius: 9, border: 0, fontSize: 13, fontWeight: 600,
    background: on ? '#1A3A2C' : 'transparent', color: on ? '#3BF08A' : '#A3BDB1',
  }
}

function swTrack(on: boolean): CSSProperties {
  return {
    flex: '0 0 44px', width: 44, height: 26, borderRadius: 999, padding: 3, display: 'flex',
    justifyContent: on ? 'flex-end' : 'flex-start', background: on ? '#3BF08A' : '#2A443C',
  }
}

function swKnob(on: boolean): CSSProperties {
  return { width: 20, height: 20, borderRadius: '50%', background: on ? '#03140A' : '#A3BDB1' }
}

const botaoSwitch: CSSProperties = {
  display: 'flex', alignItems: 'center', gap: 12, padding: 0, border: 0, background: 'transparent', textAlign: 'left',
}
const rotulo: CSSProperties = { fontSize: 12.5, color: '#A3BDB1' }
const dica: CSSProperties = { fontSize: 12, color: '#7E978B' }
const campo: CSSProperties = {
  height: 44, padding: '0 14px', borderRadius: 12, border: '1px solid #2A443C', background: '#0F1916', color: '#E6F4EC',
}
const cartao: CSSProperties = {
  flex: '1 1 380px', minWidth: 0, display: 'flex', flexDirection: 'column', gap: 12, padding: 18,
  borderRadius: 16, background: '#0A100E', border: '1px solid #1F332D',
}
const titulo: CSSProperties = { display: 'flex', alignItems: 'center', gap: 10, fontWeight: 700 }
const icone = (bg: string, cor: string): CSSProperties => ({
  width: 34, height: 34, borderRadius: 10, background: bg, color: cor,
  display: 'flex', alignItems: 'center', justifyContent: 'center',
})

// Porta válida é um inteiro de 1 a 65535; vazio ou fora disso devolve null.
function portaValida(texto: string): number | null {
  if (!/^\d+$/.test(texto)) return null
  const n = Number(texto)
  return n >= 1 && n <= 65535 ? n : null
}

export function ConfigConexao({ settings, onChange, network }: ConfigSecaoProps & { network: NetworkStatus }): ReactNode {
  const local = settings.local
  // Servidor vem de settings.server; ausente significa campos vazios.
  const srv = settings.server
  const endereco = srv?.address ?? ''
  const senha = srv?.password ?? ''
  // Patch por seção: mantém os outros campos do servidor.
  const base = { enabled: srv?.enabled ?? false, address: endereco, password: senha, use_relay: srv?.use_relay ?? true }
  const { testando, resultado, testar, limpar } = useServerTest()
  const [porta, setPorta] = useState(local.port > 0 ? String(local.port) : '')

  // Se a porta vier de fora com outro valor, o campo acompanha.
  useEffect(() => {
    setPorta((atual) => (portaValida(atual) === local.port ? atual : local.port > 0 ? String(local.port) : ''))
  }, [local.port])

  const { texto: testeTexto, cor: testeCor } = textoTeste(resultado, testando)

  const online = base.enabled
  const emSalaOnline = useSnapshot().room?.mode === 'online'
  const [confirmando, setConfirmando] = useState(false)
  const [erroSair, setErroSair] = useState<string | null>(null)

  function alternarOnline(): void {
    if (!online) {
      onChange({ server: { ...base, enabled: true } })
      return
    }
    // Desligar dentro de uma sala Online sai dela: pede confirmação antes.
    if (emSalaOnline) {
      setErroSair(null)
      setConfirmando(true)
      return
    }
    onChange({ server: { ...base, enabled: false } })
  }

  async function desligarESair(): Promise<void> {
    try {
      await api.roomLeave()
    } catch (e) {
      // Não conseguiu sair: mantém o modo Online ligado.
      setErroSair(String(e))
      return
    }
    setConfirmando(false)
    onChange({ server: { ...base, enabled: false } })
  }

  return (
    <section
      aria-labelledby="h-conexao"
      style={{
        display: 'flex', flexDirection: 'column', gap: 16, padding: 22, borderRadius: 18,
        background: '#0F1916', border: '1px solid #1F332D',
      }}
    >
      <div style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 14 }}>
        <h2 id="h-conexao" style={{ margin: 0, fontSize: 16, fontWeight: 700 }}>Conexão</h2>
        <span style={{ flex: 1 }} />
        <span id="modo-label" style={rotulo}>Abrir o app em</span>
        <div
          role="radiogroup"
          aria-labelledby="modo-label"
          style={{ display: 'flex', gap: 4, padding: 4, borderRadius: 12, background: '#0A100E', border: '1px solid #1F332D' }}
        >
          {MODOS.map(([id, label]) => {
            const on = settings.open_in === id
            return (
              <button key={id} role="radio" aria-checked={on} onClick={() => onChange({ open_in: id })} style={segStyle(on)}>
                {label}
              </button>
            )
          })}
        </div>
      </div>

      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 16 }}>
        <div style={cartao}>
          <span style={titulo}>
            <span style={icone('rgba(59,240,138,0.12)', '#3BF08A')}>
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden="true">
                <circle cx="12" cy="12" r="9" />
                <path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18" />
              </svg>
            </span>
            Online · servidor
          </span>
          <button
            type="button"
            role="switch"
            aria-label="Usar o modo Online"
            aria-checked={online}
            onClick={alternarOnline}
            style={botaoSwitch}
          >
            <span style={swTrack(online)}><span style={swKnob(online)} /></span>
            <span style={{ fontSize: 13, color: '#A3BDB1', lineHeight: 1.4 }}>
              <b style={{ color: '#E6F4EC', fontWeight: 600 }}>Usar o modo Online</b>
              <br />
              Salas pela internet, sem Radmin. O modo Local continua funcionando como sempre.
            </span>
          </button>
          {confirmando && (
            <div
              role="alertdialog"
              aria-label="Desligar o modo Online"
              style={{ display: 'flex', flexDirection: 'column', gap: 10, padding: 14, borderRadius: 12, background: '#0A100E', border: '1px solid #2A443C' }}
            >
              <span style={{ fontSize: 13, color: '#E6F4EC' }}>Você está numa sala Online. Desligar o modo Online sai da sala.</span>
              {erroSair && <span role="alert" style={{ fontSize: 13, color: '#FF6B6B' }}>{erroSair}</span>}
              <div style={{ display: 'flex', gap: 10 }}>
                <button type="button" onClick={() => void desligarESair()} style={{ height: 40, padding: '0 16px', borderRadius: 10, border: 0, background: '#FF6B6B', color: '#1A0505', fontWeight: 700 }}>
                  Desligar e sair
                </button>
                <button type="button" onClick={() => { setConfirmando(false); setErroSair(null) }} style={{ height: 40, padding: '0 16px', borderRadius: 10, border: '1px solid #2A443C', background: 'transparent', color: '#A3BDB1', fontWeight: 600 }}>
                  Cancelar
                </button>
              </div>
            </div>
          )}
          {online && (
            <>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                <label htmlFor="chave" style={rotulo}>Senha do servidor</label>
                <input
                  id="chave"
                  type="password"
                  placeholder="pede pro dono do servidor"
                  value={senha}
                  onChange={(e) => { onChange({ server: { ...base, password: e.target.value } }); limpar() }}
                  style={{ ...campo, letterSpacing: '0.1em' }}
                />
                <span style={dica}>Barra bots e quem não foi convidado.</span>
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 10, padding: 14, borderRadius: 12, border: '1px solid #1F332D' }}>
                <span style={{ fontWeight: 600, fontSize: 13.5 }}>Servidor próprio</span>
                <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                  <label htmlFor="srv" style={rotulo}>Endereço do servidor</label>
                  <input
                    id="srv"
                    placeholder="ex.: zap.seudominio.com:7443"
                    value={endereco}
                    onChange={(e) => { onChange({ server: { ...base, address: e.target.value } }); limpar() }}
                    style={{ ...campo, fontFamily: "'JetBrains Mono', monospace", fontSize: 13 }}
                  />
                </div>
                <div style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 12 }}>
                  <button
                    className="zp-cta"
                    disabled={testando}
                    onClick={() => void testar(endereco, senha)}
                    style={{ height: 44, padding: '0 18px', borderRadius: 12, border: 0, background: '#3BF08A', color: '#03140A', fontWeight: 700 }}
                  >
                    Testar conexão
                  </button>
                  <span role="status" aria-live="polite" style={{ fontSize: 13, fontWeight: 600, color: testeCor }}>{testeTexto}</span>
                </div>
                <button
                  role="switch"
                  aria-checked={base.use_relay}
                  onClick={() => onChange({ server: { ...base, use_relay: !base.use_relay } })}
                  style={botaoSwitch}
                >
                  <span style={swTrack(base.use_relay)}><span style={swKnob(base.use_relay)} /></span>
                  <span style={{ fontSize: 13, color: '#A3BDB1', lineHeight: 1.4 }}>
                    <b style={{ color: '#E6F4EC', fontWeight: 600 }}>Usar o servidor como ponte</b> quando a conexão direta com alguém falhar.
                  </span>
                </button>
              </div>
            </>
          )}
        </div>

        <div style={cartao}>
          <span style={titulo}>
            <span style={icone('rgba(92,214,255,0.12)', '#5CD6FF')}>
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden="true">
                <path d="M2 9a15 15 0 0 1 20 0M5 12.5a10 10 0 0 1 14 0M8.5 16a5 5 0 0 1 7 0M12 19.5h.01" />
              </svg>
            </span>
            Local · Radmin e rede de casa
          </span>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
            <label htmlFor="rede" style={rotulo}>Rede usada para as salas</label>
            <select
              id="rede"
              value={local.interface ?? ''}
              onChange={(e) => onChange({ local: { ...local, interface: e.target.value === '' ? null : e.target.value } })}
              style={{ ...campo, padding: '0 10px' }}
            >
              <option value="">Automática (todas as redes)</option>
              {network.interfaces.map((nome) => (
                <option key={nome} value={nome}>
                  {network.radmin_ip && /radmin/i.test(nome) ? `${nome} · ${network.radmin_ip}` : nome}
                </option>
              ))}
            </select>
            {network.radmin_ip ? <span style={dica}>{network.radmin_ip}</span> : null}
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
            <label htmlFor="porta" style={rotulo}>Porta da transmissão</label>
            <input
              id="porta"
              inputMode="numeric"
              value={porta}
              onChange={(e) => {
                const texto = e.target.value.replace(/[^0-9]/g, '').slice(0, 5)
                setPorta(texto)
                // Porta vazia ou inválida só atualiza o campo.
                const port = portaValida(texto)
                if (port !== null) onChange({ local: { ...local, port } })
              }}
              style={{ ...campo, width: 140, fontFamily: "'JetBrains Mono', monospace" }}
            />
            <span style={dica}>Se estiver ocupada, o app usa a próxima livre. Descoberta de salas na 47801.</span>
          </div>
          <button
            role="switch"
            aria-checked={local.announce}
            onClick={() => onChange({ local: { ...local, announce: !local.announce } })}
            style={botaoSwitch}
          >
            <span style={swTrack(local.announce)}><span style={swKnob(local.announce)} /></span>
            <span style={{ fontSize: 13, color: '#A3BDB1', lineHeight: 1.4 }}>
              <b style={{ color: '#E6F4EC', fontWeight: 600 }}>Anunciar minhas salas</b> para quem está na mesma rede.
            </span>
          </button>
        </div>
      </div>
    </section>
  )
}
