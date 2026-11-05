// Tarefa ui-22: tela "Dentro da sala". Compõe SalaPrincipal, SalaTiles e SalaPainel.
// Dona do estado compartilhado (lugar de cada transmissão, volumes, painel) e das assinaturas:
// toda transmissão de outra pessoa é assinada sozinha ao aparecer (useAssinaturas).
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { api } from '../../bridge/api'
import { useSnapshot } from '../../bridge/store'
import { Button } from '../../components/Button'
import { SalaPainel } from './SalaPainel'
import { SalaPrincipal } from './SalaPrincipal'
import { SalaTiles } from './SalaTiles'
import { SalaTransmissao } from './SalaTransmissao'
import {
  celulasDaPrincipal,
  inviteText,
  lugaresDasStreams,
  ownTracks,
  streamsDaSala,
  volumeInicial,
  type Lugar,
  type VolumePessoa,
} from './salaModel'
import { useAssinaturas, type PedidoStream } from './useAssinaturas'

export interface SalaScreenProps { onLeft: () => void }

export function SalaScreen({ onLeft }: SalaScreenProps): ReactNode {
  const snap = useSnapshot()
  const room = snap.room

  // Onde o usuário pôs cada transmissão (principal, faixa ou fechada); sem escolha vale o padrão.
  const [escolhas, setEscolhas] = useState<Record<string, Lugar>>({})
  // Volume que escolhi para cada pessoa (0..100).
  const [volumes, setVolumes] = useState<Record<string, VolumePessoa>>({})
  const [smoothingAuto, setSmoothingAuto] = useState(true)
  const [cameraOn, setCameraOn] = useState(false)
  const [aviso, setAviso] = useState<string | null>(null)
  // Volume geral da sala, por cima do volume de cada pessoa.
  const [muted, setMuted] = useState(false)
  const [volume, setVolume] = useState(100)
  const [panelOpen, setPanelOpen] = useState(true)
  const [painelTx, setPainelTx] = useState(false)

  // Meu id nesta sala (no Online vem do servidor); a câmera está ligada se a trilha dela existe.
  const meId = room?.me_id || snap.me.install_id
  const temCamera = room ? ownTracks(room, meId).camera !== null : false
  useEffect(() => {
    setCameraOn(temCamera)
  }, [temCamera])

  // Transmissões dos outros, onde cada uma fica e o que assinar (o canto = câmera sobre a tela).
  const streams = room ? streamsDaSala(room, meId) : []
  const lugares = lugaresDasStreams(streams, escolhas)
  const celulas = celulasDaPrincipal(streams, lugares)
  const cantos = new Set(celulas.flatMap((c) => (c.pip ? [c.pip.trackId] : [])))
  const pedidos: PedidoStream[] = streams
    .filter((s) => lugares[s.trackId] !== 'fechada')
    .map((s) => ({
      stream: s,
      lugar: cantos.has(s.trackId) ? 'canto' : lugares[s.trackId] === 'principal' ? 'principal' : 'faixa',
    }))
  const canvasDe = useAssinaturas(pedidos, celulas.length)
  const naFaixa = streams.filter((s) => lugares[s.trackId] !== 'principal')
  // O cartão "Você" só aparece quando transmito (o botão de compartilhar já está nos controles).
  const mostrarVoce = !!snap.share || temCamera || cameraOn

  // Volume geral: aplica no núcleo sempre que muda (e ao entrar na sala).
  useEffect(() => {
    void Promise.resolve(api.setMasterVolume(volume / 100, muted)).catch(() => {})
  }, [volume, muted])
  // Convite do cabeçalho: código da sala ou IP do Radmin, conforme o que existir.
  const invite = room ? inviteText(room, snap.network) : undefined

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

  // Uma escolha fixa onde tudo está agora (pôr outra na principal soma, não troca); a câmera no
  // canto de uma tela desce junto quando a tela sai da principal.
  const onMover = (trackId: string, lugar: Lugar) => {
    const canto = celulas.find((c) => c.stream.trackId === trackId)?.pip
    setEscolhas({
      ...lugares,
      ...(canto && lugar !== 'principal' ? { [canto.trackId]: 'faixa' as Lugar } : {}),
      [trackId]: lugar,
    })
  }

  const volumeDe = (ownerId: string): VolumePessoa =>
    volumes[ownerId] ?? volumeInicial(room.participants.find((p) => p.id === ownerId))
  const onVolumePessoa = (ownerId: string, v: VolumePessoa) => {
    setVolumes((m) => ({ ...m, [ownerId]: v }))
    void Promise.resolve(api.setVolume(ownerId, v.vol / 100, v.muted)).catch(() => {})
  }

  const onSmoothingAuto = (v: boolean) => {
    setSmoothingAuto(v)
    void api.setSmoothing(v, snap.tiles[0]?.smoothing_ms ?? 200).catch(() => {})
  }

  const onToggleCamera = () => {
    const novo = !cameraOn
    setCameraOn(novo)
    void api.cameraSet(novo).catch((e: unknown) => {
      setCameraOn(!novo)
      setAviso(`Câmera: ${e instanceof Error ? e.message : String(e)}`)
    })
  }

  // Trocou o dispositivo com a câmera ligada: desliga e liga com o novo.
  const onCameraTrocada = () => {
    void (async () => {
      try {
        await api.cameraSet(false)
        await api.cameraSet(true)
      } catch (e) {
        setAviso(`Câmera: ${e instanceof Error ? e.message : String(e)}`)
      }
    })()
  }

  // Compartilhar (ou ajustar a transmissão): abre o painel com fonte, som, qualidade e câmera.
  const onToggleShare = () => {
    setAviso(null)
    setPainelTx(true)
  }

  const onStopShare = () => {
    setAviso(null)
    void api.shareStop().catch(() => {})
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
          celulas={celulas}
          tiles={snap.tiles}
          canvasDe={canvasDe}
          temOutras={naFaixa.length > 0}
          volumeDe={volumeDe}
          onVolumePessoa={onVolumePessoa}
          onMover={onMover}
          share={snap.share}
          smoothingAuto={smoothingAuto}
          onSmoothingAuto={onSmoothingAuto}
          onToggleCamera={onToggleCamera}
          onToggleShare={onToggleShare}
          onStopShare={onStopShare}
          onLeave={() => { void onLeave() }}
          onFullscreen={onFullscreen}
          muted={muted}
          volume={volume}
          camOn={cameraOn}
          panelOpen={panelOpen}
          onOpenPanel={() => setPanelOpen(true)}
          onToggleMute={() => setMuted((m) => !m)}
          onVolume={setVolume}
          invite={invite}
        />
        {(aviso || naFaixa.length > 0 || mostrarVoce) && (
          <div style={{ padding: '0 28px 28px', display: 'flex', flexDirection: 'column', gap: 8 }}>
            {aviso && <span role="status" style={{ fontSize: 13, color: 'var(--texto-2, #A3BDB1)' }}>{aviso}</span>}
            <SalaTiles
              room={room}
              streams={naFaixa}
              lugares={lugares}
              canvasDe={canvasDe}
              volumeDe={volumeDe}
              onVolumePessoa={onVolumePessoa}
              onMover={onMover}
              mostrarVoce={mostrarVoce}
              meId={meId}
              camOn={cameraOn}
              sharing={!!snap.share}
              onToggleCam={onToggleCamera}
              onToggleShare={onToggleShare}
            />
          </div>
        )}
      </div>
      {painelTx && (
        <SalaTransmissao
          share={snap.share}
          camOn={cameraOn}
          onToggleCamera={onToggleCamera}
          onCameraTrocada={onCameraTrocada}
          onClose={() => setPainelTx(false)}
        />
      )}
      {panelOpen && (
        <SalaPainel
          room={room}
          onSend={onSend}
          meId={meId}
          onClose={() => setPanelOpen(false)}
        />
      )}
    </div>
  )
}
