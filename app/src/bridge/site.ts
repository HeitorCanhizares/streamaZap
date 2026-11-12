// Endereços do site do streamaZap (GitHub Pages, branch `gh-pages`, fonte em `site/`). Sem
// imports, para os modelos de texto usarem sem ciclo.

/** Raiz do site. */
export const SITE = 'https://heitorcanhizares.github.io/streamaZap'

/**
 * Link https do convite: `…/sala/#ZAP-7K2Q` (Online) ou `…/sala/#26.14.7.88` (Local). A página
 * abre `streamazap://sala/<alvo>`; https porque WhatsApp e Discord não deixam clicar no esquema
 * do app.
 */
export function linkConvite(alvo: string): string {
  return `${SITE}/sala/#${alvo}`
}
