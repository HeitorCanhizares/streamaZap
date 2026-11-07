# REST (HTTPS)

Base: `https://<servidor>:7443/v1`. Corpo e resposta em JSON. Autenticação por
`Authorization: Bearer <token>`, menos em `GET /info` e `POST /sessoes`.

`{codigo}` nos caminhos aceita o id (`s_…`) ou o código de convite em qualquer forma: `ZAP-7K2Q`,
`zap-7k2q` ou só `7K2Q`. Caminho inexistente → `404 {"erro": "nao_encontrado"}`.

## `GET /v1/info`

Usado pelo botão **Testar conexão** e antes de toda sessão.

```json
{
  "nome": "Servidor do Heitor",
  "versao_servidor": "1.0.0",
  "protocolo_min": 4,
  "protocolo_max": 4,
  "regiao": "sa-east",
  "requer_senha": true,
  "requer_conta": true,
  "conta": { "provedor": "supabase", "url": "https://abcd1234.supabase.co", "chave": "sb_publishable_…" },
  "ponte": true,
  "quic": { "porta": 7443, "alpn": "zap/4" },
  "limites": { "participantes_por_sala": 16, "trilhas_video_por_sala": 8, "bitrate_max_kbps": 12000 },
  "agora": "2026-11-04T21:30:00Z"
}
```

`ponte: false` = o servidor só faz sinalização (a mídia precisa ir direta).

- `requer_conta` (ausente = `false`): sem `acesso_conta` válido, `POST /sessoes` responde
  `conta_necessaria`.
- `conta` (ausente = o servidor não confere contas): onde o app faz login. `url` e `chave` (a chave
  publicável do projeto Supabase, pública por definição) são o que o app usa para criar o cliente
  de login; assim um servidor próprio aponta para o próprio projeto. Servidor com `conta` e sem
  `requer_conta` aceita a conta se vier (vale para o plano), mas não exige. Servidor próprio sem
  `conta`: o app nem mostra login.
- Login anônimo do Supabase (`is_anonymous: true` no JWT) não vale como conta.

## `POST /v1/sessoes`

Cria a sessão do app.

Pedido:
```json
{
  "nome": "Heitor",
  "cor": "#3BF08A",
  "instalacao_id": "9f1c…",
  "protocolo": 4,
  "versao_app": "3.0.0",
  "senha_servidor": "…",
  "acesso_conta": "eyJhbGciOiJFUzI1NiIs…"
}
```
- `instalacao_id`: UUID fixo da instalação (o mesmo participante reconectando é reconhecido).
- `senha_servidor`: só se `requer_senha`.
- `acesso_conta`: o JWT de acesso do Supabase (o `access_token` da sessão do usuário), só quando o
  servidor tem `conta`. O servidor confere a assinatura pela JWKS de `<conta.url>/auth/v1/.well-known/jwks.json`
  (em cache, sem chamar o Supabase por sessão), `iss = <conta.url>/auth/v1`, `aud = authenticated`
  e `exp` (60 s de folga), e lê o claim `plano`. Claim ausente ou desconhecido = `nenhum`.
- Com `requer_senha` **e** `requer_conta`, os dois são obrigatórios: o servidor confere a senha
  primeiro (`senha_servidor`), depois a conta (`conta_necessaria`).
- A mesma conta em duas instalações vira dois participantes.

Resposta `201`:
```json
{
  "token": "…", "expira_em": "2026-11-05T21:30:00Z", "participante_id": "p_8Kq2mZ0aL1", "ip_publico": "177.12.34.56",
  "conta_id": "2b0c6c1e-…",
  "plano": {
    "id": "hd",
    "pode_entrar": true,
    "transmitir": { "altura_max": 720, "fps_max": 60, "tela_kbps": 4000, "camera_kbps": 2500 }
  }
}
```
- `conta_id` e `plano`: só quando a sessão tem conta. **Sem conta, não há limite de plano**: até o
  servidor ligar `requer_conta`, quem entra só com a senha (apps sem login) não é limitado por plano.
  O teto de banda por espectador vale para todos.
- `plano.id`: `"nenhum"`, `"espectador"`, `"hd"` ou `"fullhd"`. Os limites vêm de
  `zap_proto::plan::limits` (a fonte dos números): `pode_entrar` (criar e entrar em sala Online) e
  `transmitir` (`null` = não transmite; senão o teto da camada 0 de cada trilha: altura, fps e kbps
  da tela/janela e da câmera). Áudio só pede `transmitir` diferente de `null`.
- Assistir não depende do plano: o espectador recebe a melhor camada que a conexão aguenta, até a que
  quem transmite mandar, com o teto de banda de 12000 kbps por espectador
  ([`balanceamento.md`](balanceamento.md) §2).

## `POST /v1/sessoes/renovar`

Corpo opcional `{ "acesso_conta": "…" }` (token novo do Supabase, depois de um login, de uma
renovação ou de assinar um plano). Resposta igual à de criação, com token novo. O antigo vale mais
5 min.

O `renovar` **sempre** renova o token da sessão (Supabase fora do ar não derruba sessão aberta):
- sem corpo: mantém conta e plano;
- `acesso_conta` válido: atualiza o plano, ou liga a conta numa sessão que entrou só com a senha
  (login depois de entrar, sem reconectar). O plano novo vale para salas e trilhas **novas** (nada
  em andamento é derrubado);
- `acesso_conta` vencido ou inválido: ignorado; a sessão continua com o plano de antes;
- `acesso_conta` de outra conta (`sub` diferente da conta da sessão): `403 proibido`. Trocar de conta
  = sessão nova (`POST /sessoes`).

`conta_necessaria` só sai do `POST /v1/sessoes`, nunca do `renovar`.

## `GET /v1/salas`

Lista as salas visíveis (o mesmo conteúdo do evento `salas.lista` do WebSocket).

```json
{
  "salas": [
    {
      "id": "s_x81Pq0",
      "codigo": "ZAP-7K2Q",
      "nome": "Sexta do Rush",
      "dono": { "id": "p_8Kq2mZ0aL1", "nome": "Marcão", "cor": "#FFB547" },
      "acesso": "aberta",
      "criada_em": "2026-11-04T21:30:00Z",
      "participantes": 4,
      "ao_vivo": true,
      "destaque": { "altura": 1080, "fps": 60 },
      "ultima_mensagem": { "nome": "Rafa", "texto": "bora, entra logo", "em": "2026-11-04T22:14:00Z" },
      "nao_lidas": 3
    }
  ]
}
```
- `destaque`: qualidade (camada 0) da trilha de vídeo principal, para o selo `1080p60` da lista.
- `ultima_mensagem` e `nao_lidas` só para quem já esteve na sala.

## `POST /v1/salas`

Cria uma sala. Quem cria vira dono.

```json
{ "nome": "Sala do Heitor", "acesso": "senha", "senha": "…" }
```
Resposta `201`: `{ "sala": { …mesmo formato da lista… } }` (inclui o `codigo`).

- `nome`: 1 a 48 caracteres; `senha`: 1 a 128.
- Quem cria já fica autorizado na sala: pode mandar `sala.entrar` sem chamar `POST …/entrar`.
- Sessão com conta de plano sem `pode_entrar` (`nenhum`): `403 plano_insuficiente`.
- A sala some 2 min depois que o último participante sai.

## `GET /v1/salas/{codigo}`

Detalhes de uma sala (para a tela de "Entrar com código"). Mesmo formato da lista, mais:
```json
{ "participantes_lista": [ { "id": "…", "nome": "Ju", "cor": "#7CF2C0", "compartilhando": ["camera"] } ] }
```
Em sala com senha, `participantes_lista` só aparece depois de entrar.

## `POST /v1/salas/{codigo}/entrar`

```json
{ "senha": "…" }
```
Resposta `200`:
```json
{
  "sala": { "…": "formato da lista" },
  "participante_id": "p_…",
  "ingresso": "…",
  "ingresso_expira_em": "2026-11-04T22:30:00Z",
  "midia": {
    "ponte": { "endereco": "zap.seudominio.com:7443", "alpn": "zap/4" },
    "direta_permitida": true
  }
}
```
O `ingresso` vai na mensagem `ola` da conexão QUIC (ver [`midia.md`](midia.md)). Renovar: chamar
`entrar` de novo (idempotente).

Sessão com conta de plano sem `pode_entrar` (`nenhum`): `403 plano_insuficiente`. A lista e a
`previa` continuam liberadas. Sessão que já está autorizada na sala (é a dona, ou entrou e não
saiu) não é conferida de novo: renovar o ingresso ou reconectar não derruba quem já está, mesmo com
o plano vencido no meio.

Servidor sem ponte (`ponte: false` em `/info`): `midia.ponte` vem `null`. O cliente deve aceitar
`null` ou o campo ausente.

## `POST /v1/salas/{codigo}/sair`

Sem corpo. `204`. O WebSocket recebe `sala.saiu`. Sair tira a autorização e invalida os ingressos
dessa sala: para voltar, é preciso chamar `entrar` de novo. Quem criou a sala continua autorizado.

## `DELETE /v1/salas/{codigo}`

Só o dono. Encerra a sala; todos recebem `sala.encerrada`. `204`.

## `POST /v1/salas/{codigo}/previa`

Só salas `aberta`. Resposta:
```json
{ "ingresso": "…", "expira_em": "…", "midia": { "ponte": { "endereco": "…", "alpn": "zap/4" } } }
```
Com esse ingresso o app só pode assinar a **camada 2** das trilhas de vídeo (sem áudio, sem chat).

- Servidor sem ponte: `403 {"erro": "proibido"}` (não há prévia sem ponte).
- A conexão QUIC de prévia recebe `sala.estado` (sem histórico do chat). Só pode assinar vídeo, e
  sempre recebe a camada mais barata (teto 2). Qualquer outra coisa → `erro` `proibido`.
- Chamar `previa` de novo estende também a conexão de prévia já aberta. Sem renovação, ela é
  fechada 5 s depois de vencer.

## `GET /v1/diagnostico`

Usado pela tela **Diagnóstico**.

```json
{ "ip_publico": "177.12.34.56", "porta_publica_udp": 51234, "protocolo_ok": true, "agora": "…" }
```
`porta_publica_udp` é a porta de origem vista pelo servidor na última conexão QUIC desse
participante (ajuda a saber se a rota direta tem chance).

## `GET /v1/ws?token=…`

Abre o WebSocket de sinalização. Mensagens em [`mensagens.md`](mensagens.md), uma por frame de
texto. O servidor manda `ping` a cada 15 s; o app responde `pong`. Sem tráfego por 45 s = conexão
morta. Ao reconectar, o app manda `sala.entrar` de novo e recebe `sala.estado` completo.
