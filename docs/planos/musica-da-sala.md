# Música da sala: fila do YouTube tocada junto

Pedido do dono do projeto (2026-11-06): um player por sala com fila, que pula a música atual
quando metade da sala vota, e aceita live do YouTube.

## Ideia

Ninguém retransmite som. Cada pessoa toca o mesmo vídeo no próprio app, pelo player oficial do
YouTube (embed), e a sala mantém todos no mesmo ponto. Ninguém precisa de conta nem de Premium.

Termos do YouTube que valem aqui:
- o player fica **visível**, com pelo menos 200×200 px: não dá para esconder o vídeo e tocar só
  o áudio;
- **anúncios não podem ser bloqueados nem pulados por código**. Cada pessoa vê os próprios
  anúncios, em momentos e com durações diferentes; vídeo sem monetização não tem anúncio. Quem tem
  YouTube Premium logado não vê anúncio (dentro do WebView2 o login do Google costuma ser
  bloqueado, então não contamos com isso).

**A sala nunca espera ninguém.** A posição da sala anda sozinha; quem pegou anúncio (ou travou)
pula para a posição da sala quando o player volta a tocar. Perde aqueles segundos, mas todo mundo
continua junto.

## Escopo desta entrega

- Modo **Local** (o dono da sala é a autoridade da fila). No modo **Online** quem repassa as
  mensagens da sala é o servidor, que é outro repositório: as mensagens ficam documentadas em
  `docs/api/mensagens.md` para o servidor implementar depois. Até lá, a interface esconde a música
  nas salas Online.
- Fila de vídeos únicos (link de vídeo, live, shorts, `music.youtube.com`). Importar playlist fica
  para depois.
- Sem pausa da sala: cada um tem volume e mudo próprios no player.

## Regras

- Qualquer pessoa adiciona. Fila com até 50 itens.
- **Pular**: voto da pessoa no item atual (liga e desliga). Pula quando os votos chegam a
  `precisa = max(1, ⌈presentes / 2⌉)` (metade da sala, arredondando para cima). Quem sai da sala
  perde o voto e o limite é recalculado na hora.
- Quem adicionou o item atual e o dono da sala pulam direto (moderação). Itens da fila (ainda
  não tocando) saem pela mesma pessoa ou pelo dono.
- **Fim do vídeo**: o player de cada um conta a duração (`musica.info`). O dono avança quando a
  posição passa da duração (+2 s de folga). Um aviso `terminou` também avança, se a posição já
  estiver perto do fim (a 5 s) ou, sem duração conhecida, depois de 10 s tocando.
- **Live**: não tem duração; todos tocam no ponto ao vivo (sem buscar posição). Sai pelo voto,
  por quem adicionou ou pelo dono, ou quando a transmissão acaba (`terminou`).
- **Erro do player** (vídeo removido, privado ou que não deixa tocar fora do YouTube): o aviso
  `erro` pula o item atual na hora.
- O item novo começa 2 s no futuro (`posicao_ms` negativa), para os players carregarem juntos.

## Protocolo (`zap-proto`, `docs/api/mensagens.md`)

```ts
type ItemMusica = {
  id: string; video_id: string /* 11 caracteres */; titulo: string; canal?: string;
  duracao_ms?: number; ao_vivo: boolean; por: string /* participante_id */; por_nome: string;
}
```

| t | Direção | Campos |
|---|---|---|
| `musica.adicionar` | C→S | `video_id`, `titulo?`, `canal?` |
| `musica.remover` | C→S | `item_id` (item da fila; o atual = pular direto) |
| `musica.pular` | C→S | `item_id` (o atual), `voto: bool` |
| `musica.info` | C→S | `item_id`, `duracao_ms?`, `ao_vivo?`, `titulo?`, `terminou?`, `erro?` |
| `musica.estado` | S→C | `fila: ItemMusica[]`, `atual?: { item, posicao_ms, votos: string[], precisa }` |

`musica.estado` vai a todos a cada mudança e para quem entra (depois do histórico do chat).
`posicao_ms` vale no instante do envio; quem recebe soma o tempo desde que recebeu. Apps antigos
ignoram tipos desconhecidos (`zap_net::control`), então não quebram: só não veem a música.

## Núcleo

- `zap_net::youtube::oembed(video_id)`: título e canal pelo oEmbed público (sem chave). 401/403 =
  não deixa tocar fora do YouTube; 404 = não existe ou é privado. Erro de rede não impede adicionar
  (o título vem depois pelo player, em `musica.info`).
- `zap_core::music`: `video_id_do_link(texto)` (pura) e `MusicQueue`, a máquina de estados do dono
  (pura, recebe o `Instant`): `add`, `remove`, `vote`, `info`, `tick`, `left`, `msg`.
- `RoomModel` guarda `music` (do dono) e `music_mirror` (o último `musica.estado` e quando chegou,
  nos membros). `handle_member_msg` trata os pedidos (o dono passa pelo mesmo caminho);
  `apply_owner_msg` guarda o espelho; `on_member_left` tira o voto de quem saiu.
- O dono roda um laço de 1 s (`music.tick`) que avança a fila e avisa todos; com música tocando,
  reenvia o estado a cada 10 s (conserta um espelho que ficou velho por dois envios cruzados).
- `Core::music_add(link)`, `music_remove`, `music_vote`, `music_report`; `RoomView.music`
  (`null` = sala sem música: Online ou dono com app antigo).

## Interface

- Aba **Música** no painel da sala: colar link, o que está tocando (quem pôs, live, votos
  "Pular · 1/2"), a fila com remover (meus itens; tudo, se sou o dono).
- O player é mais uma transmissão no layout da sala (`kind: 'musica'`): vai para a principal,
  para a faixa ou fecha (fechar = não tocar para mim; reabrir volta no ponto da sala). Começa na
  principal se não houver vídeo de ninguém.
- O `<iframe>` nunca muda de lugar no DOM (mudar recarrega o vídeo e pode trazer outro anúncio):
  ele fica numa camada da coluna da sala e segue o retângulo da vaga (`VagaMusica`).
- Controle pelo `postMessage` do embed (`enablejsapi=1`), **sem** carregar o script do YouTube
  na página do app (o script teria acesso ao IPC do Tauri). A cada 1 s: posição da sala vs. a do
  player; diferença > 2 s com o player tocando = `seekTo`. Parado sem ser por fim = `playVideo`.
  Player tocando sem o tempo andar = anúncio ou carregando: mostra "Anúncio ou carregando · você
  volta junto da sala".
- CSP do Tauri: `frame-src https://www.youtube.com https://www.youtube-nocookie.com`.

## Testes de aceite

- `zap-proto`: ida e volta de cada mensagem nova.
- `MusicQueue`: começar, avançar no fim, live sem fim, votos com 1/2/3/4 pessoas, voto de quem
  sai, pular direto (quem pôs, dono), remover da fila (permissões), limite de 50, erro do player.
- Dois apps no loopback (dono e membro): o membro adiciona, os dois veem o mesmo `musica.estado`;
  o voto do membro com 2 pessoas pula.
- `video_id_do_link`: os formatos de link e lixo.
- Interface: aba Música (adicionar, votar, remover), o player segue a vaga, a sincronia busca a
  posição quando a diferença passa de 2 s e não busca em live.
