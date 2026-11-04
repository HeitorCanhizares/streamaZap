# REST (HTTPS)

Base: `https://<servidor>:7443/v1`. Corpo e resposta em JSON. Autenticação por
`Authorization: Bearer <token>`, menos em `GET /info` e `POST /sessoes`.

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
  "ponte": true,
  "quic": { "porta": 7443, "alpn": "zap/4" },
  "limites": { "participantes_por_sala": 16, "trilhas_video_por_sala": 8, "bitrate_max_kbps": 12000 },
  "agora": "2026-11-04T21:30:00Z"
}
```

`ponte: false` = o servidor só faz sinalização (a mídia precisa ir direta).

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
  "senha_servidor": "…"
}
```
- `instalacao_id`: UUID fixo da instalação (o mesmo participante reconectando é reconhecido).
- `senha_servidor`: só se `requer_senha`.

Resposta `201`:
```json
{ "token": "…", "expira_em": "2026-11-05T21:30:00Z", "participante_id": "p_8Kq2mZ0aL1", "ip_publico": "177.12.34.56" }
```

## `POST /v1/sessoes/renovar`

Sem corpo. Resposta igual à de criação, com token novo. O antigo vale mais 5 min.

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

A sala some 2 min depois que o último participante sai.

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

## `POST /v1/salas/{codigo}/sair`

Sem corpo. `204`. O WebSocket recebe `sala.saiu`.

## `DELETE /v1/salas/{codigo}`

Só o dono. Encerra a sala; todos recebem `sala.encerrada`. `204`.

## `POST /v1/salas/{codigo}/previa`

Só salas `aberta`. Resposta:
```json
{ "ingresso": "…", "expira_em": "…", "midia": { "ponte": { "endereco": "…", "alpn": "zap/4" } } }
```
Com esse ingresso o app só pode assinar a **camada 2** das trilhas de vídeo (sem áudio, sem chat).

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
