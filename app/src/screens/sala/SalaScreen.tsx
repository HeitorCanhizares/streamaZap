// Tarefa ui-22: tela "Dentro da sala". Compõe SalaPrincipal, SalaTiles e SalaPainel.
// sala-08: dono do estado compartilhado (mudo, volume, PiP, painel) e do layout flex-wrap do design 3-sala.html.
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { api } from '../../bridge/api'
import { useSnapshot } from '../../bridge/store'
import { Button } from '../../components/Button'
import { SalaPainel } from './SalaPainel'
import { SalaPrincipal } from './SalaPrincipal'
import { SalaTiles } from './SalaTiles'

export interface SalaScreenProps { onLeft: () => void }

export function SalaScreen({ onLeft }: SalaScreenProps): ReactNode {
  const snap = useSnapshot()
  const room = snap.room

  const [mainTile, setMainTile] = useState<number | null>(snap.tiles[0]?.tile ?? null)
  // Verdadeiro quando o usuário fechou a principal e não havia outro tile para pôr no lugar.
  const [principalFechada, setPrincipalFechada] = useState(false)
  const [smoothingAuto, setSmoothingAuto] = useState(true)
  const [cameraOn, setCameraOn] = useState(false)
  const [aviso, setAviso] = useState<string | null>(null)
  const [muted, setMuted] = useState(false)
  const [volume, setVolume] = useState(80)
  const [pipHidden, setPipHidden] = useState(false)
  const [panelOpen, setPanelOpen] = useState(true)

  // Se o tile principal sumiu do snapshot, volta ao primeiro disponível (salvo se o usuário fechou a principal).
  const tileAtivo = principalFechada
    ? null
    : snap.tiles.some((t) => t.tile === mainTile) ? mainTile : (snap.tiles[0]?.tile ?? null)
  const main = snap.tiles.find((t) => t.tile === tileAtivo) ?? null

  // Marca se já estivemos numa sala; ao cair para null (sala encerrada) avisa uma vez só.
  const estavaNaSala = useRef(false)
  useEffect(() => {
    if (room) {
      estavaNaSala.current = true
    } else if (estavaNaSala.current) {
      estavaNaSala.current = false
      onLeft()
    }
  }, [room, onLeft])

  if (room === null) {
    return (
      <main style={{ flex: 1, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: 16, padding: 28 }}>
        <span style={{ fontSize: 16 }}>Você não está em uma sala</span>
        <Button variant="ghost" onClick={onLeft}>Voltar</Button>
      </main>
    )
  }

  const onPick = (tile: number) => {
    setMainTile(tile)
    setPrincipalFechada(false)
  }

  const onCloseMain = () => {
    if (tileAtivo === null) return
    void api.unwatch(tileAtivo).catch(() => {})
    // Escolhe o próximo tile; sem outro, a principal fica vazia em vez de reabrir o mesmo.
    const proximo = snap.tiles.find((t) => t.tile !== tileAtivo)?.tile ?? null
    if (proximo === null) {
      setPrincipalFechada(true)
    } else {
      setMainTile(proximo)
    }
  }

  const onSmoothingAuto = (v: boolean) => {
    setSmoothingAuto(v)
    void api.setSmoothing(v, snap.tiles[0]?.smoothing_ms ?? 200).catch(() => {})
  }

  const onToggleCamera = () => {
    const novo = !cameraOn
    setCameraOn(novo)
    void api.cameraSet(novo).catch(() => {})
  }

  const onToggleShare = () => {
    if (snap.share) {
      setAviso(null)
      void api.shareStop().catch(() => {})
    } else {
      // Sem seletor de fonte neste marco: a escolha fica em Criar sala.
      setAviso('Escolha a fonte em Criar sala')
    }
  }

  const onLeave = async () => {
    estavaNaSala.current = false
    try {
      await api.roomLeave()
    } catch {
      // Mesmo com erro ao sair, a tela deve voltar.
    }
    onLeft()
  }

  const onFullscreen = () => {
    if (document.fullscreenElement) {
      void document.exitFullscreen().catch(() => {})
    } else {
      void document.documentElement.requestFullscreen().catch(() => {})
    }
  }

  const onSend = (text: string) => {
    void api.chatSend(text).catch(() => {})
  }

  return (
    <div style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'stretch', flex: 1, minHeight: 0 }}>
      <div style={{ flex: '999 1 600px', minWidth: 0, display: 'flex', flexDirection: 'column' }}>
        <SalaPrincipal
          room={room}
          main={main}
          share={snap.share}
          smoothingAuto={smoothingAuto}
          onSmoothingAuto={onSmoothingAuto}
          onToggleCamera={onToggleCamera}
          onToggleShare={onToggleShare}
          onLeave={() => { void onLeave() }}
          onFullscreen={onFullscreen}
          muted={muted}
          volume={volume}
          camOn={cameraOn}
          pipHidden={pipHidden}
          onTogglePip={() => setPipHidden((h) => !h)}
          panelOpen={panelOpen}
          onOpenPanel={() => setPanelOpen(true)}
          onToggleMute={() => setMuted((m) => !m)}
          onVolume={setVolume}
          onCloseMain={onCloseMain}
        />
        <div style={{ padding: '0 28px 28px', display: 'flex', flexDirection: 'column', gap: 8 }}>
          {aviso && <span role="status" style={{ fontSize: 13, color: 'var(--texto-2, #A3BDB1)' }}>{aviso}</span>}
          <SalaTiles
            room={room}
            tiles={snap.tiles}
            mainTile={tileAtivo}
            onPick={onPick}
            camOn={cameraOn}
            sharing={!!snap.share}
            onToggleCam={onToggleCamera}
            onToggleShare={onToggleShare}
          />
        </div>
      </div>
      {panelOpen && (
        <SalaPainel
          room={room}
          onSend={onSend}
          meId={snap.me.install_id}
          onClose={() => setPanelOpen(false)}
        />
      )}
    </div>
  )
}
