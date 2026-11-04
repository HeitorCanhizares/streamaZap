# M3b: fidelidade da interface ao design

A primeira passada (commit "Interface v3 (M3, primeira passada)") deixou a estrutura certa
(ponte, store, mock, player, componentes, navegação), mas o visual ficou simplificado. Esta passada
porta **cada trecho do HTML do design fielmente**, mantendo a ponte e os testes.

Regras para todas as frentes:
- Fonte da verdade: `docs/design/*.html` (marcação + estilos inline) e `docs/design/prints/*.png`.
  A lógica de demonstração está no `<script type="text/x-dc">` no fim de cada arquivo (estados,
  rótulos, cores por pessoa): use-a para saber como cada estado aparece.
- Copiar textos, ícones SVG (os `path`), tamanhos, espaçamentos, raios, sombras e cores. Cores
  pelas variáveis do `app/src/theme.css` (acrescente as que faltarem). Classes `zp-*` do `<style>`
  do design vão para o `theme.css`.
- `sc-if`/`sc-for`/`{{ }}` do design viram JSX com dados da ponte (`useSnapshot`) ou estado local.
  Sem vídeo real (mock no navegador), o lugar do vídeo mostra o `ScenePreview` da cena da pessoa,
  como no design.
- Cada tela só desenha o conteúdo do `<main>`/`<aside>` dela; barra de título e trilho são da
  frente `shell`.
- Teste de cada tarefa: renderiza com o mock e confere os textos e elementos do trecho.
- Fechamento: prints com Playwright (1440×900 e altura da página) comparados com
  `docs/design/prints/`.

## Frentes e trechos

| Frente | Trava | Trechos do design | Arquivos atuais |
|---|---|---|---|
| `shell` | `app-ui-shell` | `1-salas.html` 35–61 (barra: logo, versão, breadcrumb, status do servidor e do Radmin, botões da janela; trilho com 4 itens, indicador de sala ao vivo de `3-sala.html` 63, avatar do perfil no rodapé) | `components/TitleBar.tsx`, `components/NavRail.tsx`, `App.tsx` |
| `previa` | `app-ui-previa` | `componente-previa.html` inteiro (cenas `game`, `race`, `movie`, `code`, `desktop`, `cama`–`camd`, moldura de cantos, rótulos) | `components/ScenePreview.tsx` (+ moldura reutilizável `VideoFrame`) |
| `salas` | `app-ui-salas` | `1-salas.html` 63–141 (coluna da lista: título + "Nova sala", abas Online/Local com legenda, busca, filtros com contagem, itens com avatar colorido com borda, cadeado, hora, última mensagem, não lidas, olho/qualidade/ping, status do servidor, "Você aparece como", "Entrar com código") e 142–256 (sala selecionada: cabeçalho com selos, prévia com moldura, card de acesso com "Entrar na sala →", ping/suavização, "Compartilhando agora", "Na sala" com ping por pessoa, aviso de firewall) | `screens/salas/*` |
| `criar` | `app-ui-criar` | `2-criar-sala.html` 59–199 (seções 01–06) e 200–fim do `x-dc` (resumo à direita com prévia, ficha, código da sala, copiar convite) | `screens/criar/*` |
| `sala` | `app-ui-sala` | `3-sala.html` 70–142 (cabeçalho, vídeo principal com moldura, selo AO VIVO, métricas, PiP da câmera, barra de controles em pílula com ícones), 143–195 (faixa de tiles com volume, mudo, fechar, "Você" com Câmera/Tela), 196–260 (painel Chat/Pessoas: abas, separador HOJE, mensagens de sistema, bolhas próprias à direita com ✓✓, digitando, campo e botão redondo) | `screens/sala/*` |
| `config` | `app-ui-config` | `4-configuracoes.html` 59–120 (título, "Tudo salva sozinho", Perfil, Sobre/atualização), 121–176 (Conexão), 177–216 (Vídeo e câmera), 217–fim (Som e avisos, atalhos) | `screens/config/*` |
| `diag` | `app-ui-diag` | `5-diagnostico.html` 59–89 (título, botões, resumo com ícone e selos), 90–120 (cards em grade: servidor, rede, firewall, amigos, salas), 121–fim ("Como ler isso") | `screens/diag/*` |

Dependências: todas as telas dependem de `previa` (moldura e cenas); `shell` é independente.
