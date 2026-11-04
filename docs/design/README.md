# Design da nova interface

Referência visual da v3. Os `.html` são as telas exportadas do protótipo (marcação com estilos
inline + a lógica de demonstração no `<script type="text/x-dc">` do fim de cada arquivo). Servem de
fonte para o porte em React (`app/src`); não são carregados pelo app.

| Tela | Arquivo | Print |
|---|---|---|
| 1 · Salas (tela inicial) | `1-salas.html` | `prints/1-salas.png` |
| 2 · Criar sala | `2-criar-sala.html` | `prints/2-criar-sala.png` |
| 3 · Dentro da sala | `3-sala.html` | `prints/3-sala.png` |
| 4 · Configurações | `4-configuracoes.html` | `prints/4-configuracoes.png` |
| 5 · Diagnóstico | `5-diagnostico.html` | `prints/5-diagnostico.png` |
| Componente · prévia de vídeo | `componente-previa.html` | — |

`icone.png` nos HTML = `streamazap/assets/icon.png`.

## Tokens

| Token | Valor | Uso |
|---|---|---|
| `--bg` | `#0A100E` | fundo do app |
| `--bg-barra` | `#0C1412` | barra de título e trilho lateral |
| `--superficie` | `#0F1916` | cartões |
| `--superficie-2` | `#16241F` | botões secundários, campos |
| `--hover` | `#1A2C27` | hover de botões fantasmas |
| `--borda` | `#1F332D` | bordas |
| `--borda-forte` | `#2A443C` | bordas de destaque, divisórias |
| `--texto` | `#E6F4EC` | texto principal |
| `--texto-2` | `#A3BDB1` | texto secundário |
| `--texto-3` | `#7E978B` | placeholder, legendas |
| `--verde` | `#3BF08A` | destaque, ações primárias, "online", ping bom |
| `--verde-hover` | `#6CF5A8` | hover do primário |
| `--verde-texto` | `#03140A` | texto sobre o verde |
| `--amarelo` | `#FFB547` | atenção, ping médio |
| `--vermelho` | `#FF5D6E` | ao vivo, sair, erro (`#FF8592` para texto) |
| `--azul` | `#5CD6FF` | Radmin / rede local |

Cores dos participantes (escolha no perfil): `#3BF08A`, `#5CD6FF`, `#FFB547`, `#FF8FB1`,
`#B79CFF`, `#FF9E5C`.

Ping: `< 60 ms` verde, `< 100 ms` amarelo, senão vermelho.

## Tipografia

- **Unbounded** (700): logotipo e títulos grandes.
- **Space Grotesk** (400–700): texto da interface (14 px base).
- **JetBrains Mono** (400/600): números, códigos de sala, métricas, rótulos em caixa alta.

As três fontes são OFL e vão embutidas no app (`@fontsource/*`), porque o app precisa funcionar
sem internet (Radmin).
