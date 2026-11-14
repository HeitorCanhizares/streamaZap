// Tarefa ui-22: tela "Dentro da sala". Compõe SalaPrincipal, SalaTiles, a barra da sala
// (SalaControles, no pé da coluna) e o SalaPainel. Dona do estado compartilhado (lugar de cada
// transmissão, volumes, tela cheia, painel) e das assinaturas: toda transmissão de outra pessoa é
// assinada sozinha ao aparecer (useAssinaturas).
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from 'react'
import { api, isTauri } from '../../bridge/api'
import { useSnapshot } from '../../bridge/store'
import { useTravas } from '../../bridge/usePlano'
import { Button } from '../../components/Button'
import { SalaControles } from './SalaControles'
import { SalaPainel } from './SalaPainel'
import { SalaPrincipal } from './SalaPrincipal'
import { SalaTiles } from './SalaTiles'
import { SalaTransmissao } from './SalaTransmissao'
import { alternarTelaCheia, entrarTelaCheia, sairTelaCheia } from './telaCheia'
import {
  celulasDaPrincipal,
  inviteText,
  lugaresDasStreams,
  MUSICA,
  ownTracks,
  streamsDaSala,
  volumeInicial,
  type JeitosCamera,
  type Lugar,
  type VolumePessoa,
} from './salaModel'
import { useAssinaturas, type PedidoStream } from './useAssinaturas'
import { CamadaMusica, MusicaContexto, type MusicaCtx } from './musica/BlocoMusica'

export interface SalaScreenProps { onLeft: () => void }

/** Altura (px) de um elemento, medida antes de pintar e acompanhada pelo ResizeObserver (sem ele,
 *  como no jsdom, só a medida inicial). Devolve a ref de callback e a altura (0 sem elemento). */
function useAltura(): [(el: HTMLElement | null) => void, number] {
  const [el, setEl] = useState<HTMLElement | null>(null)
  const [altura, setAltura] = useState(0)
  useLayoutEffect(() => {
    if (!el) {
      setAltura(0)
      return
    }
    const medir = () => setAltura(Math.ceil(el.getBoundingClientRect().height))
    medir()
    if (typeof ResizeObserver === 'undefined') return
    const ro = new ResizeObserver(medir)
    ro.observe(el)
    return () => ro.disconnect()
  }, [el])
  return [setEl, altura]
}

const CHAVE_VOCE_OCULTO = 'streamazap.voceOculto'
function lerVoceOculto(): boolean {
  try {
    return globalThis.localStorage?.getItem(CHAVE_VOCE_OCULTO) === '1'
  } catch {
    return false
  }
}
function gravarVoceOculto(v: boolean): void {
  try {
    globalThis.localStorage?.setItem(CHAVE_VOCE_OCULTO, v ? '1' : '0')
  } catch {
    // Sem armazenamento: vale só nesta sala.
  }
}

export function SalaScreen({ onLeft }: SalaScreenProps): ReactNode {
  const snap = useSnapshot()
  const room = snap.room

  // Onde o usuário pôs cada transmissão (principal, faixa ou fechada); sem escolha vale o padrão.
  const [escolhas, setEscolhas] = useState<Record<string, Lugar>>({})
  // Volume que escolhi para cada pessoa (0..100).
  // Como quero ver cada câmera (no canto da tela ou solta); só local, não vai ao núcleo.
  const [jeitos, setJeitos] = useState<JeitosCamera>({})
  const [volumes, setVolumes] = useState<Record<string, VolumePessoa>>({})
  const [cameraOn, setCameraOn] = useState(false)
  const [aviso, setAviso] = useState<string | null>(null)
  const [panelOpen, setPanelOpen] = useState(true)
  // Transmissão em tela cheia (trackId) e se a janela já estava em tela cheia antes dela (ao sair,
  // a janela volta como estava).
  const [cheia, setCheia] = useState<string | null>(null)
  const janelaJaCheia = useRef(false)
  const travas = useTravas(room?.mode ?? 'local')
  // Cartão Você oculto (escolha lembrada entre as salas).
  const [voceOculto, setVoceOcultoEstado] = useState(() => lerVoceOculto())
  const setVoceOculto = (v: boolean) => {
    setVoceOcultoEstado(v)
    gravarVoceOculto(v)
  }
  const [painelTx, setPainelTx] = useState(false)
  // Música da sala: o volume que escolhi para ela e se o player está num anúncio (ou carregando).
  const [volMusica, setVolMusica] = useState<VolumePessoa>({ vol: 80, muted: false })
  const [musicaTravada, setMusicaTravada] = useState(false)
  // Onde a célula ou o cartão da música deixou a vaga do player, e a coluna que rola (a camada do
  // player é filha dela).
  const [vagaMusica, setVagaMusica] = useState<HTMLElement | null>(null)
  const colunaRef = useRef<HTMLDivElement | null>(null)

  // Meu id nesta sala (no Online vem do servidor); a câmera está ligada se a trilha dela existe.
  const meId = room?.me_id || snap.me.install_id
  const temCamera = room ? ownTracks(room, meId).camera !== null : false
  useEffect(() => {
    setCameraOn(temCamera)
  }, [temCamera])

  // Transmissões (dos outros e as minhas), onde cada uma fica e o que assinar (o canto = câmera
  // sobre a tela).
  const streams = room ? streamsDaSala(room, meId) : []
  const lugares = lugaresDasStreams(streams, escolhas)
  const celulas = celulasDaPrincipal(streams, lugares, jeitos)
  const cantos = new Set(celulas.flatMap((c) => (c.pip ? [c.pip.trackId] : [])))
  // A minha miniatura com o cartão Você oculto não é assinada (não gasta codificação à toa).
  const pedidos: PedidoStream[] = streams
    // A música não é assinada: cada um toca o vídeo no próprio player.
    .filter((s) => s.kind !== 'musica')
    .filter((s) => lugares[s.trackId] !== 'fechada')
    .filter((s) => !(s.mine && voceOculto && lugares[s.trackId] === 'faixa'))
    .map((s) => ({
      stream: s,
      lugar: cantos.has(s.trackId) ? 'canto' : lugares[s.trackId] === 'principal' ? 'principal' : 'faixa',
    }))
  // Em tela cheia, a transmissão pede o tamanho de quem está sozinha na principal.
  const canvasDe = useAssinaturas(pedidos, cheia ? 1 : celulas.length)
  // Dos outros, fora da principal (as minhas ficam no cartão Você).
  const naFaixa = streams.filter((s) => !s.mine && lugares[s.trackId] !== 'principal')
  const minhaTela = streams.find((s) => s.mine && s.kind !== 'camera') ?? null
  const minhaCam = streams.find((s) => s.mine && s.kind === 'camera') ?? null
  const minhasForaDaPrincipal = streams.some((s) => s.mine && lugares[s.trackId] !== 'principal')
  // O cartão "Você" só aparece quando transmito (o botão de compartilhar já está nos controles).
  const mostrarVoce = !!snap.share || temCamera || cameraOn
  // Altura da faixa de baixo (cartões, botões das fechadas, música em uma ou duas linhas): o vídeo
  // encolhe o que ela ocupa para a sala caber na janela sem rolar.
  const [refFaixa, alturaFaixa] = useAltura()

  // O som é de cada transmissão (volume de cada pessoa e o da música): o volume geral do núcleo fica
  // em 100% ao entrar na sala, mesmo que uma versão antiga o tenha deixado mais baixo ou mudo.
  useEffect(() => {
    void Promise.resolve(api.setMasterVolume(1, false)).catch(() => {})
  }, [])

  // Tela cheia de uma transmissão: a janela entra em tela cheia junto; ao sair, volta como estava.
  const onTelaCheia = (trackId: string | null) => {
    if (trackId) {
      if (cheia === null) {
        janelaJaCheia.current = !isTauri() && document.fullscreenElement != null
        void entrarTelaCheia().then((ja) => { janelaJaCheia.current = ja }).catch(() => {})
      }
      setCheia(trackId)
    } else if (cheia !== null) {
      setCheia(null)
      if (!janelaJaCheia.current) void sairTelaCheia().catch(() => {})
    }
  }
  // A transmissão em tela cheia acabou (ou saiu da principal): sai da tela cheia.
  const cheiaNaPrincipal = cheia !== null && celulas.some((c) => c.stream.trackId === cheia)
  useEffect(() => {
    if (cheia === null || cheiaNaPrincipal) return
    setCheia(null)
    if (!janelaJaCheia.current) void sairTelaCheia().catch(() => {})
  }, [cheia, cheiaNaPrincipal])
  // No navegador, o Esc da Fullscreen API tira a janela da tela cheia sem passar pelo onKey.
  useEffect(() => {
    if (cheia === null || isTauri()) return
    const aoMudar = () => {
      if (!document.fullscreenElement) setCheia(null)
    }
    document.addEventListener('fullscreenchange', aoMudar)
    return () => document.removeEventListener('fullscreenchange', aoMudar)
  }, [cheia])
  // Saiu da sala com uma transmissão em tela cheia: a janela sai da tela cheia também.
  const cheiaRef = useRef(cheia)
  cheiaRef.current = cheia
  useEffect(() => () => {
    if (cheiaRef.current !== null && !janelaJaCheia.current) void sairTelaCheia().catch(() => {})
  }, [])
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

  // Uma escolha fixa onde tudo está agora (pôr outra na principal soma, não troca). A câmera no
  // canto de uma tela desce junto quando a tela sai da principal; a minha tela subindo leva a minha
  // câmera, que estava no canto do cartão Você. Calculado sobre o estado mais novo: duas mudanças
  // seguidas não se apagam.
  const onMover = (trackId: string, lugar: Lugar) => {
    setEscolhas((e) => {
      const atuais = lugaresDasStreams(streams, e)
      const extra: Record<string, Lugar> = {}
      const canto = celulasDaPrincipal(streams, atuais, jeitos).find((c) => c.stream.trackId === trackId)?.pip
      if (canto && lugar !== 'principal') extra[canto.trackId] = 'faixa'
      const st = streams.find((x) => x.trackId === trackId)
      if (st?.mine && st.kind !== 'camera' && lugar === 'principal' && minhaCam && atuais[minhaCam.trackId] === 'faixa') {
        extra[minhaCam.trackId] = 'principal'
      }
      return { ...atuais, ...extra, [trackId]: lugar }
    })
  }

  const onJeito = (trackId: string, jeito: 'canto' | 'solta') => setJeitos((j) => ({ ...j, [trackId]: jeito }))

  const volumeDe = (ownerId: string): VolumePessoa =>
    volumes[ownerId] ?? volumeInicial(room.participants.find((p) => p.id === ownerId))
  const onVolumePessoa = (ownerId: string, v: VolumePessoa) => {
    setVolumes((m) => ({ ...m, [ownerId]: v }))
    void Promise.resolve(api.setVolume(ownerId, v.vol / 100, v.muted)).catch(() => {})
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

  // Tela cheia do app inteiro (botão da barra e F11).
  const onFullscreen = () => {
    void alternarTelaCheia().catch(() => {})
  }

  const onSend = (text: string) => {
    void api.chatSend(text).catch(() => {})
  }

  const atualMusica = room.music?.current ?? null
  // Fechada = não toca para mim (o player é desmontado).
  const musicaAberta = !!atualMusica && lugares[MUSICA] !== undefined && lugares[MUSICA] !== 'fechada'
  const musica: MusicaCtx | null = atualMusica
    ? {
        atual: atualMusica,
        volume: volMusica,
        onVolume: setVolMusica,
        onReport: (r) => void Promise.resolve(api.musicReport(r)).catch(() => {}),
        onPausar: (p) => void Promise.resolve(api.musicPause(atualMusica.item.id, p)).catch(() => {}),
        onBuscar: (ms) => void Promise.resolve(api.musicSeek(atualMusica.item.id, ms)).catch(() => {}),
        // Sem player tocando não há anúncio a avisar.
        travado: musicaTravada && musicaAberta,
        onTravado: setMusicaTravada,
        registrarVaga: setVagaMusica,
      }
    : null

  return (
    // Ocupa a altura da janela: o vídeo e a faixa rolam do lado esquerdo e o chat tem a rolagem
    // dele, sem a página inteira crescer com as mensagens. A música da sala vai pelo contexto.
    <MusicaContexto.Provider value={musica}>
      <div style={{ display: 'flex', alignItems: 'stretch', height: '100%', minHeight: 0 }}>
        {/* Base 0: a coluna fica com o que sobra do painel (até 400 px), seja qual for a largura do
            que está dentro dela (a faixa com a música é larga e espremia o chat). */}
        <div style={{ flex: '1 1 0', minWidth: 0, minHeight: 0, display: 'flex', flexDirection: 'column' }}>
          <div ref={colunaRef} data-testid="sala-coluna" style={{ position: 'relative', flex: '1 1 auto', minWidth: 0, minHeight: 0, overflowY: 'auto', display: 'flex', flexDirection: 'column' }}>
            <SalaPrincipal
              room={room}
              celulas={celulas}
              tiles={snap.tiles}
              canvasDe={canvasDe}
              temOutras={naFaixa.length > 0 || minhasForaDaPrincipal}
              alturaFaixa={alturaFaixa}
              volumeDe={volumeDe}
              onVolumePessoa={onVolumePessoa}
              onMover={onMover}
              onJeito={onJeito}
              share={snap.share}
              onToggleCamera={onToggleCamera}
              onToggleShare={onToggleShare}
              onStopShare={onStopShare}
              onFullscreen={onFullscreen}
              onSairTelaCheia={() => { void sairTelaCheia().catch(() => {}) }}
              telaCheia={cheia}
              onTelaCheia={onTelaCheia}
              panelOpen={panelOpen}
              onOpenPanel={() => setPanelOpen(true)}
              invite={invite}
            />
            {(aviso || naFaixa.length > 0 || mostrarVoce) && (
              <div ref={refFaixa} data-testid="faixa-sala" style={{ padding: '0 28px 12px', display: 'flex', flexDirection: 'column', gap: 8 }}>
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
                  voceOculto={voceOculto}
                  onVoceOculto={setVoceOculto}
                  minhaTela={minhaTela}
                  minhaCam={minhaCam}
                  meId={meId}
                  camOn={cameraOn}
                  sharing={!!snap.share}
                  onToggleCam={onToggleCamera}
                  onToggleShare={onToggleShare}
                />
              </div>
            )}
            {/* O player da música, um só: segue a vaga da célula ou do cartão sem mudar de lugar no DOM. */}
            {musicaAberta && <CamadaMusica coluna={colunaRef} vaga={vagaMusica} />}
          </div>
          {/* Barra da sala: no pé da coluna, fora das transmissões e sempre visível. */}
          <div style={{ flex: '0 0 auto', display: 'flex', justifyContent: 'center', padding: '8px 28px 16px' }}>
            <SalaControles
              camOn={cameraOn}
              sharing={!!snap.share}
              onFullscreen={onFullscreen}
              onToggleCam={onToggleCamera}
              onToggleShare={onToggleShare}
              onLeave={() => { void onLeave() }}
              podeTransmitir={travas.podeTransmitir}
            />
          </div>
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
    </MusicaContexto.Provider>
  )
}
