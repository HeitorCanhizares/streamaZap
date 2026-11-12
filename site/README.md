# site/

Site estático do streamaZap: página inicial, política de privacidade e termos de uso do beta
gratuito. É o que o Google Auth Platform pede (Branding: página inicial e política de privacidade)
para publicar o login com Google, e o que o §14 de `docs/planos/contas-planos-camera.md` pede antes
de abrir o Online.

| Página | Arquivo | Caminho publicado |
|---|---|---|
| Página inicial | `index.html` | `/` |
| Política de privacidade | `privacidade/index.html` | `/privacidade/` |
| Termos de uso do beta | `termos/index.html` | `/termos/` |
| Convite para sala | `sala/index.html` | `/sala/#ZAP-7K2Q` ou `/sala/#26.14.7.88` |

- Sem build: HTML + `estilo.css`. Só a página `sala/` tem um script curto, inline: lê o alvo do
  fragmento (`#ZAP-7K2Q`, que nunca vai ao servidor) e abre `streamazap://sala/<alvo>`, que o app
  valida de novo e usa para entrar na sala. O app copia esse link no **Copiar convite**.
- As fontes (OFL, licenças em `fontes/`) e o ícone vão junto, então o site não chama nenhum serviço
  de fora (a política diz isso).
- Todos os caminhos são relativos: funciona na raiz de um domínio e também em
  `usuario.github.io/streamaZap/`.
- Visual: tokens e fontes de `docs/design/README.md`. `tela-salas.webp` é o `docs/design/prints/1-salas.png`.

## Ver no PC

```bash
python -m http.server 8787 --bind 127.0.0.1 --directory site
```

Ou `preview_start` com o nome `site` (`.claude/launch.json`).

## No ar

GitHub Pages do repositório, servido pelo branch `gh-pages` (só o conteúdo de `site/`, sem o resto
do `master`):

- página inicial: <https://heitorcanhizares.github.io/streamaZap/>
- política de privacidade: <https://heitorcanhizares.github.io/streamaZap/privacidade/>
- termos de uso: <https://heitorcanhizares.github.io/streamaZap/termos/>

No Google Auth Platform → Branding: essas URLs, e `heitorcanhizares.github.io` em "Authorized
domains". Se o Google pedir para verificar o domínio, o Search Console dá um arquivo HTML ou uma
meta tag para a propriedade de prefixo de URL `https://heitorcanhizares.github.io/streamaZap/`: o
arquivo vai em `site/`, a meta tag no `<head>` de `index.html`. Se ele exigir o domínio inteiro
(`heitorcanhizares.github.io/`), aí precisa do repositório `HeitorCanhizares.github.io` ou de um
domínio próprio (CNAME num subdomínio de `heitorcanhizares.net`).

## Atualizar o site

Depois de commitar a mudança em `site/` no `master`:

```bash
git subtree split --prefix site --branch gh-pages
git push origin gh-pages
```

O `split` é determinístico: refaz o mesmo histórico e só acrescenta os commits novos, sem mexer no
branch atual. O Pages publica em um ou dois minutos. `.nojekyll` desliga o Jekyll do Pages.

## Cuidados

1. O texto legal precisa passar por um advogado antes de abrir o Online ao público (inclusive a
   idade mínima de 18 anos e o ECA Digital, §14 do plano).
2. Para o que a política promete ficar verdade:
   - os logs do container do servidor (Docker) têm IP e hoje não têm rotação: pôr `max-size` no
     `docker-compose.yml` do servidor (frente H) para não passarem dos 6 meses;
   - "Supabase em São Paulo": conferir a região do projeto no painel (o plano diz `sa-east-1`).
3. Ao mudar o que o app ou o servidor guarda, atualizar a política (e a data da versão).
