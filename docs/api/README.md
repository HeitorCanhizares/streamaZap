# API do streamaZap (protocolo 4)

Este é o contrato entre o app streamaZap e o **servidor streamaZap** (modo *Online*), e também
entre dois apps no modo *Local* (Radmin/LAN). Ele foi escrito para que o servidor possa ser
implementado em qualquer linguagem.

| Documento | Conteúdo |
|---|---|
| [`rest.md`](rest.md) | Endpoints HTTPS: informação do servidor, sessão, salas, convites, prévia |
| [`mensagens.md`](mensagens.md) | Mensagens de controle (JSON), usadas no WebSocket do servidor **e** no stream de controle QUIC |
| [`midia.md`](midia.md) | Conexão QUIC `zap/4`: quadros de vídeo, áudio, sondas, relatórios do receptor |
| [`balanceamento.md`](balanceamento.md) | Algoritmo de qualidade individual por espectador (camadas + banda) |

## Visão geral

```
                 HTTPS + WebSocket (TCP 7443)            QUIC zap/4 (UDP 7443)
   ┌────────┐   sessão, salas, chat, sinalização   ┌──────────────┐   mídia (ponte)   ┌────────┐
   │  App A │ ───────────────────────────────────► │   Servidor   │ ◄────────────────► │  App B │
   │ (publica)◄──────────── QUIC zap/4 ──────────►│  streamaZap  │                    │(assiste)│
   └────────┘          mídia (ponte)               └──────────────┘                    └────────┘
        ▲                                                                                   ▲
        └─────────────────────── QUIC zap/4 direto (quando possível) ──────────────────────┘
```

- **Controle** (sessão, salas, chat, quem compartilha o quê): REST + WebSocket no servidor.
- **Mídia**: sempre QUIC com ALPN `zap/4`. Pode ir **direto** entre os apps (endereços trocados pela
  sinalização) ou passar pelo servidor, que funciona como **ponte** (SFU: recebe cada camada uma
  vez de quem publica e repassa a camada certa para cada espectador).
- No **modo Local** não há servidor: quem cria a sala faz o papel de servidor de controle
  (as mesmas mensagens de [`mensagens.md`](mensagens.md), pelo stream de controle QUIC), e cada
  pessoa que compartilha serve a própria mídia por QUIC.

## Conceitos

| Termo | Significado |
|---|---|
| **Participante** | Um app conectado numa sala. Id gerado pelo servidor (`p_` + 10 caracteres). |
| **Sala** | Id (`s_…`), código de convite `ZAP-XXXX` (4 caracteres de `23456789ABCDEFGHJKMNPQRSTUVWXYZ`, sem 0/O/1/I/L), nome, acesso (`aberta` ou `senha`). |
| **Trilha** | Mídia publicada por um participante: `tela`, `janela`, `camera` ou `audio`. Id `t_…`. Cada participante publica no máximo uma trilha de tela **ou** janela, uma de câmera e uma de áudio. |
| **Camada** | Versão codificada de uma trilha de vídeo (simulcast): `0` = qualidade escolhida por quem transmite, `1` = intermediária, `2` = prévia/miniatura. Áudio tem só a camada `0`. |
| **Assinatura** | Espectador × trilha, com o teto de camada que ele quer e o tamanho em que exibe. |
| **Ingresso** | Token curto (JWT ou opaco) que autoriza uma conexão QUIC de mídia numa sala. |

## Versões

- `protocolo`: inteiro, **4** nesta especificação. O app manda o dele em toda sessão; o servidor
  diz em `GET /v1/info` a faixa que aceita (`protocolo_min`..`protocolo_max`).
- Caminhos REST têm prefixo `/v1`. Campos novos podem aparecer em qualquer resposta: clientes e
  servidor **devem ignorar campos desconhecidos**. Remover ou mudar campo exige `protocolo` novo.
- Todo JSON é UTF-8. Nomes de campo em `snake_case`, em português.
- Tempos: instantes absolutos em ISO 8601 UTC (`2026-11-04T21:30:00Z`); durações em ms
  (campo termina com `_ms`), bitrates em kbps (`_kbps`).

## Segurança

- Tudo sobre TLS 1.3 (HTTPS, WSS, QUIC). Certificado válido no servidor (Let's Encrypt).
  No modo Local, certificado autoassinado por instalação; o app fixa a impressão digital SHA-256
  recebida na descoberta (`fingerprint`).
- **Senha do servidor** (opcional, configurada pelo dono do servidor): exigida em `POST /v1/sessoes`.
  Serve para barrar bots e quem não foi convidado.
- **Senha da sala**: enviada só em `POST /v1/salas/{codigo}/entrar` (dentro do TLS). O servidor
  guarda apenas hash (Argon2id ou bcrypt).
- **Token de sessão**: `Authorization: Bearer <token>` em toda chamada REST; no WebSocket, no
  parâmetro `?token=`. Vale 24 h; renovar com `POST /v1/sessoes/renovar`.
- Limites recomendados: 30 req/min por IP em `/sessoes` e `/entrar`; mensagens de chat até
  1000 caracteres e 5 por segundo por participante; nome até 32 caracteres.

## Fluxos principais

### Entrar numa sala Online

1. `GET /v1/info` → confere `protocolo_min/max`.
2. `POST /v1/sessoes` (nome, cor, senha do servidor) → `token`.
3. `GET /v1/ws?token=…` (WebSocket) → recebe `salas.lista` e atualizações.
4. `POST /v1/salas/{codigo}/entrar` → `ingresso`, estado da sala e endpoints de mídia.
5. WebSocket: `sala.entrar {sala_id}` → servidor envia `sala.estado` e passa a mandar eventos
   da sala.
6. Para cada trilha que quer ver: `assinar` (pelo WebSocket). O servidor responde com a rota
   (`midia.rota`: `ponte` ou `direta`) e a mídia passa a chegar pela conexão QUIC.

### Publicar (compartilhar tela, câmera, som)

1. WebSocket: `trilha.publicar` com o tipo, título e a escada de camadas → servidor responde
   `trilha.publicada` (com `trilha_id`) para todos da sala.
2. O app abre (ou reaproveita) a conexão QUIC com o servidor e manda as camadas **que o
   servidor pedir** (`camadas.pedidas`). Camada que ninguém assina não é codificada.
3. Para parar: `trilha.remover`.

### Ponte x direta

Com "Usar o servidor como ponte" ligado, o servidor tenta primeiro a rota direta: manda
`p2p.candidatos` com os endereços de cada lado; os dois apps tentam conectar por 3 s. Se der certo,
o espectador avisa `midia.direta_ok` e o servidor passa a só sinalizar. Se não, a mídia vai pela
ponte. Sem ponte, a sala só funciona com rota direta.

### Prévia na lista de salas

`POST /v1/salas/{codigo}/previa` devolve um ingresso de prévia: só camada 2, sem áudio, sem chat,
válido por 60 s (renovável). Apenas em salas `aberta`.

## Erros

REST: status HTTP + corpo `{"erro": "<codigo>", "mensagem": "texto para o usuário"}`.
WebSocket/QUIC: mensagem `erro` com os mesmos códigos.

| Código | HTTP | Quando |
|---|---|---|
| `protocolo_incompativel` | 426 | `protocolo` fora da faixa do servidor (a resposta traz `protocolo_min/max`) |
| `senha_servidor` | 401 | senha do servidor ausente ou errada |
| `conta_necessaria` | 401 | só no `POST /sessoes`: o servidor exige conta (`requer_conta`) e o pedido não trouxe `acesso_conta` válido (ausente, vencido, anônimo, de outro emissor ou com assinatura errada). App velho recebe a `mensagem` "Atualize o streamaZap para entrar com sua conta" |
| `plano_insuficiente` | 403 | ação acima do plano da conta. Traz `plano` (só o id, ex.: `"espectador"`). Ao transmitir (`trilha.publicar`/`trilha.atualizar`) traz também `campo`: `"tipo"` quando o plano não transmite nada (nem só o som) e `"camadas"` quando uma camada passa do teto. Em `POST /salas` e `POST /salas/{codigo}/entrar` (plano `nenhum`), sem `campo` |
| `nao_autenticado` | 401 | token ausente, inválido ou expirado |
| `sala_nao_encontrada` | 404 | código ou id inexistente |
| `senha_sala` | 403 | senha da sala errada |
| `sala_cheia` | 409 | sala atingiu o limite de participantes |
| `proibido` | 403 | ação só do dono (ex.: encerrar sala) |
| `limite` | 429 | limite de requisições/mensagens (traz `tentar_em_ms`) |
| `invalido` | 400 | corpo fora do formato (traz `campo`) |
| `nao_encontrado` | 404 | caminho REST inexistente |
| `interno` | 500 | erro no servidor |

No WebSocket e no QUIC, a mensagem `erro` traz os mesmos campos extras (`campo`, `tentar_em_ms`,
`protocolo_min`/`protocolo_max`, `plano`) e repete o `ref` do pedido.
