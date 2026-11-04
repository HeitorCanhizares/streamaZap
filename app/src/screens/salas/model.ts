// Tarefa salas-02: modelo e funções puras da tela Salas.
import type { CSSProperties } from 'react'
import type { RoomListing, RoomMode, RoomPerson, RoomSharer, SceneKind } from '../../bridge/types'

export type FiltroSala = 'todas' | 'abertas' | 'senha'

/** Sala já com todos os campos do design preenchidos (defaults aplicados). */
export interface SalaVM {
  id: string
  nome: string
  host: string
  modo: RoomMode
  /** Código (modo online) ou IP (modo local); '' se desconhecido. */
  addr: string
  trancada: boolean
  /** Versão incompatível ('' = compatível). */
  versaoAviso: string
  cena: SceneKind
  camCena: SceneKind | null
  hora: string
  ultima: string
  naoLidas: number
  digitando: boolean
  ping: number
  qualidade: string
  bitrate: string
  assistindo: number
  compartilhando: RoomSharer[]
  pessoas: RoomPerson[]
}

export interface Contagens { todas: number; abertas: number; senha: number }

/** Paleta de cores por pessoa do design (1-salas.html linhas 264-266). */
const CORES_PESSOA: Record<string, string> = {
  'Marcão': '#FFB547',
  Rafa: '#5CD6FF',
  Bia: '#FF8FB1',
  'Léo': '#B79CFF',
  Dudu: '#FF9E5C',
  Ju: '#7CF2C0',
  Gabi: '#F2E86D',
  Nina: '#9FE870',
  Caio: '#FFB547',
}
const COR_PADRAO = '#3BF08A'

/** Cor da pessoa (paleta do design, 1-salas.html linhas 264-266); desconhecida = '#3BF08A'. */
export function corPessoa(nome: string): string {
  return Object.prototype.hasOwnProperty.call(CORES_PESSOA, nome) ? CORES_PESSOA[nome] : COR_PADRAO
}

/** Cor do ping: < 60 '#3BF08A', < 100 '#FFB547', senão '#FF8592'. */
export function corPing(ms: number): string {
  if (ms < 60) return '#3BF08A'
  if (ms < 100) return '#FFB547'
  return '#FF8592'
}

/** Estilo do avatar colorido do design (função `av`, linhas 266-269). */
export function estiloAvatar(nome: string, tamanho: number, anel: boolean): CSSProperties {
  const c = corPessoa(nome)
  const estilo: CSSProperties = {
    flex: `0 0 ${tamanho}px`,
    width: tamanho,
    height: tamanho,
    borderRadius: '50%',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    fontWeight: 700,
    fontSize: `${Math.round(tamanho * 0.4)}px`,
    color: c,
    background: `${c}24`,
  }
  // Anel: dois box-shadows, o de fora na cor da pessoa (mesmo do design).
  if (anel) estilo.boxShadow = `0 0 0 2px #0F1916, 0 0 0 4px ${c}`
  return estilo
}

/** Converte RoomListing da ponte em SalaVM aplicando defaults. */
export function paraSalaVM(room: RoomListing): SalaVM {
  const host = room.host
  const cena: SceneKind = room.scene ?? 'game'
  const ping = room.ping_ms ?? 0
  return {
    id: room.room_id,
    nome: room.name,
    host,
    modo: room.mode,
    addr: room.addr ?? '',
    trancada: room.has_password,
    versaoAviso: room.version_warn ?? '',
    cena,
    camCena: room.cam_scene ?? null,
    hora: room.last_time ?? '',
    ultima: room.last_message ?? '',
    naoLidas: room.unread ?? 0,
    digitando: room.typing ?? false,
    ping,
    qualidade: room.quality ?? '720p30',
    bitrate: room.bitrate ?? '',
    assistindo: room.viewers ?? Math.max(room.participants - 1, 0),
    // Sem lista explícita, o host aparece compartilhando a cena principal.
    compartilhando: room.sharers ?? [{ name: host, kind: 'Tela', scene: cena }],
    pessoas: room.people ?? [{ name: host, role: 'host', ping_ms: ping }],
  }
}

/** Salas do modo escolhido (sem filtro nem busca), já como SalaVM. */
export function salasDoModo(rooms: RoomListing[], modo: RoomMode): SalaVM[] {
  return rooms.filter((r) => r.mode === modo).map(paraSalaVM)
}

/** Contagens por filtro de uma lista (já restrita ao modo). */
export function contar(salas: SalaVM[]): Contagens {
  const senha = salas.filter((s) => s.trancada).length
  return { todas: salas.length, abertas: salas.length - senha, senha }
}

/** Remove acentos e põe em minúscula, para comparar buscas. */
function normalizar(texto: string): string {
  return texto.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase()
}

/** Aplica filtro e busca (nome ou host, sem acento e sem caixa). */
export function filtrarSalas(salas: SalaVM[], filtro: FiltroSala, busca: string): SalaVM[] {
  const termo = normalizar(busca.trim())
  return salas.filter((s) => {
    if (filtro === 'abertas' && s.trancada) return false
    if (filtro === 'senha' && !s.trancada) return false
    if (!termo) return true
    return normalizar(`${s.nome} ${s.host}`).includes(termo)
  })
}
