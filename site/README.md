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

- Sem build e sem JavaScript: HTML + `estilo.css`. As fontes (OFL, licenças em `fontes/`) e o ícone
  vão junto, então o site não chama nenhum serviço de fora (a política diz isso).
- Todos os caminhos são relativos: funciona na raiz de um domínio e também em
  `usuario.github.io/streamaZap/`.
- Visual: tokens e fontes de `docs/design/README.md`. `tela-salas.webp` é o `docs/design/prints/1-salas.png`.

## Ver no PC

```bash
python -m http.server 8787 --bind 127.0.0.1 --directory site
```

Ou `preview_start` com o nome `site` (`.claude/launch.json`).

## Antes de publicar

1. Trocar `EMAIL_CONTATO` pelo e-mail de contato em todos os arquivos. Conferir que não sobrou:
   `grep -rn EMAIL_CONTATO site` não pode achar nada.
2. O texto legal precisa passar por um advogado antes de abrir o Online ao público (inclusive a
   idade mínima de 18 anos e o ECA Digital, §14 do plano).
3. Para o que a política promete ficar verdade:
   - os logs do container do servidor (Docker) têm IP e hoje não têm rotação: pôr `max-size` no
     `docker-compose.yml` do servidor (frente H) para não passarem dos 6 meses;
   - "Supabase em São Paulo": conferir a região do projeto no painel (o plano diz `sa-east-1`).
4. Ao mudar o que o app ou o servidor guarda, atualizar a política (e a data da versão).

## Publicar

Qualquer hospedagem estática que sirva a pasta `site/` como raiz:

- **Cloudflare Pages:** projeto ligado ao repositório, sem comando de build, pasta de saída `site`,
  domínio próprio num subdomínio de `heitorcanhizares.net` (não `streamazap.`, que é o servidor).
- **GitHub Pages:** workflow com `actions/upload-pages-artifact` (`path: site`) e
  `actions/deploy-pages`, e Pages ligado em Settings → Pages → Source: GitHub Actions.

Depois, no Google Auth Platform → Branding: página inicial `https://<domínio>/`, política
`https://<domínio>/privacidade/`, termos `https://<domínio>/termos/`, e o domínio em
"Authorized domains" (verificado no Search Console).
