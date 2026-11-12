// Tarefa salas-14: compõe a tela Salas (ListaSalas + DetalheSala) e liga à ponte.
import { useEffect, useState, type ReactNode } from 'react'
import { api } from '../../bridge/api'
import { alvoParaEntrar } from '../../bridge/convite'
import { useOnlineEnabled } from '../../bridge/onlineStore'
import { useSnapshot } from '../../bridge/store'
import { useTravas } from '../../bridge/usePlano'
import type { ConvitePedido } from '../../bridge/useConvites'
import { ConviteEspectador } from '../planos/ConviteEspectador'
import type { JoinTarget, RoomMode } from '../../bridge/types'
import { Button } from '../../components/Button'
import { TextField } from '../../components/TextField'
import { AtualizadoCard } from './AtualizadoCard'
import { CartaoContaNecessaria } from './CartaoContaNecessaria'
import { CartaoSenhaServidor } from './CartaoSenhaServidor'
import { DetalheSala } from './DetalheSala'
import { EntrarCodigo } from './EntrarCodigo'
import { ListaSalas } from './ListaSalas'
import { contar, filtrarSalas, salasDoModo, type FiltroSala } from './model'

export interface SalasScreenProps {
  onCreate: () => void
  onEntered: () => void
  onAbrirConfig?: () => void
  /** Convite de sala recebido pelo link: a tela entra direto (ou abre o painel, se faltar senha). */
  convite?: ConvitePedido | null
  /** Chamado assim que a tela pega o convite, para ele não ser tratado de novo. */
  onConviteTratado?: () => void
}

// Porta padrão do protocolo local quando o usuário digita só o IP.
const PORTA_PADRAO = 47800

// Lê "ip:porta" (ou só "ip", com a porta padrão). Devolve null se o formato não for válido.
function lerAlvoManual(texto: string): JoinTarget | null {
  const limpo = texto.trim()
  if (limpo === '') return null
  const i = limpo.lastIndexOf(':')
  const ip = i === -1 ? limpo : limpo.slice(0, i).trim()
  const portaTxt = i === -1 ? '' : limpo.slice(i + 1).trim()
  if (ip === '') return null
  const port = portaTxt === '' ? PORTA_PADRAO : Number(portaTxt)
  if (!Number.isInteger(port) || port < 1 || port > 65535) return null
  return { ip, port }
}

export function SalasScreen({ onCreate, onEntered, onAbrirConfig, convite, onConviteTratado }: SalasScreenProps): ReactNode {
  const snap = useSnapshot()

  const modoInicial: RoomMode = snap.rooms[0]?.mode ?? 'local'
  const habilitado = useOnlineEnabled()
  const [modoEstado, setModo] = useState<RoomMode>(modoInicial)
  // Online desligado: o modo efetivo é sempre local (o estado é ignorado).
  const modo: RoomMode = habilitado ? modoEstado : 'local'
  const travas = useTravas(modo)
  const semPlano = habilitado && modo === 'online' && !travas.podeEntrar
  const [filtro, setFiltro] = useState<FiltroSala>('todas')
  const [busca, setBusca] = useState('')
  const [selectedId, setSelectedId] = useState<string | null>(
    () => salasDoModo(snap.rooms, modoInicial)[0]?.id ?? null,
  )
  const [senha, setSenha] = useState('')
  const [apelido, setApelido] = useState(snap.me.name)
  const [ocupado, setOcupado] = useState(false)
  const [erro, setErro] = useState<string | null>(null)
  const [promptManual, setPromptManual] = useState(false)
  // Painel "Entrar com código" (modo Online), no lugar do prompt de IP do modo Local.
  const [promptCodigo, setPromptCodigo] = useState(false)
  const [manual, setManual] = useState('')
  const [firewallOculto, setFirewallOculto] = useState(false)
  // Código que o painel "Entrar com código" abre preenchido (convite pelo link).
  const [codigoInicial, setCodigoInicial] = useState('')
  // Alvo do convite esperando o modo da tela trocar para então entrar.
  const [entrarConvite, setEntrarConvite] = useState<JoinTarget | null>(null)

  const todas = salasDoModo(snap.rooms, modo)
  const contagens = contar(todas)
  const visiveis = filtrarSalas(todas, filtro, busca)
  // Se a selecionada sumiu da lista, cai na primeira visível.
  const sala = todas.find((s) => s.id === selectedId) ?? visiveis[0] ?? null
  const mostrarFirewall =
    !firewallOculto && snap.errors.some((e) => e.toLowerCase().includes('firewall'))

  function trocarModo(m: RoomMode): void {
    setModo(m)
    setFiltro('todas')
    setSenha('')
    setErro(null)
    setPromptManual(false)
    setPromptCodigo(false)
    setSelectedId(salasDoModo(snap.rooms, m)[0]?.id ?? null)
  }

  function selecionar(id: string): void {
    setSelectedId(id)
    setSenha('')
    setErro(null)
  }

  async function entrar(target: JoinTarget, pwd?: string): Promise<void> {
    // Sem plano para entrar no Online: o convite já está na tela, não chama a ponte.
    if (semPlano) return
    setOcupado(true)
    setErro(null)
    try {
      if (pwd === undefined) await api.roomJoin(target)
      else await api.roomJoin(target, pwd)
      onEntered()
    } catch (e) {
      setErro(String(e))
    } finally {
      setOcupado(false)
    }
  }

  function onEntrar(): void {
    if (sala === null) return
    if (sala.trancada) {
      if (senha === '') {
        setErro('Digite a senha da sala')
        return
      }
      void entrar({ room_id: sala.id }, senha)
      return
    }
    void entrar({ room_id: sala.id })
  }

  function entrarPorCodigo(codigo: string, senhaSala: string | null): void {
    void entrar({ code: codigo }, senhaSala ?? undefined)
  }

  function confirmarManual(): void {
    const alvo = lerAlvoManual(manual)
    if (alvo === null) {
      setErro('Informe um IP válido, como 192.168.0.10:47800')
      return
    }
    void entrar(alvo)
  }

  function confirmarApelido(): void {
    const nome = apelido.trim()
    if (nome !== '' && nome !== snap.me.name) void api.settingsSet({ name: nome })
  }

  // Convite pelo link: troca para o modo certo e abre o painel já preenchido (se a sala pedir
  // senha, a pessoa só digita a senha). A entrada vem no efeito seguinte, com o modo já trocado.
  const conviteSeq = convite?.seq
  useEffect(() => {
    if (!convite) return
    onConviteTratado?.()
    const alvo = alvoParaEntrar(convite.alvo)
    if (alvo === null) return
    if ('code' in alvo) {
      if (!habilitado) {
        setErro(`O convite é para a sala Online ${alvo.code}: ative o modo Online em Configurações para entrar.`)
        return
      }
      trocarModo('online')
      setCodigoInicial(alvo.code)
      setPromptCodigo(true)
    } else if ('ip' in alvo) {
      trocarModo('local')
      setManual(`${alvo.ip}:${alvo.port}`)
      setPromptManual(true)
    } else {
      return
    }
    setEntrarConvite(alvo)
    // eslint-disable-next-line react-hooks/exhaustive-deps -- só um convite novo (seq) dispara
  }, [conviteSeq])

  useEffect(() => {
    if (entrarConvite === null) return
    const modoAlvo: RoomMode = 'code' in entrarConvite ? 'online' : 'local'
    if (modo !== modoAlvo) return
    setEntrarConvite(null)
    void entrar(entrarConvite)
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `entrar` é recriada a cada render
  }, [entrarConvite, modo])

  return (
    <div style={{ display: 'flex', height: '100%', minHeight: 0, minWidth: 0 }}>
      <ListaSalas
        modo={modo}
        onModo={trocarModo}
        salas={visiveis}
        contagemOnline={habilitado ? salasDoModo(snap.rooms, 'online').length : 0}
        contagemLocal={salasDoModo(snap.rooms, 'local').length}
        contagens={contagens}
        filtro={filtro}
        onFiltro={setFiltro}
        busca={busca}
        onBusca={setBusca}
        selecionadaId={sala?.id ?? null}
        onSelect={selecionar}
        apelido={apelido}
        onApelido={setApelido}
        onApelidoConfirmar={confirmarApelido}
        onCriar={onCreate}
        servidor={habilitado ? snap.server : undefined}
        onAbrirConfig={onAbrirConfig}
        onEntrarManual={() => {
          setErro(null)
          setCodigoInicial('')
          if (habilitado && modo === 'online') {
            setPromptManual(false)
            setPromptCodigo(true)
          } else {
            setPromptCodigo(false)
            setPromptManual(true)
          }
        }}
      />
      {/* O detalhe rola sozinho, sem levar a lista junto. */}
      <div data-testid="detalhe-coluna" style={{ flex: '999 1 560px', minWidth: 0, minHeight: 0, overflowY: 'auto', display: 'flex', flexDirection: 'column' }}>
        <AtualizadoCard />
        {habilitado && modo === 'online' && snap.account?.login_error === 'senha_servidor' ? (
          <CartaoSenhaServidor />
        ) : null}
        {habilitado && modo === 'online' && snap.account?.login_error === 'conta_necessaria' ? (
          <CartaoContaNecessaria />
        ) : null}
        {habilitado && promptCodigo ? (
          <EntrarCodigo
            key={codigoInicial}
            codigoInicial={codigoInicial}
            onEntrar={entrarPorCodigo}
            onCancelar={() => {
              setPromptCodigo(false)
              setErro(null)
            }}
            ocupado={ocupado}
            erro={erro}
          />
        ) : null}
        {promptManual ? (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 10, padding: '16px 32px 0' }}>
            <TextField value={manual} onChange={setManual} placeholder="IP:porta (ex. 192.168.0.10:47800)" />
            {erro !== null ? (
              <p role="alert" style={{ margin: 0, color: 'var(--vermelho)', fontSize: 13 }}>{erro}</p>
            ) : null}
            <div>
              <Button variant="primary" disabled={ocupado} onClick={confirmarManual}>Entrar</Button>
            </div>
          </div>
        ) : null}
        <DetalheSala
          sala={sala}
          mostrarFirewall={mostrarFirewall}
          onCorrigirFirewall={() => setFirewallOculto(true)}
          senha={senha}
          onSenha={setSenha}
          ocupado={ocupado}
          erro={promptManual || promptCodigo ? null : erro}
          onEntrar={onEntrar}
          convite={semPlano ? <ConviteEspectador /> : undefined}
        />
      </div>
    </div>
  )
}
