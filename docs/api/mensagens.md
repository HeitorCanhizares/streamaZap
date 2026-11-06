# Mensagens de controle

Formato único usado em dois lugares:
- **WebSocket** do servidor (`/v1/ws`): uma mensagem por frame de texto;
- **stream de controle QUIC** (modo Local, e também na conexão de mídia com o servidor):
  `u32` big-endian com o tamanho + JSON UTF-8. Tamanho máximo: 64 KiB.

Toda mensagem é um objeto com o campo `t` (tipo). Pedidos que esperam resposta podem levar `ref`
(string do cliente); a resposta ou o `erro` correspondente repete o `ref`.

Direção: **C→S** = app para servidor (ou para quem criou a sala, no modo Local);
**S→C** = o contrário.

### Canal de cada mensagem (modo Online)

No modo Online o app tem dois canais com o servidor: o WebSocket e o stream de controle da conexão
QUIC de mídia.
- **C→S**: o app pode mandar qualquer mensagem por qualquer um dos dois.
- **S→C, eventos de sala e chat**: pelo WebSocket quando ele está conectado; senão, pelo QUIC.
- **S→C, mensagens da mídia** (`camadas.pedidas`, `keyframe.pedir`, `camada.atual`, `relatorio`):
  **sempre** pelo QUIC.
- **Resposta direta a um pedido** (`pong`, `erro`, `trilha.publicada` com `local_id`,
  `chat.mensagem` com `cliente_id`): volta pelo canal do pedido, repetindo o `ref`.

## Tipos comuns

```ts
type Sala = {
  id: string; codigo?: string /* só Online */; nome: string; dono_id: string;
  acesso: "aberta" | "senha"; modo: "online" | "local"; criada_em: string;
}
type Participante = {
  id: string; nome: string; cor: string /* "#RRGGBB" */; papel: "dono" | "membro";
  entrou_em: string; versao_app: string; protocolo: number;
  latencia_ms?: number;          // RTT até o servidor/dono
  assistindo?: string | null;    // trilha_id principal que está vendo
}
type Camada = { camada: 0 | 1 | 2; largura: number; altura: number; fps: number; kbps: number }
type Trilha = {
  id: string; participante_id: string;
  numero: number;                // u16 único na sala; é o `trilha` dos cabeçalhos binários (midia.md)
  tipo: "tela" | "janela" | "camera" | "audio";
  titulo: string;                // "Tela 1", "cs2.exe", "Câmera integrada"
  camadas: Camada[];             // vídeo: 1 a 3 (camada 0 = escolhida por quem transmite); áudio: []
  audio?: { taxa: 48000; canais: 1 | 2; kbps: number };
  endereco?: Endereco;           // só modo Local: onde conectar para receber esta trilha
  camera?: CameraJeito;          // só tipo "camera"; ausente = canto inferior direito
}
// Como quem transmite quer a câmera para quem assiste: num canto da tela dele ou solta (célula própria).
type CameraJeito =
  | { modo: "canto"; canto: "sup_esq" | "sup_dir" | "inf_esq" | "inf_dir" }
  | { modo: "solta" }
type Endereco = { enderecos: string[] /* "26.12.34.56" */; porta: number; fingerprint: string /* SHA-256 hex do certificado */ }
```

## Conexão e sessão

| t | Direção | Campos | Notas |
|---|---|---|---|
| `ping` | ambos | `ts_ms` | Responder `pong` com o mesmo `ts_ms` |
| `pong` | ambos | `ts_ms` | RTT = agora − `ts_ms` |
| `erro` | S→C | `codigo`, `mensagem`, `ref?`, e os extras do REST (`campo`, `tentar_em_ms`, `protocolo_min`/`protocolo_max`) | Códigos em [`README.md`](README.md#erros). Repete o `ref` do pedido que falhou |

O servidor manda `ping` no WebSocket a cada 15 s (conexão morta depois de 45 s sem tráfego) e no
QUIC a cada 2 s. A `latencia_ms` do participante vem do `pong` do QUIC; a do WebSocket só conta
quando não há conexão QUIC.

### Aperto de mão QUIC (primeira troca no stream de controle)

| t | Direção | Campos |
|---|---|---|
| `ola` | C→S | `protocolo`, `versao_app`, `instalacao_id`, `nome`, `cor`, `ingresso?` (Online), `sala_id?` (Local), `previa?: bool` |
| `desafio` | S→C | `nonce` (base64, 32 bytes), `acesso` | Só no modo Local com senha |
| `autenticar` | C→S | `hmac` = base64(HMAC-SHA256(chave = senha UTF-8, msg = nonce)) |
| `bem_vindo` | S→C | `participante_id`, `sala: Sala`, `relogio_us` (relógio do servidor, só diagnóstico), `segredo_midia?` (só modo Local, ver abaixo) |
| `recusado` | S→C | `codigo`, `mensagem` | A conexão é fechada em seguida |

Tempo máximo do aperto de mão: 10 s.

**Modo Online (conexão com o servidor).**
- Um `ola` com ingresso de membro vale como `sala.entrar`. Logo depois do `bem_vindo`, o servidor
  manda `sala.estado` e o histórico do chat pela própria conexão QUIC, como o dono faz no modo
  Local. Assim o app pode funcionar só com QUIC, se quiser.
- Uma conexão de mídia por participante: uma conexão nova substitui a antiga.
- `recusado.codigo`:

| Código | Quando |
|---|---|
| `nao_autenticado` | ingresso inválido ou vencido |
| `protocolo_incompativel` | `protocolo` fora da faixa do servidor |
| `invalido` | a primeira mensagem não é `ola` |
| `proibido` | ingresso válido, mas o participante não está mais autorizado na sala: chamar `POST …/entrar` de novo |
| `sala_nao_encontrada` | a sala do ingresso não existe mais |

**Modo Local, mídia de outro participante.** Quem criou a sala gera um `segredo_midia` (32 bytes
aleatórios, base64) e o entrega em `bem_vindo` a cada membro. Para assistir a trilha de outro
membro, o app conecta direto no `endereco` da trilha e manda `ola` com
`sala_id` e `ingresso = base64(HMAC-SHA256(chave = segredo_midia, msg = participante_id))`. O
publicador confere com o mesmo segredo e responde `bem_vindo` (sem `segredo_midia`).

## Salas e participantes

| t | Direção | Campos | Notas |
|---|---|---|---|
| `salas.lista` | S→C | `salas: [...]` (formato de `GET /v1/salas`) | Só WebSocket; ao conectar e a cada mudança (máx. 1/s) |
| `sala.entrar` | C→S | `sala_id` | WebSocket, depois do `POST …/entrar` (quem criou a sala já está autorizado e não precisa do `entrar`). No QUIC, o `ola` com ingresso faz o mesmo papel |
| `sala.sair` | C→S | — | |
| `sala.estado` | S→C | `sala`, `participantes: Participante[]`, `trilhas: Trilha[]` | Estado completo; depois só eventos |
| `sala.saiu` | S→C | `sala_id` | Só para quem saiu (`POST …/sair` ou `sala.sair`). Clientes antigos ignoram |
| `sala.encerrada` | S→C | `motivo`: `"dono"` \| `"vazia"` (2 min sem ninguém) \| `"servidor"` (servidor desligando) | |
| `participante.entrou` | S→C | `participante` | |
| `participante.saiu` | S→C | `participante_id`, `motivo`: `"saiu"` \| `"caiu"` \| `"removido"` | `"caiu"` = 15 s presente na sala sem WebSocket nem QUIC |
| `participante.atualizou` | S→C | `participante` | Mudou nome, cor ou quem está assistindo; ou a latência variou ≥ 10 ms (no máx. 1 a cada 2 s por participante) |
| `perfil.atualizar` | C→S | `nome?`, `cor?` | |

As mensagens de sistema do chat ("Ju ligou a webcam", "Marcão criou a sala") são geradas pelo
app a partir desses eventos, não pelo servidor.

## Chat

| t | Direção | Campos |
|---|---|---|
| `chat.enviar` | C→S | `texto` (1–1000 caracteres), `cliente_id` (para confirmar o envio) |
| `chat.mensagem` | S→C | `id`, `participante_id`, `nome`, `cor`, `texto`, `em`, `cliente_id?` (só para quem enviou) |
| `chat.digitando` | ambos | C→S sem campos (no máx. 1 a cada 3 s); S→C `participante_id` |

O servidor guarda as últimas 200 mensagens e as manda logo depois de `sala.estado`.

## Trilhas (quem publica)

| t | Direção | Campos | Notas |
|---|---|---|---|
| `trilha.publicar` | C→S | `local_id`, `tipo`, `titulo`, `camadas`, `audio?`, `endereco?` (Local), `camera?` (só `camera`) | `camadas` = escada completa que o app sabe produzir |
| `trilha.publicada` | S→C | `trilha`, `local_id?` (só para quem publicou) | Para todos da sala |
| `trilha.atualizar` | C→S | `trilha_id`, `titulo?`, `camadas?`, `camera?` | Troca de qualidade ao vivo, janela redimensionada, câmera trocada de canto ou solta |
| `trilha.atualizada` | S→C | `trilha` | |
| `trilha.remover` | C→S | `trilha_id` | |
| `trilha.removida` | S→C | `trilha_id` | |
| `camadas.pedidas` | S→C | `trilha_id`, `camadas: [{camada, kbps}]` | **Quais camadas codificar e com que bitrate.** Camada fora da lista = parar de codificar. Ver [`balanceamento.md`](balanceamento.md) |
| `keyframe.pedir` | S→C | `trilha_id`, `camada` | Quem publica manda um keyframe dessa camada (máx. 1 por segundo por camada) |

Regras:
- Um participante tem no máximo uma trilha `tela` **ou** `janela`, uma `camera` e uma `audio`.
- `camadas[0]` é a qualidade escolhida por quem transmite; o servidor **nunca** pede bitrate acima
  do `kbps` anunciado em cada camada.
- `titulo` até 64 caracteres; `local_id` (e o `cliente_id` do chat) até 64.
- Ordem: quem publica recebe `trilha.publicada` (com `local_id`) **antes** do primeiro
  `camadas.pedidas` dessa trilha. Se ninguém estiver assistindo, esse primeiro vem com
  `camadas: []`: não codifique nada até o servidor pedir.
- `camadas.pedidas` cobre só quem assiste **pela ponte**. Espectadores na rota direta são alocados
  pelo próprio publicador ([`balanceamento.md`](balanceamento.md)).
- `camera` (em `trilha.publicar`, `trilha.atualizar` e na `Trilha`): só vale para trilha `camera`;
  em outro tipo, o servidor ignora. O servidor guarda o campo na trilha e o repassa em
  `trilha.publicada`, `trilha.atualizada` e `sala.estado`. Em `trilha.atualizar`, ausente = não
  muda. Trocar o jeito da câmera é só `trilha.atualizar` (a trilha não é republicada). App antigo
  sem o campo: todo mundo vê a câmera no canto inferior direito.
- **Plano (Online com conta):** `trilha.publicar` e `trilha.atualizar` são recusados com `erro`
  `plano_insuficiente` (com `ref`) quando o plano da sessão não transmite ou quando a camada 0 passa
  do limite do plano (`zap_proto::plan::check_publish`). Sessão sem conta não tem limite de plano.

## Assinaturas (quem assiste)

| t | Direção | Campos | Notas |
|---|---|---|---|
| `assinar` | C→S | `trilha_id`, `tamanho`: `"miniatura"` \| `"principal"` \| `"cheia"`, `camada_max?` (0–2, ausente = automático), `audio?: bool` | Mandar de novo para mudar tamanho/teto |
| `cancelar` | C→S | `trilha_id` | |
| `midia.rota` | S→C | `trilha_id`, `rota`: `"ponte"` \| `"direta"`, `endereco?` | Onde a mídia vai chegar |
| `camada.atual` | S→C | `trilha_id`, `camada`, `largura`, `altura`, `fps`, `motivo`: `"inicio"` \| `"banda"` \| `"teto"` \| `"tamanho"` \| `"publicador"` | Enviado quando a camada desse espectador muda |
| `keyframe.pedir` | C→S | `trilha_id` | Perdeu referência; o servidor repassa (ou responde com o próximo keyframe que já tiver) |

Tamanho → teto de camada: `miniatura` = 2, `principal` = 0, `cheia` = 0.

- **Áudio**: o espectador recebe o áudio de um publicador assinando a trilha `audio` dele **ou**
  mandando `audio: true` no `assinar` de uma trilha de vídeo do mesmo publicador. O áudio segue
  enquanto um dos dois valer (o app assina a trilha `audio` direto quando a pessoa compartilha só o
  som) e para quando o último sai (`cancelar` da trilha de áudio e dos vídeos que pediam áudio).
- **`camada.atual`** sai quando o keyframe da camada nova é repassado a esse espectador: `motivo`
  `"inicio"` na primeira vez, depois `"banda"`, `"teto"`, `"tamanho"` ou `"publicador"`.

## Rota direta (Online)

| t | Direção | Campos |
|---|---|---|
| `p2p.meus_enderecos` | C→S | `enderecos: string[]` ("ip:porta" locais, Radmin e o público visto pelo servidor), `fingerprint` |
| `p2p.candidatos` | S→C | `trilha_id`, `com`: participante_id, `papel`: `"cliente"` \| `"servidor"`, `enderecos`, `fingerprint` |
| `midia.direta_ok` | C→S | `trilha_id` |
| `midia.direta_falhou` | C→S | `trilha_id` |

O app com `papel: "cliente"` (quem assiste) tenta conectar em todos os `enderecos` ao mesmo tempo
por 3 s; o `"servidor"` (quem publica) manda um pacote QUIC para cada endereço do outro lado para
abrir o NAT. Se nenhum responder, `midia.direta_falhou` e segue pela ponte.

- `p2p.meus_enderecos` aceita até 16 endereços.
- O servidor manda `p2p.candidatos` no primeiro `assinar` da trilha, se os dois lados
  já mandaram `p2p.meus_enderecos`. Ele acrescenta à lista o endereço que viu na conexão QUIC de
  cada um.

## Modo Local: conexão reversa

| t | Direção | Campos |
|---|---|---|
| `conexao.reversa` | C→S | `trilha_id`, `endereco: "ip:porta"`, `token` |

Quem assiste não alcança quem publica (firewall do outro lado): pede, via quem criou a sala, para
o publicador conectar nele. A conexão QUIC aberta pelo publicador começa com `ola` contendo
`token`, e daí em diante os papéis de mídia são os mesmos.

## Música da sala

Fila de vídeos do YouTube que cada participante toca no próprio player (embed oficial), na
posição da sala. Ninguém retransmite som. Plano: [`../planos/musica-da-sala.md`](../planos/musica-da-sala.md).

```ts
type ItemMusica = {
  id: string; video_id: string /* 11 caracteres */; titulo: string; canal?: string;
  duracao_ms?: number; ao_vivo: boolean; por: string /* participante_id */; por_nome: string;
}
```

| t | Direção | Campos | Notas |
|---|---|---|---|
| `musica.adicionar` | C→S | `video_id`, `titulo?`, `canal?` | Fila com até 50 itens; com nada tocando, o item começa já |
| `musica.remover` | C→S | `item_id` | Só quem pôs o item ou o dono da sala; o item atual = pular direto |
| `musica.pular` | C→S | `item_id`, `voto: bool` | Voto no item atual (`false` desfaz). Pula com `max(1, ⌈presentes/2⌉)` votos |
| `musica.info` | C→S | `item_id`, `duracao_ms?`, `ao_vivo?`, `titulo?`, `terminou?`, `erro?` | O que o player descobriu. A maior duração vence; `titulo` só substitui o título padrão |
| `musica.estado` | S→C | `fila: ItemMusica[]`, `atual?: { item, posicao_ms, votos: string[], precisa }` | Para todos a cada mudança, para quem entra (depois do histórico do chat) e de novo a cada 10 s com música tocando |

Regras:
- `posicao_ms` vale no instante do envio (negativa = o item começa daqui a pouco; o item novo começa
  2 s no futuro para os players carregarem juntos). Quem recebe soma o tempo desde que recebeu.
- Fim: quem é a fonte avança quando a posição passa da duração (+2 s). `terminou` avança se a
  posição estiver a 5 s do fim (sem duração: depois de 10 s tocando); em live, sempre. `erro`
  pula o item atual na hora.
- Quem sai da sala perde o voto, e o limite é recalculado na hora (pode pular).
- Erros de pedido: `erro` com `codigo: "musica_invalida"` só para quem pediu.
- **Quem é a fonte**: no modo Local, quem criou a sala; no Online, o servidor. A regra é uma só,
  `zap_proto::music::MusicQueue` (pura; o tempo e o id do item vêm de quem chama), usada pelo
  dono no app e pelo servidor. Referência do lado do servidor: `crates/zap-test-server/src/music.rs`.
- **Online**: os pedidos vão pelo WebSocket, de quem está na sala (`sala.entrar` feito; senão
  `erro` `proibido`). O servidor manda `musica.estado` a todos depois do histórico do chat no
  `sala.entrar`, a cada mudança, quando alguém sai (o voto sai junto) e a cada 10 s com música
  tocando; o laço de 1 s avança a fila no fim do vídeo. O limite de votos conta os membros da sala.
  Servidor sem música: o app não recebe `musica.estado` e a aba avisa que o servidor precisa ser
  atualizado.
- Apps antigos ignoram `musica.*`.

## Estatísticas para a interface

| t | Direção | Campos |
|---|---|---|
| `estatisticas` | C→S | a cada 2 s: `ping_ms`, `trilhas: [{trilha_id, camada, kbps, fps, suavizacao_ms, perdidos}]` |

O servidor usa isso para atualizar `participante.atualizou` (latência, quem está assistindo) e
pode guardar para métricas.

## Descoberta local (UDP 47801, broadcast)

Datagramas JSON (até 1200 bytes) em todas as interfaces, a cada 1,5 s; uma sala some da lista
depois de 5 s sem anúncio.

```json
{ "app": "streamazap", "v": 4, "tipo": "sala",
  "sala": { "id": "s_…", "nome": "Corrida em Interlagos", "acesso": "aberta" },
  "dono": { "id": "…", "nome": "Rafa", "cor": "#5CD6FF" },
  "endereco": { "enderecos": ["26.9.77.140"], "porta": 47800, "fingerprint": "ab12…" },
  "participantes": 3, "destaque": { "altura": 1080, "fps": 60 }, "versao_app": "3.0.0" }
```

```json
{ "app": "streamazap", "v": 4, "tipo": "presenca",
  "instalacao_id": "…", "nome": "Heitor", "versao_app": "3.0.0",
  "ouve": ["<instalacao_id de quem eu recebo>", "…"] }
```

`ouve` permite mostrar "recebe você" / "não recebe nada do seu PC" no Diagnóstico.
Pacotes com `app` diferente ou `v` diferente de 4 são ignorados (mas `v` diferente é mostrado
como "outra versão").
