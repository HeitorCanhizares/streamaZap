// Tarefa ui-14: compõe a tela Salas (lista + detalhe) e liga à ponte.
// Porte de docs/design/1-salas.html: lista à esquerda, detalhe à direita.
import { useState, type CSSProperties, type ReactNode } from 'react'
import { api } from '../../bridge/api'
import { useSnapshot } from '../../bridge/store'
import type { JoinTarget, RoomListing } from '../../bridge/types'
import { Button } from '../../components/Button'
import { TextField } from '../../components/TextField'
import { SalasDetalhe } from './SalasDetalhe'
import { SalasLista, type SalasFiltro } from './SalasLista'

export interface SalasScreenProps { onCreate: () => void; onEntered: () => void }

// Porta padrão do protocolo local quando o usuário digita só o IP.
const PORTA_PADRAO = 47800

// Abertura inline de senha ou de IP:porta, mostrada acima do botão de entrar.
type Prompt = { tipo: 'senha'; room: RoomListing } | { tipo: 'manual' } | null

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

const colunaLista: CSSProperties = {
  flex: '1 1 320px',
  maxWidth: 392,
  minWidth: 0,
  display: 'flex',
  flexDirection: 'column',
}

export function SalasScreen({ onCreate, onEntered }: SalasScreenProps): ReactNode {
  const snap = useSnapshot()

  // Primeira sala do snapshot como seleção inicial; depois a escolha do usuário manda.
  const [selectedId, setSelectedId] = useState<string | null>(snap.rooms[0]?.room_id ?? null)
  const [query, setQuery] = useState('')
  const [filter, setFilter] = useState<SalasFiltro>('todas')
  const [prompt, setPrompt] = useState<Prompt>(null)
  const [senha, setSenha] = useState('')
  const [manual, setManual] = useState('')
  const [erro, setErro] = useState<string | null>(null)
  const [ocupado, setOcupado] = useState(false)

  // Se a sala escolhida sumiu do snapshot, o detalhe fica vazio em vez de mostrar um fantasma.
  const room = snap.rooms.find((r) => r.room_id === selectedId) ?? null

  // Entra de fato: chama a ponte e, em sucesso, avisa a tela pai.
  async function entrar(target: JoinTarget, pwd?: string): Promise<void> {
    setOcupado(true)
    setErro(null)
    try {
      if (pwd === undefined) await api.roomJoin(target)
      else await api.roomJoin(target, pwd)
      setPrompt(null)
      setSenha('')
      setManual('')
      onEntered()
    } catch (e) {
      setErro(String(e))
    } finally {
      setOcupado(false)
    }
  }

  function onEnter(r: RoomListing): void {
    setErro(null)
    if (r.has_password) {
      setSenha('')
      setPrompt({ tipo: 'senha', room: r })
      return
    }
    setPrompt(null)
    void entrar({ room_id: r.room_id })
  }

  function onEnterManual(): void {
    setErro(null)
    setPrompt({ tipo: 'manual' })
  }

  function confirmarPrompt(): void {
    if (prompt === null) return
    if (prompt.tipo === 'senha') {
      void entrar({ room_id: prompt.room.room_id }, senha)
      return
    }
    const alvo = lerAlvoManual(manual)
    if (alvo === null) {
      setErro('Informe um IP válido, como 192.168.0.10:47800')
      return
    }
    void entrar(alvo)
  }

  return (
    <div style={{ display: 'flex', height: '100%', minHeight: 0, minWidth: 0 }}>
      <div style={colunaLista}>
        <div style={{ padding: '16px 20px 0', background: 'var(--superficie)' }}>
          <Button variant="primary" onClick={onCreate}>
            Criar sala
          </Button>
        </div>
        <SalasLista
          rooms={snap.rooms}
          selectedId={room?.room_id ?? null}
          onSelect={(id) => {
            setSelectedId(id)
            setPrompt(null)
            setErro(null)
          }}
          query={query}
          onQuery={setQuery}
          filter={filter}
          onFilter={setFilter}
        />
        <p
          style={{
            margin: 0,
            padding: '10px 20px',
            fontSize: 12,
            color: 'var(--texto-3)',
            background: 'var(--superficie)',
            borderRight: '1px solid var(--borda)',
            borderTop: '1px solid var(--borda)',
          }}
        >
          Seu nome: {snap.me.name}
        </p>
      </div>

      <div style={{ flex: '999 1 560px', minWidth: 0, display: 'flex', flexDirection: 'column' }}>
        {prompt !== null || erro !== null ? (
          <div
            style={{
              display: 'flex',
              flexDirection: 'column',
              gap: 10,
              padding: '16px 32px 0',
            }}
          >
            {prompt?.tipo === 'senha' ? (
              <TextField type="password" value={senha} onChange={setSenha} placeholder="Senha da sala" />
            ) : null}
            {prompt?.tipo === 'manual' ? (
              <TextField value={manual} onChange={setManual} placeholder="IP:porta (ex. 192.168.0.10:47800)" />
            ) : null}
            {erro !== null ? (
              <p role="alert" style={{ margin: 0, color: 'var(--vermelho)', fontSize: 13 }}>
                {erro}
              </p>
            ) : null}
            {prompt !== null ? (
              <div>
                <Button variant="primary" disabled={ocupado} onClick={confirmarPrompt}>
                  Entrar
                </Button>
              </div>
            ) : null}
          </div>
        ) : null}

        <SalasDetalhe
          room={room}
          peers={snap.peers}
          onEnter={onEnter}
          onEnterManual={onEnterManual}
        />
      </div>
    </div>
  )
}
