// Tela cheia da janela. No app, pela janela do Tauri: o WebView2 do wry não leva a tela cheia de um
// elemento até a janela, que continuaria do mesmo tamanho. Na interface de teste (navegador), pela
// Fullscreen API. A tela cheia de uma transmissão é a janela em tela cheia com a transmissão sozinha
// por cima de tudo (SalaPrincipal).
import { isTauri } from '../../bridge/api'

async function janela(): Promise<import('@tauri-apps/api/window').Window> {
  const { getCurrentWindow } = await import('@tauri-apps/api/window')
  return getCurrentWindow()
}

/** Põe a janela em tela cheia e diz se ela já estava. */
export async function entrarTelaCheia(): Promise<boolean> {
  if (!isTauri()) {
    // Sem await antes do pedido: o navegador só aceita durante o clique ou a tecla.
    const ja = document.fullscreenElement != null
    if (!ja) await document.documentElement.requestFullscreen?.()
    return ja
  }
  const w = await janela()
  const ja = await w.isFullscreen()
  if (!ja) await w.setFullscreen(true)
  return ja
}

/** Tira a janela da tela cheia (nada acontece se ela não estiver). */
export async function sairTelaCheia(): Promise<void> {
  if (!isTauri()) {
    if (document.fullscreenElement) await document.exitFullscreen?.()
    return
  }
  const w = await janela()
  if (await w.isFullscreen()) await w.setFullscreen(false)
}

/** Botão e F11 da sala: alterna a tela cheia da janela (o app inteiro). */
export async function alternarTelaCheia(): Promise<void> {
  if (!isTauri()) {
    if (document.fullscreenElement) await sairTelaCheia()
    else await entrarTelaCheia()
    return
  }
  const w = await janela()
  await w.setFullscreen(!(await w.isFullscreen()))
}
