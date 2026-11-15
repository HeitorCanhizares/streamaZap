# Modo palco (v3.6.0)

Plano do arquiteto, 2026-11-14. Alvo: **v3.6.0**. Vale para o app (`HeitorCanhizares/streamaZap`)
e para o servidor (`streamaZapi`, repositório privado). A fonte do contrato é `docs/api/` do app: a
frente F1 (contrato) vai para o GitHub primeiro, e só depois a sessão do servidor sobe o `rev` dos
crates e implementa a seção 7.

Modelo de formato: `musica-da-sala.md` e `contas-planos-camera.md`.

## 0. Decisões

### Fechadas com o Heitor

| Assunto | Decisão |
|---|---|
| O que é | Um jeito de sala. O dono escolhe ao criar (Local e Online) e liga/desliga com a sala aberta |
| Anfitriões | O dono é sempre anfitrião. Só o dono promove e tira. Vários anfitriões transmitem ao mesmo tempo |
| Plateia | Quem não é anfitrião não transmite nada (tela, janela, som de app, câmera, microfone). O chat continua para todos |
| Pedir para subir | A plateia levanta a mão; o dono aceita (vira anfitrião) ou recusa |
| Microfone | Só existe no modo palco e só para anfitriões. Desligar o palco ou perder o papel fecha o microfone |
| Quem vira plateia | As transmissões dessa pessoa acabam na hora |
| Lançamento | v3.6.0 |

### Tomadas neste plano

O Heitor confirmou todas em 2026-11-14 (D1, D2, D3, D5, D6, D7 e D8 como estão aqui).
Cada uma tem a alternativa, para a troca ser barata.

| # | Assunto | Decisão | Por quê | |
|---|---|---|---|---|
| D1 | Microfone na rede | **Trilha própria**: `tipo: "audio"` com o campo novo `fonte: "microfone"` (Opus mono 48 kbps) | Quem assiste ajusta a voz separada do som do jogo e pode fechar a tela sem calar a voz; o servidor enxerga o microfone e o barra fora do palco; nenhum valor novo em enum existente (§3.5). Alternativa: misturar no áudio de apps (zero servidor, sem volume separado) | confirmada |
| D2 | App antigo (sem `recursos: ["palco"]`) | **Não entra** em sala em palco e **sai** quando o palco liga (`sala.encerrada {motivo: "atualizar_app"}`). O dono é avisado antes de ligar | Na v3.5 o app escolhe "o primeiro áudio" da pessoa e, sem vídeo, assina os dois áudios no mesmo receptor e embaralha o som (§3.5). Também mostraria botões de transmitir que sempre falham. Alternativa: deixar como plateia e o servidor esconder dele as trilhas de microfone (ele não ouve a voz de ninguém) | confirmada |
| D3 | Música no palco | A chave do Heitor ("plateia pode pôr música", qualquer anfitrião liga e desliga, começa **desligada**) e mais três regras: (a) com o palco ligado, **todo anfitrião modera** a música como o dono (tira qualquer item, pula direto); (b) com a chave desligada, o limite de votos conta **só os anfitriões presentes** e os votos da plateia somem; (c) `musica.info` (o player contando duração, fim e erro) continua liberado para todos | (a) sem isso, num palco com 10 na plateia e 2 anfitriões ninguém pula a música; (b) idem: com 12 presentes o limite seria 6 e os anfitriões nunca chegariam lá; (c) é telemetria do player, não controle: bloquear trava a fila quando o único anfitrião tem anúncio | confirmada |
| D4 | Protocolo | Fica no **4**. Tudo é aditivo | §3.5 | |
| D5 | Eco e ruído | **"Cancelar eco e ruído" ligado por padrão**, pelo Voice Capture DSP do Windows (eco + ruído + ganho, voz em 16 kHz). Desligado = captura WASAPI direta a 48 kHz. O DSP que não abre cai sozinho para a direta, com aviso | Funciona no Windows 10 e 11, sem dependência nova. A voz em 16 kHz é o preço; quem usa fone desliga | confirmada (padrão ligado) |
| D6 | Papel de anfitrião | Continua guardado com o palco desligado (religar devolve os mesmos anfitriões). Quem era anfitrião e volta à sala em até **60 s** continua anfitrião | Pausar o palco não obriga o dono a promover todo mundo de novo; uma queda curta do Radmin não derruba o microfone de vez | confirmada |
| D7 | Mãos | Todos veem quem levantou a mão; só o dono responde | Uma fonte de estado só (`palco.estado`), sem mensagem por destinatário | confirmada |
| D8 | Plano do microfone | Mesma regra do som de apps: precisa de plano de Transmissor quando a cobrança ligar (hoje todo mundo tem `fullhd`) | Zero mudança em `zap_proto::plan`. Alternativa: liberar o microfone para o Espectador (voz custa 48 kbps) | confirmada |
| D9 | Mensagens | 4 C→S (`palco.ligar`, `palco.anfitriao`, `palco.mao`, `palco.musica`) e 1 S→C (`palco.estado`). Aceitar = `palco.anfitriao` (promover); recusar = o dono baixa a mão da pessoa (`palco.mao` com `participante_id`) | Menos mensagens, menos testes no servidor | |
| D10 | Saber se o outro lado tem palco | Campo `recursos: string[]` no `ola`, no `POST /v1/sessoes`, no `Participante` e no `GET /v1/info` (hoje só `"palco"`) | Comparar `versao_app` é frágil (os testes usam `"0.1.0"`, builds de dev). Subir o protocolo quebraria a sala Local entre 3.5 e 3.6 (o aperto de mão recusa protocolo diferente) | |

## 1. Escopo

Entra na v3.6.0:
- modo palco nas salas Local e Online: criar com palco, ligar e desligar na sala;
- promover e tirar anfitrião (só o dono); levantar e baixar a mão; aceitar e recusar;
- servidor e dono do Local recusam e derrubam as transmissões da plateia;
- microfone para anfitriões no palco: trilha própria, escolha do dispositivo, mudo (botão e tecla
  **V**), medidor de nível, "Cancelar eco e ruído", aviso de "sem fone";
- quem assiste: volume da voz de cada anfitrião separado do som da transmissão; indicador de
  "falando";
- chave "plateia pode pôr música";
- selo **PALCO** na lista de salas (Local e Online) e no cabeçalho da sala;
- app antigo: recusado ao entrar e removido quando o palco liga;
- a correção que o palco precisa e o app não tem: quando o outro lado remove a **minha** trilha, o
  app para de capturar e transmitir aquela parte (§5.3).

Fica fora: ver §10.

## 2. Regras

### 2.1 Papéis

| Papel | Quem | Pode |
|---|---|---|
| dono | quem criou a sala (`sala.dono_id`) | tudo do anfitrião; ligar/desligar o palco; promover e tirar; recusar pedidos |
| anfitrião | o dono e quem ele promoveu | transmitir tela/janela, som de apps, câmera e microfone; ligar/desligar a chave da música; moderar a música |
| plateia | o resto, com o palco ligado | assistir, chat, levantar a mão; música só com a chave ligada |

Com o palco **desligado**, todo mundo transmite como hoje e **ninguém tem microfone**. A lista de
anfitriões continua guardada (D6).

### 2.2 Ligar e desligar (`palco.ligar`, só o dono)

**Ligar**, nesta ordem:
1. Quem não tem `"palco"` em `recursos` (app antigo) sai da sala: recebe só ele
   `sala.encerrada {motivo: "atualizar_app"}`, e os outros veem a saída normal (trilhas dele somem,
   `participante.saiu {motivo: "removido"}`). No Online ele perde a autorização da sala.
2. Toda trilha de quem não é anfitrião sai (`trilha.removida` para todos).
3. Com a chave da música desligada, os votos da plateia somem (§2.6).
4. `palco.estado` para todos.

**Desligar**: todas as trilhas de microfone saem; as mãos baixam; `palco.estado` para todos. Nada
mais muda (a plateia pode voltar a transmitir).

Ligar já ligado ou desligar já desligado: nada acontece, nada é enviado, sem erro.

### 2.3 Promover e tirar (`palco.anfitriao`, só o dono)

- Promover: entra no fim da lista de anfitriões; a mão dela baixa; o prazo de "pedir de novo" dela
  zera. Já anfitrião: nada.
- Tirar com o palco ligado: todas as trilhas dela saem (inclusive o microfone). Com o palco
  desligado: só sai da lista. Já plateia: nada.
- O dono não pode ser tirado (`invalido`); promover o dono não faz nada.
- Alvo fora da sala: `invalido`.
- Vale com o palco desligado (prepara a lista para quando ligar). A interface só mostra os botões
  com o palco ligado.

### 2.4 Mãos (`palco.mao`)

- Só com o palco ligado (senão `invalido`). Anfitrião levantando: `invalido` ("Você já está no
  palco"); anfitrião baixando: nada.
- Ordem de chegada; cada pessoa aparece uma vez.
- Levantar de novo exige **5 s** desde a última vez que levantou (`limite`, `tentar_em_ms` = o que
  falta).
- Recusar = o dono manda `palco.mao {levantada: false, participante_id}`. A pessoa só levanta de
  novo **30 s** depois da recusa (`limite`). Recusar quem não está com a mão levantada: nada.
- Desligar o palco baixa todas as mãos e zera os dois prazos.

### 2.5 Transmitir

Matriz de `trilha.publicar` (a mesma no dono do Local, no servidor e na conferência do app):

| Trilha | palco desligado | palco ligado, anfitrião | palco ligado, plateia |
|---|---|---|---|
| tela, janela, câmera, som de apps | ok | ok | `proibido` "No modo palco, só os anfitriões transmitem" |
| microfone | `proibido` "O microfone só existe no modo palco, para os anfitriões" | ok | `proibido` "No modo palco, só os anfitriões transmitem" |

- Por participante: no máximo uma tela **ou** janela, uma câmera, um som de apps e um microfone.
- `trilha.atualizar` e `trilha.remover` da própria trilha continuam valendo para todos (uma trilha
  que existe já passou pela regra).
- Ordem das conferências no servidor: formato → presença na sala → **palco** → plano → conflito de
  tipo → limite de vídeos da sala.

### 2.6 Música

| Ação | palco desligado | anfitrião | plateia, chave ligada | plateia, chave desligada |
|---|---|---|---|---|
| `musica.adicionar`, `musica.pular`, `musica.pausar`, `musica.buscar`, `musica.remover` | regra de hoje | ok | regra de hoje | `proibido` "No palco, a música fica com os anfitriões" |
| `musica.info` | ok | ok | ok | ok |

- Moderação: com o palco ligado, todo anfitrião tira qualquer item e pula direto (o `eh_dono` de
  `MusicQueue::remove` vira "dono ou anfitrião").
- Quem vota: com o palco desligado ou a chave ligada, todos os presentes; com a chave desligada, só
  os anfitriões presentes. O `presentes` passado a `MusicQueue::{vote, left, msg}` é essa contagem
  (`precisa = max(1, ⌈votantes/2⌉)`).
- Quando a chave desliga, um anfitrião vira plateia ou o palco liga com a chave desligada: para cada
  presente que não vota mais, `MusicQueue::left(id, votantes, agora)` (tira o voto e recalcula; pode
  pular). Depois, `musica.estado` para todos.
- `palco.musica` só com o palco ligado (senão `invalido`), por qualquer anfitrião (senão
  `proibido`). A chave fica guardada com o palco desligado.

### 2.7 Saída, queda e volta

- Sair da sala (`saiu`, `caiu`, `removido`) tira a mão e o papel. O anfitrião que sai fica 60 s
  numa lista de "voltou logo".
- Volta em até 60 s: anfitrião de novo, antes do `sala.estado` dele (a republicação do que ele
  transmitia passa). Depois de 60 s: plateia.
- "A mesma pessoa": no Online, o mesmo `participante_id` (o servidor já reconhece a instalação); no
  Local, a mesma `instalacao_id` do `ola` (o id de participante muda a cada conexão).
- O dono que cai no Online continua dono (a sala só acaba com `DELETE`); ninguém promove enquanto
  ele está fora.

### 2.8 Ordem das mensagens de uma mudança

Quem decide (dono do Local ou servidor) manda, nesta ordem:
1. para cada app antigo removido: `sala.encerrada {motivo: "atualizar_app"}` só para ele, depois a
   saída dele como em qualquer saída (`trilha.removida` de cada trilha, `participante.saiu`,
   `musica.estado`);
2. `trilha.removida` de cada trilha de quem virou plateia;
3. `trilha.removida` de cada microfone (palco desligou);
4. `musica.estado`, se os votantes mudaram;
5. `palco.estado` para todos.

Erro: só `erro` para quem pediu, com o `ref` do pedido. Sucesso não tem resposta própria: o
resultado é o `palco.estado`.

**Quem entra** recebe: `bem_vindo` (QUIC) → `sala.estado` → `palco.estado` → histórico do chat →
`musica.estado`. Uma fonte que tem palco manda `palco.estado` **sempre**, mesmo com o palco
desligado: é assim que o app sabe que pode mostrar os controles.

### 2.9 Números

| Nome (`zap_proto::stage`) | Valor | Uso |
|---|---|---|
| `HAND_INTERVAL` | 5 s | levantar de novo depois de levantar |
| `HAND_AFTER_DECLINE` | 30 s | pedir de novo depois de recusado |
| `HOST_GRACE` | 60 s | anfitrião que volta nesse prazo continua anfitrião |
| `FEATURE` | `"palco"` | valor em `recursos` |
| limite de `recursos` | 16 itens de até 32 caracteres | o resto é ignorado |
| `MIC_KBPS` (`zap-media`) | 48 | Opus mono, `Voip`, FEC em banda, perda esperada 10% |
| nível do meu microfone | pico dos últimos 250 ms, 0..1 | `MicView.level` |
| falando | pico > 0,03 (≈ −30 dBFS) nos últimos 300 ms | `Participant.speaking` |

## 3. Contrato

Textos prontos para a frente F1 colar em `docs/api/`. Nada aqui muda campo existente.

### 3.1 `docs/api/mensagens.md`

**Tipos comuns** (acrescentar a `Participante` e `Trilha`):

```ts
type Participante = {
  // ...campos de hoje...
  recursos?: string[];           // o que o app sabe além do protocolo 4 base (hoje só "palco"); ausente = []
}
type Trilha = {
  // ...campos de hoje...
  fonte?: "apps" | "microfone";  // só tipo "audio". Ausente = "apps" (som de apps). "microfone" só no modo palco
  mudo?: boolean;                // só microfone; ausente = false
}
```

- `fonte` desconhecido (de uma versão futura) vale como ausente (`zap_proto::types::tolerant`); a
  mensagem continua válida. Quem decide guarda `fonte` só quando é `microfone` (`apps` vira
  ausente).
- `recursos`: até 16 itens de até 32 caracteres; o excesso é ignorado.

**Aperto de mão**: `ola` ganha `recursos?: string[]`.

| t | Direção | Campos |
|---|---|---|
| `ola` | C→S | `protocolo`, `versao_app`, `instalacao_id`, `nome`, `cor`, `ingresso?` (Online), `sala_id?` (Local), `previa?: bool`, `recursos?: string[]` |

Modo Local, `recusado.codigo` (tabela nova):

| Código | Quando |
|---|---|
| `versao` | `protocolo` diferente do do dono |
| `senha_sala` | senha errada |
| `ingresso` | ingresso de mídia inválido |
| `atualizar_app` | a sala está no modo palco e o `ola` não traz `"palco"` em `recursos` |

No Online o servidor usa os `recursos` da sessão (`POST /v1/sessoes`); os do `ola` são ignorados.

**Salas e participantes**: `sala.encerrada.motivo` ganha `"atualizar_app"` (só para quem tem app
sem o recurso `palco`, quando o palco liga; os outros veem `participante.saiu {motivo: "removido"}`).

**Trilhas (quem publica)**, linhas novas da tabela:

| t | Direção | Campos | Notas |
|---|---|---|---|
| `trilha.publicar` | C→S | `local_id`, `tipo`, `titulo`, `camadas`, `audio?`, `endereco?` (Local), `camera?` (só `camera`), `fonte?` (só `audio`) | `fonte: "microfone"` só no modo palco, só anfitrião |
| `trilha.atualizar` | C→S | `trilha_id`, `titulo?`, `camadas?`, `camera?`, `mudo?` (só microfone) | `mudo` ausente = não muda; em outra trilha, descartado. `mudo` sozinho passa com qualquer plano |

Regras (substitui o primeiro item e acrescenta dois):
- Um participante tem no máximo uma trilha `tela` **ou** `janela`, uma `camera`, uma `audio` de
  apps e uma `audio` com `fonte: "microfone"`.
- Microfone: `audio: { taxa: 48000, canais: 1, kbps: 48 }`, título `"Microfone"`. Com `mudo: true`
  não chega pacote (a trilha continua).
- **`audio: true` num `assinar` de vídeo traz só o som de apps** do mesmo publicador. O microfone
  só chega a quem assina a trilha dele.
- Modo palco: ver "Modo palco" abaixo (quem pode publicar).

**Seção nova "Modo palco"** (depois de "Música da sala"):

> Só os anfitriões transmitem; a plateia assiste e conversa no chat. O dono é sempre anfitrião e é
> o único que liga o palco, promove, tira e recusa pedidos. Plano:
> [`../planos/modo-palco.md`](../planos/modo-palco.md).

```ts
type PalcoEstado = {
  ligado: boolean;
  anfitrioes: string[];      // participante_id; o dono primeiro, depois na ordem em que subiram
  maos: string[];            // pedidos para subir, na ordem de chegada
  plateia_musica: boolean;   // a plateia pode mexer na música (adicionar, votar, pausar, buscar, remover os seus)
}
```

| t | Direção | Campos | Notas |
|---|---|---|---|
| `palco.ligar` | C→S | `ligado: bool` | Só o dono. Ligar: app sem o recurso `palco` sai da sala; as trilhas de quem não é anfitrião saem. Desligar: os microfones saem e as mãos baixam |
| `palco.anfitriao` | C→S | `participante_id`, `anfitriao: bool` | Só o dono. `true` promove (aceita o pedido, se houver: a mão baixa). `false` tira; com o palco ligado, todas as trilhas da pessoa saem. Tirar o dono ou alvo fora da sala: `invalido` |
| `palco.mao` | C→S | `levantada: bool`, `participante_id?` | Sem `participante_id` (ou com o próprio): a própria mão, só plateia e com o palco ligado. Com o de outra pessoa e `levantada: false`: o dono recusa o pedido. Levantar de novo: 5 s depois da última vez, 30 s depois de recusado (`limite` com `tentar_em_ms`) |
| `palco.musica` | C→S | `plateia: bool` | Qualquer anfitrião, com o palco ligado. Desligar tira os votos da plateia |
| `palco.estado` | S→C | `ligado`, `anfitrioes`, `maos`, `plateia_musica` | Para todos a cada mudança e para quem entra (logo depois de `sala.estado`). Quem tem palco manda sempre, até com o palco desligado |

Regras:
- **Transmitir**: com o palco ligado, `trilha.publicar` de quem não é anfitrião é recusado com
  `erro` `proibido` (`campo: "palco"`). Microfone (`fonte: "microfone"`) só com o palco ligado e
  só anfitrião, também `proibido`.
- **Música**: com o palco ligado e `plateia_musica: false`, a plateia recebe `proibido` em
  `musica.adicionar`, `musica.remover`, `musica.pular`, `musica.pausar` e `musica.buscar`;
  `musica.info` continua para todos. Com o palco ligado, todo anfitrião tira qualquer item e pula
  direto. O limite de votos conta só quem pode votar (`plateia_musica: false` → só os anfitriões
  presentes).
- **Ordem** de uma mudança: saídas de apps antigos, `trilha.removida` de quem virou plateia e dos
  microfones, `musica.estado` (se os votantes mudaram), `palco.estado`.
- **Erros** (só para quem pediu, com o `ref`):

| Código | `campo` | Quando |
|---|---|---|
| `proibido` | `palco` | não é o dono (`palco.ligar`, `palco.anfitriao`, recusar); não é anfitrião (`palco.musica`); plateia transmitindo; microfone fora do palco; plateia mexendo na música trancada |
| `invalido` | `palco` | palco desligado (`palco.mao`, `palco.musica`); anfitrião levantando a mão; levantar a mão de outra pessoa |
| `invalido` | `participante_id` | tirar o dono; alvo fora da sala |
| `limite` | `palco` | levantar a mão cedo demais (`tentar_em_ms`) |

- **Saída e volta**: quem sai perde o papel e a mão. Anfitrião que volta em até 60 s (Online: o
  mesmo participante; Local: a mesma instalação) continua anfitrião.
- **App antigo** (sem `"palco"` em `recursos`): não entra em sala em palco (Online:
  `426 atualizar_app` no `POST …/entrar`; Local: `recusado {codigo: "atualizar_app"}`) e sai quando
  o palco liga. Mensagens `palco.*` são desconhecidas para ele e puladas.
- O `PROTOCOL_VERSION` não muda.

**Música da sala**, regra nova no fim da lista: "Modo palco: ver a seção "Modo palco" (quem pode
mexer, quem modera e quem vota)."

**Descoberta local**: o anúncio `sala` ganha `"palco": true` quando o palco está ligado (ausente =
desligado):

```json
"sala": { "id": "s_…", "nome": "Corrida em Interlagos", "acesso": "aberta", "palco": true }
```

### 3.2 `docs/api/rest.md`

- `GET /v1/info` ganha `"recursos": ["palco"]` (ausente = nenhum). "O app só oferece o modo palco
  em sala Online se `recursos` tiver `"palco"`."
- `POST /v1/sessoes`, pedido ganha `"recursos": ["palco"]` (o que o app sabe fazer; até 16 itens de
  até 32 caracteres). O servidor repassa em `Participante.recursos`.
- `GET /v1/salas` (e `salas.lista`): `"palco": true` na sala em palco (ausente = desligado). O
  `POST /v1/salas` e o `GET /v1/salas/{codigo}` usam o mesmo formato.
- `POST /v1/salas`: corpo ganha `"palco": true` (opcional, padrão `false`): a sala nasce com o
  palco ligado. Sessão sem `"palco"` em `recursos` pedindo `palco: true`: `400 invalido`
  (`campo: "palco"`).
- `POST /v1/salas/{codigo}/entrar`: sala com o palco ligado e sessão sem `"palco"` em `recursos`:
  `426 atualizar_app`, com a `mensagem` "Esta sala está no modo palco. Atualize o streamaZap
  (versão 3.6.0 ou mais nova) para entrar." Quem já está autorizado também é conferido.

### 3.3 `docs/api/README.md`, erros

Linha nova e duas atualizadas:

| Código | HTTP | Quando |
|---|---|---|
| `atualizar_app` | 426 | a sala está no modo palco e o app não tem o recurso `palco` (`POST …/entrar`; no Local, `recusado`). A `mensagem` pede para atualizar |
| `proibido` | 403 | ação só do dono (ex.: encerrar sala, ligar o palco) ou dos anfitriões; no modo palco, a plateia transmitindo ou mexendo na música trancada (traz `campo: "palco"`) |
| `limite` | 429 | limite de requisições/mensagens, ou levantar a mão cedo demais no palco (traz `tentar_em_ms`) |

### 3.4 Tipos em `crates/zap-proto`

Ver F1 (§6). Resumo: `AudioSource`, `Participant.features`, `Track.source`/`muted`, `Hello.features`,
`TrackPublish.source`, `TrackUpdate.muted`, cinco variantes `ControlMsg::Stage*`,
`RoomSummary.stage`, `version::FEATURES` e o módulo puro `zap_proto::stage` (regras usadas pelo
dono do Local, pelo servidor e pela conferência do app, como `zap_proto::music`).

### 3.5 Compatibilidade com a v3.5.x (protocolo 4)

O que o código da v3.5 faz com cada tipo de novidade:

| Novidade | Onde lê | O que acontece |
|---|---|---|
| Campo desconhecido numa mensagem conhecida | `serde` sem `deny_unknown_fields` (`zap-proto`; teste `campos_desconhecidos_ignorados` em `types.rs`); no servidor, `NewRoom`/`NewSession` também não negam | Ignorado. Seguro |
| `t` desconhecido, QUIC | `ControlDecoder::next` (`zap-proto/src/framing.rs`) vira `UnknownMessage`; `ControlRx::recv` (`zap-net/src/control.rs`) loga e pula | Pulado. Seguro |
| `t` desconhecido, WebSocket | `zap-net/src/online/ws.rs` manda `ApiError::Protocol`; `session_run.rs` loga em debug e segue | Pulado. Seguro |
| **Valor novo num enum existente** (ex.: `TrackKind` `"microfone"`, `Role` `"anfitriao"`) dentro de `sala.estado` | O erro do serde contém "unknown variant", e o `ControlDecoder` classifica a **mensagem inteira** como desconhecida (o `t` dela, `sala.estado`); no WebSocket a mensagem inteira cai | **O `sala.estado` inteiro é descartado**: o app antigo fica sem participantes nem trilhas. Também perde `trilha.publicada` e `participante.entrou/atualizou`. **Proibido** |
| Tipo errado num campo existente | QUIC: `ProtoError::Json`, e `member_run::recebe` trata como queda e reconecta (em laço, porque o `sala.estado` seguinte vem igual) | **Proibido** |

Decisão: **fica no protocolo 4**. Só entram campos opcionais com `default`, mensagens `t` novas, e
um campo de texto novo (`fonte`) lido de forma tolerante. `Role`, `TrackKind`, `Access` e `RoomMode`
não ganham valores. O microfone é `tipo: "audio"` com `fonte`, e o anfitrião vai em
`palco.estado.anfitrioes`, não no `papel`.

Por que o app antigo não pode ficar numa sala em palco (D2): na v3.5, `Core::watch` de um vídeo
pega **a primeira** trilha `audio` da pessoa (`engine_watch.rs`), e o `AudioRegistry` tem **um
receptor por participante** (`subscribe/audio_rx.rs`). Com som de apps e microfone:
- a pessoa com vídeo: o app antigo ouve só um dos dois (o publicado primeiro);
- a pessoa sem vídeo: as duas trilhas viram "só o som", as duas assinaturas usam o mesmo
  `AudioRx`, e as sequências dos dois fluxos se misturam no mesmo decodificador Opus. O som sai
  embaralhado.

Como o microfone só existe no palco e o app antigo não fica em sala em palco, **ele nunca vê duas
trilhas de áudio da mesma pessoa**.

O que um app antigo vê:
- na lista: a sala aparece sem selo (`palco` ignorado);
- ao entrar numa sala em palco: Online, a `mensagem` do servidor ("Esta sala está no modo palco.
  Atualize…"); Local, "Recusado pela sala: atualizar_app";
- numa sala que vira palco: sai com "Sala encerrada (atualizar_app)"; fora da sala, o atualizador
  da v3.3 pode instalar a 3.6;
- numa sala sem palco com dono ou servidor novos: nada muda (o `palco.estado {ligado: false}` é
  pulado com um aviso no log).

Os outros sentidos:
- **App 3.6 com servidor antigo**: `GET /v1/info` sem `recursos` → a opção de palco aparece
  desligada ("O servidor ainda não tem o modo palco"); na sala, sem `palco.estado`, os controles do
  palco não aparecem. `recursos` no `POST /sessoes` é ignorado.
- **App 3.6 como membro de dono 3.5 (Local)**: o dono antigo ignora `recursos` do `ola` e nunca
  manda `palco.estado`; nada do palco aparece.

## 4. Microfone

### 4.1 Na rede: trilha própria ou misturar (D1)

| Critério | Misturar no som de apps | Trilha própria (`fonte: "microfone"`) |
|---|---|---|
| Compatibilidade | Perfeita: o app antigo ouve tudo junto | Perfeita para quem pode estar na sala (o app antigo não fica em sala em palco, §3.5) |
| Volume para quem assiste | Um controle só por pessoa: jogo alto abafa a voz e ninguém conserta | Voz e som da transmissão com controles separados; fechar a tela não cala a voz |
| Microfone fora do palco | Só o app impede (o servidor não enxerga a voz dentro do áudio) | O servidor recusa `fonte: microfone` fora do palco e derruba quando o palco desliga |
| "Falando" | Não dá para separar a voz | O receptor mede só o microfone |
| Trabalho no servidor | Nenhum além das regras do palco | Pequeno: conflito por `(tipo, fonte)`, a regra do palco, `mudo` repassado, `audio: true` sem o microfone |
| Trabalho no app | Mistura no `AudioPipeline` | Publicador próprio, receptor por fonte (§4.5), `ajustar_audio` do Local por trilha |

Escolha: **trilha própria**. O custo extra é de um dia de servidor e a mudança no receptor; o ganho
(voz com volume próprio, servidor barrando o microfone) é o que faz o palco funcionar como um
programa ao vivo entre amigos.

### 4.2 Captura, eco e ruído no Windows

O que existe:

| Opção | Windows 10 | Eco | Ruído | Custo |
|---|---|---|---|---|
| WASAPI com `AudioCategory_Communications` | sim | só se o driver tiver o efeito de comunicação (varia por fabricante) | idem | zero, mas não garante nada |
| **Voice Capture DSP** (`CWMAudioAEC`, um DMO, `Mfwmaaec.dll`) | sim (desde o Vista) | sim, com referência na saída padrão | sim (`FEATR_NS`), mais ganho automático (`FEATR_AGC`) | saída só em 8/11,025/16/22,05 kHz; em modo fonte ele mesmo abre o microfone e a saída |
| `IAcousticEchoCancellationControl` | não (Windows 11 build 22621+) | escolhe a referência do eco do driver | — | só ajuda onde o driver já tem eco |
| WebRTC APM / Speex / RNNoise | sim | WebRTC sim | sim | dependência nativa (meson, abseil) ou só ruído; fica para depois |

Mínimo da v3.6.0 (D5):
- **"Cancelar eco e ruído" ligado (padrão)**: Voice Capture DSP em **modo fonte**:
  - `MFPKEY_WMAAECMA_SYSTEM_MODE = 0` (`SINGLE_CHANNEL_AEC`);
  - `FEATURE_MODE = true`, `FEATR_NS = 1`, `FEATR_AGC = true`, `FEATR_ECHO_LENGTH = 256` (eco de
    até 256 ms), todos postos explicitamente (não depender do padrão);
  - `DEVICE_INDEXES`: microfone = índice do escolhido na coleção
    `EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)` (−1 = padrão); saída = −1 (a padrão, que é
    onde o `AudioOut` do app toca);
  - saída 16 kHz mono PCM 16 bits, lida por `ProcessOutput` a cada 10 ms, juntada em blocos de
    20 ms e reamostrada para 48 kHz (×3, §4.3).
- **Desligado**: WASAPI compartilhado, evento, `AUTOCONVERTPCM` para 48 kHz float mono, blocos de
  20 ms. Sem processamento.
- **Reserva**: o DSP que falha ao abrir (formato que ele recusa, microfone em matriz, erro do COM)
  faz a captura cair na direta, com o aviso "Cancelamento de eco indisponível neste microfone: usando
  o som direto" e `MicView.processing = false`.
- `WMAAECMA_E_NO_ACTIVE_RENDER_STREAM` (nada tocando na saída): bloco de silêncio e segue. O
  `AudioOut` do app fica aberto tocando silêncio, então isso só acontece sem saída de áudio.
- As chaves `MFPKEY_WMAAECMA_*` **não estão no `windows` 0.62** (só `CWMAudioAEC`, `AEC_SYSTEM_MODE`,
  `IMediaObject` e `MoInitMediaType`): definir as `PROPERTYKEY` em `zap-audio/src/mic_dsp_keys.rs`,
  copiando os GUIDs de `wmcodecdsp.h` do Windows SDK instalado.

Referências (Microsoft): [Voice Capture DSP](https://learn.microsoft.com/en-us/windows/win32/medfound/voicecapturedmo),
[MFPKEY_WMAAECMA_SYSTEM_MODE](https://learn.microsoft.com/en-us/windows/win32/medfound/mfpkey-wmaaecma-system-modeproperty),
[MFPKEY_WMAAECMA_DEVICE_INDEXES](https://learn.microsoft.com/en-us/windows/win32/medfound/mfpkey-wmaaecma-device-indexesproperty),
[IAcousticEchoCancellationControl](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nn-audioclient-iacousticechocancellationcontrol)
(o exemplo da Microsoft pede Windows 11 build 22540+; a referência da API, 22621).

Avisos para quem fala:
- **Sem fone**: a forma da saída padrão (`PKEY_AudioEndpoint_FormFactor`: `Headphones`/`Headset`
  = fone; `Speakers` = caixa) vai em `MicView.headphones`. Com o cancelamento desligado e saída de
  caixa: "Sem fone e sem cancelamento de eco: os outros vão ouvir eco".
- **Microfone bloqueado** (`E_ACCESSDENIED`, privacidade do Windows): "O Windows bloqueou o
  microfone. Libere em Configurações → Privacidade → Microfone → Permitir que aplicativos da área de
  trabalho acessem o microfone."

### 4.3 Codificação

- Bloco: 20 ms, mono 48 kHz (960 amostras), `capture_us = zap_audio::clock::now_us()` no fim do
  bloco (a mesma regra do `AppCapture`).
- Reamostragem 16→48 kHz: FIR polifásico ×3 próprio (`zap_audio::resample3`, puro): passa-baixa
  sinc com janela de Kaiser (β = 8), corte em 7,6 kHz, 96 coeficientes (32 por fase), atraso de 1 ms.
  Motivo de não usar o `rubato` (que já existe): razão inteira, sem bloco fixo, e teste exato no
  Linux.
- Opus: `OpusVoiceEnc`, mono, `Application::Voip`, 48 kbps, complexidade 8, FEC em banda, perda
  esperada 10%, sem DTX (a sequência fica contínua). Quem recebe usa o `OpusDec` estéreo de hoje (o
  Opus decodifica mono para os dois canais).
- Mudo: a captura continua (o eco segue convergido e o medidor anda); o codificador não manda nada;
  `seq` não avança no mudo (o primeiro pacote depois do mudo é `último + 1`).

### 4.4 O que entra para quem fala

- **Dispositivo**: lista em `Sources.microphones` (`mic:<id do endpoint>`, nome amigável) mais
  "Padrão do Windows (comunicação)" (`None`). Guardado em `settings.sound.mic_id`. Trocar com o
  microfone ligado reabre a captura na mesma trilha (sem republicar).
- **Cancelar eco e ruído**: `settings.sound.mic_processing` (padrão `true`). Trocar ao vivo reabre a
  captura.
- **Mudo**: botão do dock e tecla **V** (a M já silencia a pessoa na tela). Manda
  `trilha.atualizar {mudo}`.
- **Desligar o microfone**: no menu do botão; remove a trilha e solta o dispositivo.
- **Nível**: `MicView.level` (pico dos últimos 250 ms) no snapshot (até 4 vezes por segundo).
- Ligar: o primeiro clique no botão publica a trilha já com som (`mudo: false`).

### 4.5 Quem assiste

- O app assina o microfone de cada anfitrião sozinho (assinatura só de áudio), fora do layout: ele
  não vira miniatura e não fecha.
- **Receptor por fonte**: o `AudioRegistry` passa a ter a chave `(participante, fonte)`; o
  microfone usa `"<participante>#mic"` como chave do mixer e do volume. Os dois receptores da mesma
  pessoa usam o mesmo relógio do participante (`PlayoutHub::clock_for`).
- `Core::watch` de um vídeo pega o som de **apps** da pessoa (nunca o microfone).
- Volume da voz: `Core::set_voice_volume(participante, volume, mudo)`, guardado como o volume de
  hoje.
- Falando: o `AudioRx` do microfone guarda o pico de cada quadro decodificado; `speaking` = pico
  > 0,03 nos últimos 300 ms.

## 5. Autoridade e reação do app

### 5.1 Local: o app do dono decide

- `RoomModel` (`crates/zap-core/src/room/model.rs`) ganha `stage: zap_proto::stage::Stage` (só o
  dono usa) e `stage_mirror: Option<StageMirror>` (o último `palco.estado`; o dono atualiza o
  próprio espelho a cada mudança, para o resto do código ler um lugar só).
- `handle_member_msg` (`room/dispatch.rs`) trata `palco.*` com `Stage::apply`, confere
  `trilha.publicar` com `Stage::can_publish` e a música com `Stage::check_music`,
  `music_moderator` e `voters`. O dono passa pelo mesmo caminho para as próprias ações
  (`OwnerRoom::stage(msg)`, como `OwnerRoom::music`).
- Expulsar: `handle_member_msg` devolve `Outgoing {Only(id), RoomClosed {"atualizar_app"}}` e já
  roda `on_member_left(model, id, "removido")`. `OwnerRoom::send_all`, ao mandar um `RoomClosed` para
  `Target::Only`, fecha a conexão dessa pessoa e marca o id como expulso; o `serve_member` dela, ao
  acabar, não repete a saída (o `on_member_left` já não acha o participante).
- Entrada com o palco ligado: o `accept_loop` chama `accept_handshake_gated(..., Some("palco"))`
  (F2), que recusa com `atualizar_app` antes da senha.
- Volta em 60 s: o `OwnerRoom` guarda `instalacao_id → último id`. No `serve_member`, se a
  instalação era de um anfitrião, `stage.rejoined(antigo, novo, agora)` **antes** de montar o
  `sala.estado`.

### 5.2 Online: o servidor decide, o app aplica

- `ws_apply::apply_ws_msg` (`online/ws_apply.rs`) guarda `palco.estado` em `stage_mirror` e emite
  `RoomEvent::StageChanged` + `Changed`.
- `Core::stage_*` manda as mensagens pelo WebSocket, sem esperar resposta (o resultado chega em
  `palco.estado`). Recusa do servidor vira `RoomEvent::Error(mensagem)` (caminho que já existe);
  `limite` com `campo: "palco"` também ajusta `hand_retry_at_ms`.
- Antes de mandar, a **conferência do app** (`stage_gate.rs`, com `Stage::from_msg` do espelho)
  recusa na hora o que o servidor recusaria: transmitir na plateia, microfone fora do palco, música
  trancada. Ela não substitui o servidor.

### 5.3 Quando a minha trilha some

**Hoje**: `RoomEvent::TrackRemoved(id)` (`engine_room.rs`, `pump_room_events`) só encerra
**assinaturas**. Se o outro lado remove uma trilha **minha**, a `ShareSession` continua capturando
e codificando (luz da câmera acesa, GPU ocupada), o `MediaServer` do Local continua com a trilha
registrada e a interface mostra "Transmissão". No Online a ponte só descarta os quadros.

**Muda** (F6):
- `TrackRemoved(id)` de trilha minha → `ShareSession::drop_local(shared, id)`: para aquela parte
  (tela, som de apps, câmera ou microfone), tira do `MediaServer`, **não** manda `trilha.remover`
  (a trilha já não existe; mandar geraria "Trilha não encontrada"). Sem nada sobrando, `request`
  vira `None`.
- `RoomEvent::StageChanged` → `enforce_stage`: sem permissão de transmitir (palco ligado e eu
  plateia) → `ShareSession::stop_local` (tudo, sem `trilha.remover`); sem permissão de microfone
  (palco desligado ou eu plateia) → `ShareSession::stop_mic_local`. O dono do Local também emite
  `StageChanged` e `TrackRemoved` das próprias trilhas que a mudança tirou (os microfones, ao
  desligar o palco).
- Publicar enquanto vira plateia (corrida): o `trilha.publicar` volta `proibido`; o `start` falha e
  já limpa o que criou (caminho de hoje).

## 6. Frentes

Uma por crate (ou pasta do front-end). Duas frentes do mesmo crate nunca andam juntas (F6 e F7 são
em sequência). Os números de teste estão em cada frente.

### F1. `contrato`: `zap-proto` + `docs/api/`

Primeira, sozinha (só a F3 anda junto). Vai para o GitHub antes de o servidor começar.

`docs/api/`: os textos de §3.1 a §3.3.

`crates/zap-proto/src/types.rs`:

```rust
/// Origem do som de uma trilha `audio` (`fonte`). Ausente = `Apps`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AudioSource {
    #[default]
    #[serde(rename = "apps")]
    Apps,
    #[serde(rename = "microfone")]
    Mic,
}

// Participant, campo novo:
    /// O que o app sabe além do protocolo 4 base (`version::FEATURES`). Ausente = vazio.
    #[serde(rename = "recursos", default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,

// Track, campos novos:
    /// Só `audio`: `Some(Mic)` = microfone; `None` = som de apps. Desconhecido = `None`.
    #[serde(rename = "fonte", default, deserialize_with = "tolerant", skip_serializing_if = "Option::is_none")]
    pub source: Option<AudioSource>,
    /// Só microfone: mudo.
    #[serde(rename = "mudo", default, skip_serializing_if = "std::ops::Not::not")]
    pub muted: bool,

impl Track {
    /// Fonte do som; `None` em trilha de vídeo.
    pub fn audio_source(&self) -> Option<AudioSource>;
    pub fn is_mic(&self) -> bool;
}
```

`crates/zap-proto/src/control.rs`:

```rust
// Hello, campo novo:
    #[serde(rename = "recursos", default, skip_serializing_if = "Vec::is_empty")]
    features: Vec<String>,
// TrackPublish, campo novo:
    #[serde(rename = "fonte", default, deserialize_with = "crate::types::tolerant", skip_serializing_if = "Option::is_none")]
    source: Option<AudioSource>,
// TrackUpdate, campo novo:
    #[serde(rename = "mudo", default, skip_serializing_if = "Option::is_none")]
    muted: Option<bool>,

// Variantes novas:
    #[serde(rename = "palco.ligar")]
    StageSet { #[serde(rename = "ligado")] on: bool },
    #[serde(rename = "palco.anfitriao")]
    StageHost {
        #[serde(rename = "participante_id")] participant_id: String,
        #[serde(rename = "anfitriao")] host: bool,
    },
    #[serde(rename = "palco.mao")]
    StageHand {
        #[serde(rename = "levantada")] raised: bool,
        #[serde(rename = "participante_id", default, skip_serializing_if = "Option::is_none")]
        participant_id: Option<String>,
    },
    #[serde(rename = "palco.musica")]
    StageMusic { #[serde(rename = "plateia")] audience: bool },
    #[serde(rename = "palco.estado")]
    StageState {
        #[serde(rename = "ligado")] on: bool,
        #[serde(rename = "anfitrioes")] hosts: Vec<String>,
        #[serde(rename = "maos", default)] hands: Vec<String>,
        #[serde(rename = "plateia_musica", default)] audience_music: bool,
    },
```

`discovery.rs`: `RoomSummary` ganha
`#[serde(rename = "palco", default, skip_serializing_if = "std::ops::Not::not")] pub stage: bool`.

`version.rs`: `pub const FEATURES: &[&str] = &["palco"];` e
`pub fn features() -> Vec<String>`.

`crates/zap-proto/src/stage.rs` (novo, puro, sem I/O; o tempo vem de quem chama):

```rust
pub const FEATURE: &str = "palco";
pub const HAND_INTERVAL: Duration = Duration::from_secs(5);
pub const HAND_AFTER_DECLINE: Duration = Duration::from_secs(30);
pub const HOST_GRACE: Duration = Duration::from_secs(60);

/// Um presente na sala, para as regras que olham todo mundo.
pub struct StageMember<'a> { pub id: &'a str, pub has_feature: bool }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusicAction { Add, Remove, Vote, Pause, Seek, Info }

/// O que quem chama tem de fazer depois de uma ação (§2.8).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct StageChange {
    /// Mandar `palco.estado` a todos.
    pub changed: bool,
    /// Viraram plateia com o palco ligado: todas as trilhas saem. Não inclui `must_leave`.
    pub demoted: Vec<String>,
    /// O palco desligou: todo microfone sai.
    pub drop_mics: bool,
    /// Apps sem o recurso: saem da sala (`sala.encerrada {motivo: "atualizar_app"}`).
    pub must_leave: Vec<String>,
    /// Quem vota na música mudou: `MusicQueue::left` para quem não vota mais.
    pub voters_changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StageError {
    #[error("Só o dono da sala faz isso")] OwnerOnly,
    #[error("Só os anfitriões fazem isso")] HostOnly,
    #[error("No modo palco, só os anfitriões transmitem")] AudienceCannotPublish,
    #[error("O microfone só existe no modo palco, para os anfitriões")] MicOutsideStage,
    #[error("No palco, a música fica com os anfitriões")] MusicLocked,
    #[error("O modo palco está desligado")] StageOff,
    #[error("Você já está no palco")] AlreadyHost,
    #[error("Só dá para levantar a própria mão")] NotYourHand,
    #[error("Essa pessoa não está na sala")] UnknownParticipant,
    #[error("O dono da sala é sempre anfitrião")] OwnerIsAlwaysHost,
    #[error("Espere {} s para pedir de novo", .0.div_ceil(1000))] HandTooSoon(u64),
}

impl StageError {
    /// "proibido" | "invalido" | "limite" (tabela de erros de §3.1).
    pub fn code(&self) -> &'static str;
    /// "palco" ou "participante_id".
    pub fn field(&self) -> &'static str;
    /// Só `HandTooSoon`.
    pub fn retry_ms(&self) -> Option<u64>;
    /// `erro` pronto, com `campo`, `tentar_em_ms` e o `ref`.
    pub fn to_msg(&self, reference: Option<String>) -> ControlMsg;
}

#[derive(Clone, Debug)]
pub struct Stage { /* on, owner, hosts: Vec<String> (sem o dono), hands: Vec<String>,
                      audience_music, last_raise, declined_at, gone_hosts: HashMap<String, Instant> */ }

impl Stage {
    pub fn new(owner_id: &str, on: bool) -> Stage;
    /// Espelho a partir de um `palco.estado` (sem os prazos). `None` se `msg` não for `StageState`.
    pub fn from_msg(owner_id: &str, msg: &ControlMsg) -> Option<Stage>;
    pub fn is_on(&self) -> bool;
    /// O dono sempre; os outros pela lista (vale com o palco desligado).
    pub fn is_host(&self, id: &str) -> bool;
    pub fn hands(&self) -> &[String];
    pub fn audience_music(&self) -> bool;
    pub fn can_publish(&self, id: &str, kind: TrackKind, source: AudioSource) -> Result<(), StageError>;
    pub fn check_music(&self, id: &str, action: MusicAction) -> Result<(), StageError>;
    /// Tira qualquer item e pula direto: o dono, ou anfitrião com o palco ligado.
    pub fn music_moderator(&self, id: &str) -> bool;
    pub fn is_voter(&self, id: &str) -> bool;
    pub fn voters<'a>(&self, present: impl IntoIterator<Item = &'a str>) -> usize;
    pub fn set_on(&mut self, by: &str, on: bool, members: &[StageMember<'_>]) -> Result<StageChange, StageError>;
    pub fn set_host(&mut self, by: &str, target: &str, host: bool, members: &[StageMember<'_>]) -> Result<StageChange, StageError>;
    pub fn raise_hand(&mut self, by: &str, raised: bool, now: Instant) -> Result<StageChange, StageError>;
    pub fn decline(&mut self, by: &str, target: &str, now: Instant) -> Result<StageChange, StageError>;
    pub fn set_audience_music(&mut self, by: &str, allowed: bool) -> Result<StageChange, StageError>;
    /// Saída: tira mão e papel; anfitrião vai para a lista de "voltou logo".
    pub fn left(&mut self, id: &str, now: Instant) -> StageChange;
    /// Volta em até `HOST_GRACE`: `new_id` vira anfitrião. Também limpa a lista vencida.
    pub fn rejoined(&mut self, old_id: &str, new_id: &str, now: Instant) -> StageChange;
    /// `palco.*` C→S: `Some(resultado)`; outra mensagem: `None`. `palco.mao` com o
    /// `participante_id` de outro e `levantada: false` = `decline`; com `true` = `NotYourHand`.
    pub fn apply(&mut self, by: &str, msg: &ControlMsg, members: &[StageMember<'_>], now: Instant)
        -> Option<Result<StageChange, StageError>>;
    /// `palco.estado`: o dono primeiro, depois os anfitriões na ordem em que subiram.
    pub fn msg(&self) -> ControlMsg;
}
```

Ordem das conferências em `can_publish`: palco ligado e plateia → `AudienceCannotPublish`; senão
microfone com o palco desligado → `MicOutsideStage`; senão ok.

**Ajuste mecânico** (tarefa à parte, declarada no crate que mexe): os literais de `Participant`,
`Track`, `ControlMsg::Hello/TrackPublish/TrackUpdate` em `zap-net`, `zap-core` e `zap-test-server`
ganham os campos novos vazios (`features: Vec::new()`, `source: None`, `muted: false`/`None`), sem
comportamento novo. O workspace inteiro compila no fim da F1.

**Aceite F1** (`cargo test -p zap-proto`):
- ida e volta de cada mensagem nova e de cada campo novo; ausentes continuam ausentes ao
  reserializar; `fonte` desconhecido (`"x"`, `3`, `null`) vira `None` sem derrubar a `Trilha`;
- `tests/compat_v35.rs`: cópia congelada dos tipos da v3.5.1 (`Room`, `Participant`, `Track`, e um
  `ControlMsg` só com `sala.estado`, `trilha.publicada`, `trilha.atualizada`,
  `participante.entrou/atualizou`, `sala.encerrada`). O JSON gerado pela 3.6 dessas mensagens, com
  `recursos`, `fonte: "microfone"` e `mudo: true`, desserializa nos tipos congelados; o
  `palco.estado` falha com "unknown variant" no `t` (o decoder antigo pula); `sala.encerrada
  {motivo: "atualizar_app"}` desserializa;
- `stage.rs`, com `Instant` simulado:
  - matriz de `can_publish` (3 situações × 5 tipos de trilha, §2.5) e de `check_music` (§2.6);
  - `set_on(true)` com dono, A anfitrião, B e C (C sem o recurso): `demoted = [B]`,
    `must_leave = [C]`, `changed`, `voters_changed`; de novo: `StageChange::default()`;
  - `set_on(false)`: `drop_mics`, mãos vazias;
  - `set_host`: promover tira a mão; tirar com o palco ligado põe em `demoted`; com o palco
    desligado, não; tirar o dono → `OwnerIsAlwaysHost`; alvo fora → `UnknownParticipant`; quem não
    é o dono → `OwnerOnly`;
  - mãos: levantar, baixar, levantar com 4,9 s → `HandTooSoon(100)`, com 5,0 s → ok; recusar e
    levantar com 29,9 s → `HandTooSoon(100)`, com 30,0 s → ok; anfitrião levantando →
    `AlreadyHost`; palco desligado → `StageOff`; mão de outro com `true` → `NotYourHand`;
  - `left` + `rejoined` com 59,9 s → anfitrião; com 60,1 s → plateia;
  - `voters`: palco ligado, chave desligada, 2 anfitriões e 10 na plateia → 2; chave ligada → 12;
  - `msg()`: o dono primeiro, depois a ordem de promoção;
  - `from_msg(msg())` dá as mesmas respostas de `can_publish`, `check_music` e `is_host`.

### F2. `net`: `zap-net`

Depende de F1.

- `handshake::Hello` ganha `pub features: Vec<String>`; o cliente manda em `recursos`; o servidor
  preenche ao ler o `ola`.
- `pub async fn accept_handshake_gated(conn: &quinn::Connection, auth: &ServerAuth, room: &Room,
  participant_id: String, media_secret: Option<&str>, require: Option<&str>)
  -> Result<(ControlTx, ControlRx, Hello), NetError>`: depois de conferir o protocolo e **antes** do
  desafio de senha, se `require = Some(f)` e `!hello.features.contains(f)`, responde
  `recusado {codigo: "atualizar_app", mensagem: "Esta sala está no modo palco. Atualize o streamaZap (versão 3.6.0 ou mais nova) para entrar."}`.
  `accept_handshake_with` passa a chamar esta com `require = None`.
- `online::types`: `ServerInfo.features: Vec<String>` (`recursos`, `default`);
  `LoginRequest.features: Vec<String>` (`recursos`, omitido vazio); `CreateRoomRequest.stage: bool`
  (`palco`, omitido `false`); `RoomListing.stage: bool` (`palco`, `default`).

**Aceite F2**: `ola` com e sem `recursos` (ida e volta); `accept_handshake_gated` com
`Some("palco")` recusa o cliente sem o recurso com `NetError::Rejected("atualizar_app")` em menos de
1 s e sem pedir senha numa sala com senha; aceita o cliente com o recurso; `None` aceita os dois;
`LoginRequest` com `features` vazio não tem `recursos` no JSON; `ServerInfo` sem `recursos` →
vazio; `CreateRoomRequest {stage: false}` sem `palco`; `RoomListing` sem `palco` → `false`.

### F3. `audio`: `zap-audio`

Não depende de nada (o `zap-audio` não usa o `zap-proto`): começa junto com a F1.

Portáveis (testam no Linux):

```rust
// resample3.rs: 16 kHz → 48 kHz, ×3, FIR polifásico (§4.3).
pub struct Upsampler3 { /* histórico de 32 amostras */ }
impl Upsampler3 {
    pub fn new() -> Self;
    /// 320 amostras de 16 kHz → 960 de 48 kHz.
    pub fn process(&mut self, input: &[f32; 320], out: &mut [f32; FRAME]);
}
// level.rs
pub struct PeakHold { /* janela */ }
impl PeakHold {
    pub fn new(hold_us: u64) -> Self;
    pub fn push(&mut self, peak: f32, now_us: u64);
    pub fn value(&self, now_us: u64) -> f32;
}
// opus.rs
pub struct OpusVoiceEnc { /* mono, Voip, 48 kbps, FEC, perda 10% */ }
impl OpusVoiceEnc {
    pub fn new(kbps: u32) -> Result<Self, AudioError>;
    pub fn encode(&mut self, pcm: &[f32; FRAME]) -> Result<Bytes, AudioError>;
}
```

Só Windows (com stub fora: lista vazia e `AudioError::Unsupported`):

```rust
pub struct MicDevice { pub id: String, pub name: String, pub is_default: bool }
/// Microfones ativos; `is_default` = o padrão de comunicação.
pub fn list_microphones() -> Result<Vec<MicDevice>, AudioError>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormFactor { Headphones, Speakers, Unknown }
pub fn default_output_form_factor() -> OutputFormFactor;

pub struct MicOptions { pub device_id: Option<String>, pub processing: bool }
/// 20 ms mono 48 kHz.
pub struct MicBlock { pub pcm: Box<[f32; FRAME]>, pub capture_us: u64, pub level: f32 }
pub struct MicCapture { /* thread "zap-mic" (MMCSS "Pro Audio") */ }
impl MicCapture {
    /// `processing` tenta o DSP (§4.2) e cai para a captura direta se ele falhar.
    pub fn start(opts: MicOptions, out: crossbeam_channel::Sender<MicBlock>) -> Result<Self, AudioError>;
    /// O DSP está ativo de verdade.
    pub fn processing_active(&self) -> bool;
    pub fn errors(&self) -> Vec<String>;
}
```

- Módulos: `mic.rs` (escolha e reserva), `mic_dsp.rs` (DMO em modo fonte; implementa `IMediaBuffer`
  com `#[implement]`, como o `loopback.rs` já faz com o tratador de ativação), `mic_dsp_keys.rs`,
  `mic_wasapi.rs` (captura direta), `mic_devices.rs` (lista e forma da saída).
- `AudioError` ganha `MicBlocked` ("O Windows bloqueou o microfone…", §4.2).
- `unsafe`: só aqui, como já existe no loopback, com `// SAFETY:` em cada bloco.
- Thread nova `zap-mic` por microfone ligado: a captura tem de rodar em tempo real, longe do tokio
  (o mesmo motivo do `zap-audio-capture`). Canal limitado a 8 blocos; cheio = descarta o mais velho.
- Features do `windows` a acrescentar no `zap-audio`: `Win32_Media_DxMediaObjects`,
  `Win32_Media_MediaFoundation` (`CWMAudioAEC`, `AEC_SYSTEM_MODE`) e `Win32_Devices_FunctionDiscovery`
  (`PKEY_Device_FriendlyName`); conferir na compilação.

**Aceite F3**:
- Linux: `Upsampler3` com ganho DC 1 ± 0,01; seno de 1 kHz a 16 kHz sai com 1 kHz (cruzamentos por
  zero em 1 s: 2000 ± 2) e distorção < −50 dB; imagem em 15 kHz (de 1 kHz) atenuada > 40 dB.
  `PeakHold(250 ms)`: segura o pico 250 ms e cai depois. `OpusVoiceEnc` + `OpusDec` estéreo: 960
  amostras mono viram 1920 com os dois canais iguais; seno de 440 Hz a −20 dBFS com relação sinal/ruído
  > 20 dB (procurando o atraso em ±480 amostras); pacote médio ≤ 140 bytes (48 kbps).
- Windows (pula com aviso sem microfone): `list_microphones` não falha; `MicCapture` no padrão
  entrega ≥ 90 blocos em 2 s (esperado 100), período médio 20 ± 2 ms; com `processing: true`,
  `processing_active()` ou um aviso de reserva em `errors()`; `default_output_form_factor` não entra
  em pânico.

### F4. `media`: `zap-media`

Depende de F3.

```rust
pub struct MicConfig { pub device: Option<String>, pub processing: bool }
pub struct MicStatus { pub level: f32, pub processing: bool, pub errors: Vec<String> }
pub struct MicPipeline { /* captura + thread "zap-mic-enc" */ }
impl MicPipeline {
    /// Windows: `MicCapture` → `OpusVoiceEnc(48)` → `EncodedAudio`. Fora: `MediaError::Unsupported`.
    pub fn start(track: u16, cfg: MicConfig, out: Sender<EncodedAudio>) -> Result<Self, MediaError>;
    /// Todas as plataformas: seno de 440 Hz a −20 dBFS (pico 0,1), mono, 20 ms.
    pub fn start_synthetic(track: u16, out: Sender<EncodedAudio>) -> Result<Self, MediaError>;
    pub fn set_muted(&self, muted: bool);
    /// Reabre a captura (dispositivo ou processamento novos) sem mudar `track` nem `seq`.
    pub fn reconfigure(&self, cfg: MicConfig) -> Result<(), MediaError>;
    pub fn status(&self) -> MicStatus;
}
```

Thread nova `zap-mic-enc`: o Opus fora da thread de captura (mesmo desenho do `zap-audio-enc`).

**Aceite F4**: sintético entrega 50 ± 3 pacotes por segundo com `seq` contínua e decodificáveis;
`set_muted(true)` → no máximo 3 pacotes depois e nenhum após 100 ms; ao desmutar, o primeiro pacote
tem `seq` = último + 1; `status().level` 0,1 ± 0,02 (também no mudo); `drop` em menos de 500 ms.
Windows: `start` com o microfone de verdade ou pula.

### F5. `servidor-teste`: `zap-test-server`

Depende de F1 e F2. Faz no servidor de teste o que a seção 7 pede ao de produção, para os testes do
`zap-core` rodarem no Linux: `recursos` na sessão e no `/v1/info` (`["palco"]`), `palco` em criar e
na lista, `426 atualizar_app`, `palco.*` com `Stage::apply`, as regras de publicar e de música, a
ordem de §2.8, `mudo` repassado, `audio: true` sem o microfone na ponte (`relay.rs`), expulsão do app
antigo e a volta em 60 s.

**Aceite F5** (`tests/stage.rs`, contra o servidor rodando no loopback):
- sala criada com `palco: true` aparece com `palco: true` na lista; quem entra recebe
  `sala.estado` e logo depois `palco.estado {ligado: true, anfitrioes: [dono]}`;
- plateia publica tela → `erro proibido` (`campo: "palco"`, `ref` igual); promovida → publica;
  tirada → `trilha.removida` antes de `palco.estado`;
- `palco.ligar true` com duas pessoas transmitindo: as trilhas da plateia saem; a sessão sem
  recursos recebe `sala.encerrada {motivo: "atualizar_app"}` e os outros `participante.saiu
  {motivo: "removido"}`; ela tenta `entrar` de novo → `426 atualizar_app`;
- microfone com o palco desligado → `proibido`; anfitrião com som de apps e microfone ao mesmo
  tempo → ok; segundo microfone → `invalido`; `palco.ligar false` → só os microfones saem;
- mão: levantar duas vezes em 5 s → `limite` com `tentar_em_ms` entre 1 e 5000; recusada e levantada
  em 30 s → `limite`;
- música trancada: plateia adiciona → `proibido`; `musica.info` passa; 2 anfitriões e 3 na plateia:
  `precisa = 1`; chave ligada: `precisa = 3`;
- espectador que assina só o vídeo do anfitrião com `audio: true` recebe só os pacotes do som de
  apps; assinando o microfone, recebe os dele;
- anfitrião sai e volta com o mesmo participante em 1 s: anfitrião de novo.

### F6. `nucleo-palco`: `zap-core` (palco, sem microfone)

Depende de F1, F2 e F5.

- `room/model.rs`: `stage`, `stage_mirror: Option<StageMirror>`,
  `pub fn stage_rules(&self) -> Option<zap_proto::stage::Stage>` (do espelho) e
  `pub fn stage_view(&self, my_id: &str) -> Option<StageView>`.
- `room/dispatch.rs`: `palco.*`, a regra em `trilha.publicar` e na música, expulsão (§5.1);
  `on_member_joined(model, p, rejoin_of: Option<String>)` chama `stage.rejoined` (volta em 60 s) e
  manda `palco.estado` para `AllBut(novo)` só se a lista mudou (o novo recebe o dele direto do
  `serve_member`, logo depois do `sala.estado`); `on_member_left` chama `stage.left`.
- `room/owner.rs`: `OwnerRoom::stage(msg)`; `start` com `req.stage`; `send_all` fecha a conexão de
  quem recebeu `RoomClosed` em `Only`; `accept_loop` com `accept_handshake_gated`; `instalacao_id →
  id` para a volta em 60 s; anúncio da descoberta com `stage`.
- `room/owner_member.rs`: `Participant.features` do `hello.features` (o `Participant` do dono leva
  `version::features()`); `palco.estado` entre `sala.estado` e o histórico.
- `room/apply.rs` e `online/ws_apply.rs`: `StageState` → espelho + `Effect::StageChanged` (membro
  Local, que o `member_run` vira `RoomEvent::StageChanged`) ou `RoomEvent::StageChanged` (Online);
  `Effect::Error` e o tratamento de `erro` ganham `field` e `retry_ms`.
- `hand_retry_at_ms` = o maior entre: última levantada + 5 s, recusa + 30 s e agora +
  `tentar_em_ms` do último `limite` com `campo: "palco"`. Some quando passa.
- `room/mod.rs`: `RoomEvent::StageChanged`.
- `stage_gate.rs` (novo): conferência do app (§5.2) e
  `pub(crate) fn infer_declined(prev: &StageMirror, next: &StageMirror, me: &str, lowered_recently: bool) -> bool`
  (minha mão sumiu de `maos`, eu não baixei nos últimos 5 s, o palco segue ligado e eu não virei
  anfitrião).
- `share.rs`: `ShareSession::drop_local(shared, track_id)` e `stop_local(shared)` (§5.3).
- `engine_room.rs`: `TrackRemoved` de trilha minha → `drop_local`; `StageChanged` →
  `enforce_stage`.
- `online/`: `CreateRoomRequest.stage`; `LoginRequest.features = zap_proto::version::features()`;
  `ServerStatus.stage` (o `/v1/info` tem `"palco"`); criar com palco em servidor sem o recurso →
  `CoreError::Invalid("O servidor ainda não tem o modo palco")`.
- Snapshot (`snapshot.rs`):

```rust
pub struct StageView {
    pub on: bool,
    pub audience_music: bool,
    /// O dono primeiro, depois na ordem em que subiram.
    pub hosts: Vec<String>,
    /// Na ordem de chegada.
    pub hands: Vec<String>,
    /// Posso levantar a mão de novo a partir daqui (ms desde 1970). Ausente = já posso.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hand_retry_at_ms: Option<u64>,
    /// O dono recusou meu último pedido neste instante (ms desde 1970).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declined_at_ms: Option<u64>,
}
// RoomView, campo novo (ausente = a fonte não tem palco):
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage: Option<StageView>,
// snapshot::Participant, campos novos:
    #[serde(default, skip_serializing_if = "std::ops::Not::not")] pub host: bool,      // palco ligado e anfitrião
    #[serde(default, skip_serializing_if = "std::ops::Not::not")] pub hand: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")] pub old_app: bool,   // sem "palco" em recursos (só com fonte que tem palco)
// RoomListing, campo novo:
    #[serde(default, skip_serializing_if = "Option::is_none")] pub stage: Option<bool>,
// types::CreateRoom, campo novo:
    #[serde(default)] pub stage: bool,
// online::ServerStatus, campo novo:
    #[serde(default)] pub stage: bool,
```

- `Core` (`engine_stage.rs`, novo):

```rust
impl Core {
    pub async fn stage_set(&self, on: bool) -> Result<(), CoreError>;
    pub async fn stage_set_host(&self, participant_id: &str, host: bool) -> Result<(), CoreError>;
    pub async fn stage_hand(&self, raised: bool) -> Result<(), CoreError>;
    pub async fn stage_decline(&self, participant_id: &str) -> Result<(), CoreError>;
    pub async fn stage_audience_music(&self, allowed: bool) -> Result<(), CoreError>;
}
```

  Sem sala: `NoRoom`. Fonte sem palco (`stage_mirror` vazio): `Invalid("Esta sala não tem modo palco")`.

**Aceite F6**:
- `dispatch` (puro): as matrizes de §2.5 e §2.6 pelo dono; `palco.ligar` de membro → `erro
  proibido` só para ele; ordem de §2.8 (`trilha.removida` antes de `palco.estado`; `RoomClosed`
  em `Only` antes da saída).
- Dois apps no loopback, Local:
  - dono cria com `stage: true`; o membro recebe `RoomView.stage.on == true` em até 1 s;
    `start_share` do membro → `Invalid` sem mandar nada (o dono não recebe `trilha.publicar`);
    `trilha.publicar` cru pelo controle → `erro proibido`;
  - dono promove → o membro publica; dono tira → em até 1 s o `snapshot.share` do membro fica
    `None`, o dono não recebe `trilha.remover` dele e o modelo do dono não tem trilha dele;
  - membro com `Hello` sem recursos: com o palco ligado, `join` → `Rejected("atualizar_app")`; com o
    palco desligado entra; o dono liga → ele recebe `Closed("atualizar_app")` em até 1 s, e o outro
    membro vê a saída com `removido`;
  - anfitrião derruba a conexão e reconecta em menos de 60 s (com `TEMPO_CONECTAR` normal): continua
    em `hosts` e a republicação passa;
  - mão: levantar → o dono vê em `hands`; `stage_decline` → o membro vê `declined_at_ms` e
    `hand_retry_at_ms` ≈ agora + 30 000 (± 500).
- Online com o `zap-test-server`: criar com palco, promover, tirar (o `ShareSession` para em até
  1 s), mão e recusa, música trancada (o `music_add` da plateia → `Invalid` sem ir ao servidor),
  servidor sem `"palco"` (`/v1/info` de teste sem recursos) → `create_room` com palco → `Invalid`.
- `drop_local`: o servidor de teste remove uma trilha minha → aquela parte para e o resto continua.

### F7. `nucleo-mic`: `zap-core` (microfone e vozes)

Depende de F4 e F6.

- `publish/audio.rs`: `AudioSource` ganha `Mic(zap_media::MicConfig)` e `SyntheticMic`;
  `AudioPublisher` ganha `pub source: zap_proto::AudioSource`, `set_muted`, `reconfigure`,
  `mic_status`.
- `publish/media_serve.rs`: `ajustar_audio` por trilha: o som de apps vai quando um vídeo pede áudio
  ou ele é assinado direto; o microfone, só assinado direto (hoje pega uma trilha de áudio só, a de
  menor id, e quebraria com duas).
- `share_mic.rs` (novo, `impl ShareSession`): `mic: Option<PublishedMic>`;
  `set_mic(shared, on)`, `set_mic_muted(shared, muted)` (manda `trilha.atualizar {mudo}`),
  `set_mic_device(shared, id)`, `set_mic_processing(shared, on)`, `stop_mic_local(shared)`. O
  microfone não entra no `ShareRequest`: liga e desliga à parte da tela. `stop` (sair da sala) e
  `republish` (reconexão) cuidam dele.
- `subscribe/audio_rx.rs`: chave `(participante, fonte)` (§4.5), pico por quadro, `speaking`;
  `subscribe/mod.rs` e `engine_watch.rs`: vídeo leva o som de apps; microfone = assinatura só de
  áudio com a chave de microfone.
- `sources.rs`: `Sources.microphones: Vec<SourceItem>` (`SourceKind::Mic`, id `mic:<id>`).
- `settings.rs`: `SoundSettings.mic_id: Option<String>` (padrão `None`) e `mic_processing: bool`
  (padrão `true`, também em arquivo antigo).
- Snapshot:

```rust
pub struct MicView {
    pub device: String,             // nome do dispositivo em uso
    pub device_id: Option<String>,  // `mic:<id>`; None = padrão de comunicação
    pub muted: bool,
    pub processing: bool,           // cancelamento ativo de verdade
    pub level: f32,                 // 0..1, pico dos últimos 250 ms
    pub headphones: Option<bool>,   // Some(true) fone, Some(false) caixa, None não sei
}
// Snapshot, campo novo (None = microfone desligado):
    #[serde(default, skip_serializing_if = "Option::is_none")] pub mic: Option<MicView>,
// snapshot::Participant, campos novos:
    #[serde(default, skip_serializing_if = "std::ops::Not::not")] pub speaking: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub voice_volume: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub voice_muted: Option<bool>,
// TrackViewKind ganha `Mic` ("mic"); TrackView ganha
    #[serde(default, skip_serializing_if = "std::ops::Not::not")] pub muted: bool,
```

- `Core`:

```rust
impl Core {
    pub async fn mic_set(&self, on: bool) -> Result<(), CoreError>;
    pub async fn mic_mute(&self, muted: bool) -> Result<(), CoreError>;
    pub async fn mic_device(&self, device_id: Option<String>) -> Result<(), CoreError>;
    pub async fn mic_processing(&self, on: bool) -> Result<(), CoreError>;
    pub fn set_voice_volume(&self, participant_id: &str, volume: f32, muted: bool);
}
```

  `mic_set(true)` sem palco ou sendo plateia: `Invalid` (a mensagem de `MicOutsideStage` ou
  `AudienceCannotPublish`), sem mandar nada.

**Aceite F7** (sintético, roda no Linux):
- anfitrião publica microfone: `trilha.publicar` com `fonte: "microfone"`, `audio {taxa: 48000,
  canais: 1, kbps: 48}`; quem assiste recebe ≥ 45 pacotes por segundo no receptor de chave
  `"<id>#mic"`;
- som de apps e microfone da mesma pessoa tocam em duas trilhas do mixer;
  `set_voice_volume(id, 0.0, false)` deixa a voz em 0 e o som de apps em 0,8;
- `watch` do vídeo do anfitrião registra o som de apps, não o microfone;
- Local: espectador que assina só o microfone não recebe pacote do som de apps (contador de pacotes
  por número de trilha na conexão);
- `mic_mute(true)`: nenhum pacote depois de 100 ms; o espectador vê `TrackView.muted` em até 1 s;
  `speaking` vai a `false` em até 500 ms; desmutar volta;
- tom sintético → `speaking == true` em até 500 ms;
- palco desligado → em até 1 s `snapshot.mic == None` e o `MicPipeline` caiu; anfitrião tirado →
  idem;
- `mic_device` e `mic_processing` com o microfone ligado: mesma `trilha_id`, sem republicar.

### F8. `tauri`: `app/src-tauri`

Depende de F7.

Comandos novos (em `commands/room.rs` e `commands/share.rs`), cada um só repassa ao `Core`:
`stage_set(on)`, `stage_set_host(participant_id, host)`, `stage_hand(raised)`,
`stage_decline(participant_id)`, `stage_audience_music(allowed)`, `mic_set(on)`, `mic_mute(muted)`,
`mic_device(device_id: Option<String>)`, `mic_processing(on)`,
`set_voice_volume(participant_id, volume, muted)`. Registrados no `invoke_handler`.

**Aceite F8**: `cargo check --workspace --all-targets` e `cargo clippy` limpos; o teste de nomes
(como os de `conf_tests.rs`) lista os 10 comandos.

### F9. `interface`: `app/src`

Depende de F1 (o desenho dos tipos está neste plano). Anda junto com F6 e F7 contra o mock e liga no
núcleo depois da F8. Sem design pronto em `docs/design/`: seguir o estilo atual (tokens de
`theme.css`, dock redondo de 44 px de `SalaControles.tsx`, o selo âmbar `#FFB547` sobre
`rgba(255,181,71,0.15)` do "host" em `PainelPessoas.tsx`).

**Ponte** (`bridge/types.ts`, `api.ts`, `mock.ts`):
- tipos: `CreateRoom.stage?`, `StageView`, `RoomView.stage?`, `Participant.host/hand/old_app/speaking/
  voice_volume/voice_muted`, `TrackKind` com `'mic'`, `TrackView.muted?`, `RoomListing.stage?`,
  `MicView`, `Snapshot.mic?`, `ServerStatus.stage?`, `Sources.microphones`,
  `settings.sound.mic_id/mic_processing` (mesmos nomes do Rust);
- `api`: `stageSet`, `stageSetHost`, `stageHand`, `stageDecline`, `stageAudienceMusic`, `micSet`,
  `micMute`, `micDevice`, `micProcessing`, `setVoiceVolume`;
- mock com as regras de §2 e o parâmetro `?palco=dono|anfitriao|plateia` (como o `?plano=`).

**Telas**:

| Onde | O que |
|---|---|
| `criar/CriarNomeAcesso.tsx` (seção 03) | Abaixo dos cartões de acesso, uma linha com `Toggle` **Modo palco** e a dica "Só você e quem você chamar transmitem. Os outros assistem e conversam no chat." No Online com servidor sem o recurso (`server.stage` falso): desligado, com "O servidor ainda não tem o modo palco". `montarPedido` leva `stage` |
| `criar/CriarResumo.tsx` | Linha "Modo palco: só você transmite até chamar alguém" |
| `salas/SalaItem.tsx` (e o detalhe) | Selo **PALCO** âmbar ao lado do nome quando `listing.stage` |
| `sala/SalaCabecalho.tsx` | Chip **PALCO** âmbar depois do chip do modo, com o palco ligado |
| `sala/SalaControles.tsx` (dock) | **Anfitrião no palco**: botão redondo de microfone entre a câmera e o Compartilhar. Desligado: vermelho (`#3A1C21`/`#FF8592`, como a câmera desligada) com microfone riscado, "Ligar o microfone"; ligado: normal, com um anel verde `#3BF08A` cuja espessura (0 a 4 px) segue `mic.level`; mudo: vermelho riscado, "Tirar o mudo (V)". Uma seta pequena colada abre o **menu do microfone**: lista de dispositivos (rádio, "Padrão do Windows" primeiro), `Toggle` "Cancelar eco e ruído", barra de nível, aviso de sem fone (§4.2), "Desligar microfone". **Plateia**: câmera e Compartilhar com `aria-disabled`, opacidade 0,5 e o título "No modo palco, só os anfitriões transmitem. Levante a mão para pedir para subir."; antes do separador do Sair, a pílula **Levantar a mão** (ícone de mão); levantada, fica âmbar com "Baixar a mão"; no prazo, desligada com "Pedir de novo em 25 s" |
| `sala/SalaTransmissao.tsx` | Plateia: em vez do formulário, "No modo palco, só os anfitriões transmitem." e o botão Levantar a mão |
| `sala/PainelPessoas.tsx` | Palco ligado: no topo, para o dono, **Pedidos para subir · N** com Aceitar (verde) e Recusar em cada um; depois **No palco · N** (o dono com o selo "dono", os outros com "anfitrião") e **Plateia · N** (mão âmbar ao lado de quem pediu, selo cinza "app antigo" em `old_app`). Anfitrião com microfone: ícone de microfone (riscado no mudo), anel verde no avatar quando `speaking`, e uma linha de volume "Voz". Menu ⋯ do dono em cada pessoa: "Tornar anfitrião" / "Tirar do palco". No topo, para o dono, `Toggle` **Modo palco**; para todo anfitrião, `Toggle` **Plateia pode pôr música** |
| `sala/SalaPainel.tsx` | Selo da aba Pessoas = número de mãos, só para o dono |
| Confirmação ao ligar o palco | "Ligar o modo palco? As transmissões de quem não é anfitrião vão parar." Com `old_app` na sala: "Ju e Rafa estão com o app antigo e vão sair da sala até atualizarem." Botões Ligar / Cancelar |
| `sala/musica/*` | Plateia com a música trancada: sem campo de link, sem votar, pausar, buscar e remover; linha "No palco, a música fica com os anfitriões." Anfitrião no palco: remover em todos os itens |
| `sala/SalaPrincipal.tsx` | Nos controles de cada transmissão de quem tem microfone, segunda linha de volume "Voz" (o mesmo estado do painel). Tecla **V**: mudo do meu microfone (ou liga, se desligado) |
| `sala/SalaScreen.tsx` | Assina sozinho o microfone de cada anfitrião (`vozesDaSala`, sem célula, sem fechar). Avisos (hook `useAvisosPalco`, comparando o `StageView` anterior e o atual): "Você subiu ao palco: agora pode transmitir e usar o microfone"; "Você voltou para a plateia: suas transmissões pararam"; "O dono recusou seu pedido. Você pode pedir de novo em 30 s"; para o dono, "Rafa quer subir ao palco" com Aceitar/Recusar (som de mensagem, se ligado). Linhas de sistema no chat: "Modo palco ligado/desligado", "Ju subiu ao palco", "Rafa voltou para a plateia" |
| `config/ConfigSom.tsx` | Seção **Microfone (modo palco)**: dispositivo e "Cancelar eco e ruído" |

`sala/salaModel.ts`: `streamsDaSala` ignora `kind: 'mic'`; função nova
`vozesDaSala(room, meId): Stream[]` (os microfones dos outros); `kindLabel('mic') = 'Microfone'`.

**Aceite F9** (`npm run check`, `npm test`):
- `CriarNomeAcesso`: o toggle aparece no Local e no Online com recurso; desligado e com o texto sem
  recurso; `montarPedido` com `stage: true`;
- `SalaControles`: plateia vê câmera e Compartilhar desligados (clicar não chama nada) e Levantar a
  mão; levantada mostra Baixar; no prazo mostra a contagem; anfitrião vê o microfone nos 3 estados;
  fora do palco, nenhum dos dois;
- `PainelPessoas`: grupos e contagens; Aceitar chama `stageSetHost(id, true)`; Recusar chama
  `stageDecline(id)`; menu do dono; não-dono não vê pedidos nem menu;
- música trancada esconde os controles da plateia;
- `vozesDaSala` devolve só microfones dos outros; `streamsDaSala` não muda com microfone na sala;
- `useAvisosPalco`: os quatro avisos nas transições certas e nenhum ao abrir a sala;
- confirmação ao ligar lista os `old_app`;
- tecla V chama `micMute`/`micSet`; com foco num campo de texto, não;
- selo PALCO na lista e no cabeçalho.

## 7. Servidor (`streamaZapi`)

Feito por outra sessão, no repositório do servidor, depois que a F1 estiver no GitHub. Referência
em código: o `zap-test-server` da F5 e o `zap_proto::stage`.

**Crates**: subir o `rev` de `zap-proto`, `zap-rate` e `zap-net` para o commit da F1 (e, no fim,
para o do lançamento). Ajuste mecânico: `ProtoParticipant {.., features}`, `Track {.., source,
muted}` e o `match` de `ControlMsg::TrackUpdate` (agora com `muted`).

**Estado** (`src/state.rs`):
- `Participant.features: Vec<String>` (da sessão); `proto_participant` preenche `recursos`.
- `Room.stage: zap_proto::stage::Stage` (`Stage::new(dono, palco)` no `create_room`).
- `send_snapshot`: `sala.estado`, **`palco.estado`**, histórico do chat (a música continua depois,
  no `join`).
- `join`: sala com o palco ligado e participante sem `"palco"` → `ApiError::upgrade_required()`;
  participante que volta: `stage.rejoined(pid, pid, agora)` antes do `send_snapshot`; se mudou,
  `palco.estado` para os outros.
- `leave`: `stage.left(pid, agora)`; se mudou, `palco.estado` depois do `participante.saiu`. O
  `presentes` da música passa a ser `stage.voters(membros)`.
- Helper `apply_stage_change(room_id, change, agora)`: a ordem de §2.8. Expulsar = `sala.encerrada
  {motivo: "atualizar_app"}` só para ele, `leave(room, pid, "removido")` e
  `authorized.remove(room_id)`.

**REST** (`src/http.rs`):
- `GET /v1/info`: `"recursos": ["palco"]`.
- `NewSession.recursos: Vec<String>` (`default`; até 16 itens de até 32 caracteres, o resto
  ignorado).
- `NewRoom.palco: bool` (`default`); sessão sem `"palco"` pedindo `true` → `400 invalido`
  (`campo: "palco"`).
- `summary`: `"palco": true` quando ligado.
- `enter_room`: sala com o palco ligado e sessão sem `"palco"` → `426 atualizar_app` (antes da
  senha e do plano; vale também para quem já está autorizado).

**Controle** (`src/control.rs`):
- `dispatch`: `StageSet`, `StageHost`, `StageHand`, `StageMusic` → `room.stage.apply(pid, &msg,
  &membros, agora)`; `Err(e)` → `e.to_msg(ref)` só para quem pediu; `Ok(change)` →
  `apply_stage_change`. `StageState` vindo do app: ignorado. Prévia: `proibido` (já é).
- `track_publish`: `stage.can_publish(pid, kind, fonte)` depois do `present_room` e antes do plano;
  conflito por `(Screen|Window)`, `Camera`, `(Audio, Apps)`, `(Audio, Mic)` (mensagem "Você já
  compartilha uma trilha desse tipo (uma tela ou janela, uma câmera, um som de apps e um
  microfone)"); `Track.source = Some(Mic)` só no microfone; `muted = false`.
- `track_update`: `mudo` só em microfone (outros: descarta); `mudo` sozinho não confere plano.
- `music`: `stage.check_music(pid, ação)` antes da fila (`MusicAction::Info` passa);
  `eh_dono = room.owner == pid || stage.music_moderator(pid)`; `presentes = stage.voters(...)`.

**Ponte** (`src/relay.rs`): `on_audio` e `receives_audio`: o som que vai para quem pediu
`audio: true` num vídeo é só o de trilha **sem** `fonte: microfone`. O `TrackRelay` guarda a fonte.

**Erros** (`src/error.rs`): `ApiError::upgrade_required()` (426, `atualizar_app`) e
`ApiError::stage(e: &StageError)` (código, `campo`, `tentar_em_ms`).

**Testes** (`tests/e2e.rs`, pelo WebSocket e pelo QUIC), os mesmos de **Aceite F5**, mais:
- compatibilidade: o `erro proibido` e o `palco.estado` do servidor lidos com o `ControlMsg` do app
  (como o teste de `plano_insuficiente` da fase 3);
- servidor com `recursos` e app que não manda `recursos` numa sala sem palco: tudo como hoje;
- os testes de hoje passam sem mudança.

**Documentação do servidor**: `README.md` (o que mudou visível) e `BACKLOG.md`.

**Ordem**: o servidor vai para produção **antes** do app 3.6.0 (é aditivo: app antigo não percebe
nada, e só a 3.6 cria sala em palco).

## 8. Testes de aceite do marco

O marco está pronto quando:
1. todos os aceites de F1 a F9 passam;
2. a verificação do `CLAUDE.md` passa (fmt, clippy `-D warnings`, `cargo test --workspace`,
   `cargo check --workspace --all-targets` no Windows; `npm run check` e `npm test`);
3. `online_aceite_2` e `aceite_camadas_local` continuam passando (o palco não pode mexer no
   balanceamento);
4. o servidor de produção tem a seção 7 no ar e o teste de compatibilidade dele passa;
5. o roteiro abaixo passa com duas pessoas, num `v3.6.0-beta.1`.

### Roteiro manual com duas pessoas

A = Heitor (dono, com fone), B = um amigo (notebook com caixa de som). Os dois com a
`v3.6.0-beta.1`. C = um terceiro app na 3.5.1 (o portátil serve) para o passo 12.

Local (Radmin):
1. A cria sala Local com **Modo palco**, compartilhando a tela. B vê o selo PALCO na lista e entra.
   Esperado: B vê a tela; câmera e Compartilhar de B desligados com a explicação; botão Levantar a
   mão.
2. A liga o microfone e fala. B ouve; no painel Pessoas de B, A tem o anel de "falando". B põe a
   **Voz** de A em 30%: a voz baixa e o som da tela não muda. B fecha a tela de A: a voz continua.
3. A aperta **V**: em até 1 s B vê o microfone de A riscado e não ouve nada; A aperta V de novo.
4. B levanta a mão. A recebe "B quer subir ao palco" e o selo na aba Pessoas. A recusa: B vê "O dono
   recusou seu pedido" e o botão mostra a contagem de 30 s.
5. Passados 30 s, B levanta e A aceita: B vê "Você subiu ao palco" e os botões ligam. B liga o
   microfone (cancelamento ligado, caixa de som) e A fala: A **não** pode ouvir a própria voz de
   volta. B desliga o cancelamento: aparece o aviso de sem fone, e A passa a ouvir eco. B religa.
6. B compartilha a tela: os dois transmitem ao mesmo tempo e os dois microfones funcionam.
7. A tira B do palco: em até 1 s a tela e o microfone de B param (a luz de microfone do Windows
   apaga no notebook) e B vê "Você voltou para a plateia".
8. Música: B não tem o campo de link e vê "No palco, a música fica com os anfitriões". A liga
   **Plateia pode pôr música**: B adiciona um vídeo e vota. A desliga: o voto de B some.
9. A desliga o palco: o microfone de A para; B consegue compartilhar. A liga de novo: a transmissão
   de B para na hora.
10. A promove B, e B tira o cabo de rede (ou desliga o Radmin) por 20 s: B volta anfitrião e a tela
    dele volta sozinha.
11. B (anfitrião) fecha o app e abre depois de 2 min: volta como plateia.
12. C (3.5.1) tenta entrar na sala em palco: "Recusado pela sala: atualizar_app". A desliga o
    palco, C entra. A liga: a confirmação avisa que C vai sair; C sai com "Sala encerrada
    (atualizar_app)".

Online (servidor de produção já atualizado): repetir 1, 2, 4, 5, 7, 8 e 12 (no 12, C vê a mensagem
"Esta sala está no modo palco. Atualize…"). Com um servidor sem o recurso (o `zap-test-server` de
uma versão anterior ou um servidor próprio antigo), a opção Modo palco aparece desligada com "O
servidor ainda não tem o modo palco".

Microfones para o passo 5 (se der): o do notebook, um fone USB e o microfone de uma webcam. Anotar
em quais o cancelamento abre (`MicView.processing`) e em quais cai para a captura direta.

## 9. Riscos

| Risco | Mitigação |
|---|---|
| O Voice Capture DSP recusa alguns dispositivos (matriz de microfones, 44,1 kHz, fone USB) ou o índice de `DEVICE_INDEXES` não bate com a coleção | Reserva automática na captura direta com aviso; teste manual com três microfones (§8); o índice é calculado na hora de abrir, da mesma coleção (`EnumAudioEndpoints(eCapture, ACTIVE)`) |
| O cancelamento de eco do DSP é fraco com jogo alto e dois falando ao mesmo tempo | Aviso de sem fone; o "Cancelar eco e ruído" é desligável; WebRTC APM fica anotado para a v3.7 se o teste mostrar eco |
| `WMAAECMA_E_NO_ACTIVE_RENDER_STREAM` sem saída de áudio | Bloco de silêncio e segue; o `AudioOut` do app toca silêncio e mantém a saída ativa |
| Microfone bloqueado pela privacidade do Windows | `AudioError::MicBlocked` com o caminho para liberar |
| App antigo expulso quando o palco liga (D2) | Confirmação lista quem vai sair; o atualizador instala a 3.6 fora da sala; a alternativa de D2 é barata de trocar |
| App 3.6 lançado antes do servidor | `recursos` no `/v1/info`: sem `"palco"`, a opção fica desligada. Ordem do §7: servidor primeiro |
| Corrida: virar plateia no meio do `trilha.publicar` | `proibido` + o `start` limpa o que criou; `stop_local` nunca manda `trilha.remover` |
| Medidor de nível a 4 por segundo parece lento | Aceito na v3.6.0 (o snapshot já sai nesse ritmo); um canal próprio de nível fica para depois |
| A mudança no `AudioRegistry` (chave por fonte) quebra o som que já funciona | O som de apps continua com a chave antiga (`participant_id`); os testes de áudio de hoje continuam e a F7 acrescenta os de duas fontes |
| O `ajustar_audio` do Local passa a ligar duas trilhas | Teste de contagem de pacotes por trilha (Aceite F7) |
| Spam de mão | 5 s entre levantadas, 30 s depois de recusado |
| `musica.info` com `erro: true` de alguém da plateia pula a música | Já vale hoje para todos; fica igual (D3c) |
| Volta em 60 s no Local pela `instalacao_id` | A `instalacao_id` não é segredo; quem forja precisa de acesso à sala (senha) e só ganha o papel que a pessoa tinha, por 60 s |

## 10. Fora da v3.6.0

- Microfone fora do palco e para a plateia ("falar sem subir").
- Aperte para falar; ajuste de ganho, de limiar de ruído e de AGC.
- Cancelamento de eco do WebRTC (APM) ou o `IAcousticEchoCancellationControl` do Windows 11.
- Testar o microfone em Configurações (o medidor do menu do microfone, na sala, cobre).
- Expulsar e banir pessoas; passar a sala para outro dono; anfitriões promovendo outros.
- Ordem da fila de pedidos mudada pelo dono; limite de anfitriões além dos limites de hoje (16
  pessoas e 8 trilhas de vídeo por sala no Online).
- Canal próprio para o medidor de nível (mais de 4 por segundo).
- Microfone no Linux (só o sintético dos testes).

## 11. Ordem de lançamento

1. F1 em commit no `master` do app e no GitHub. Avisar a sessão do servidor com o hash.
2. Em paralelo: servidor (§7) contra o `rev` da F1; F2 a F9 no app.
3. Servidor em produção (aditivo).
4. `v3.6.0-beta.1` (canal beta) e o roteiro do §8 contra a produção.
5. Ajustes; `v3.6.0`. Atualizar `docs/ESTADO.md` e a versão nos 5 lugares (ver o topo do ESTADO).

## 12. Frentes para o `zap-marco`

```json
[
  { "id": "contrato", "crate": "zap-proto", "resumo": "Contrato do palco em docs/api (mensagens.md, rest.md, README.md) e no zap-proto: AudioSource, recursos, fonte/mudo, palco.* e o módulo puro zap_proto::stage; ajuste mecânico dos literais nos outros crates; teste compat_v35", "depende_de": [] },
  { "id": "net", "crate": "zap-net", "resumo": "Hello.features, accept_handshake_gated (recusa atualizar_app), recursos/palco nos tipos REST do cliente", "depende_de": ["contrato"] },
  { "id": "audio", "crate": "zap-audio", "resumo": "Microfone no Windows: lista, Voice Capture DSP com reserva na captura direta, forma da saída, Upsampler3, PeakHold, OpusVoiceEnc", "depende_de": [] },
  { "id": "media", "crate": "zap-media", "resumo": "MicPipeline (captura, Opus de voz 48 kbps, mudo, reconfigurar, nível) e o sintético", "depende_de": ["audio"] },
  { "id": "servidor-teste", "crate": "zap-test-server", "resumo": "Palco no servidor de teste igual à seção 7 (regras, ordem, 426, expulsão, microfone, audio:true sem o microfone, volta em 60 s)", "depende_de": ["contrato", "net"] },
  { "id": "nucleo-palco", "crate": "zap-core", "resumo": "Autoridade do palco no dono Local, espelho no Online, conferência do app, StageView, comandos do Core, e parar a minha parte quando o outro lado remove minha trilha", "depende_de": ["contrato", "net", "servidor-teste"] },
  { "id": "nucleo-mic", "crate": "zap-core", "resumo": "Microfone do anfitrião (ShareSession, AudioPublisher, ajustar_audio por trilha), receptor por fonte, volume da voz, falando, MicView, configurações", "depende_de": ["nucleo-palco", "media"] },
  { "id": "tauri", "crate": "app/src-tauri", "resumo": "Os 10 comandos novos de palco e microfone", "depende_de": ["nucleo-mic"] },
  { "id": "interface", "crate": "app/src", "resumo": "Tipos e mock; palco em Criar sala, dock (microfone, mão), Pessoas (pedidos, anfitriões, voz), música trancada, selos, avisos e Configurações → Som", "depende_de": ["contrato"] }
]
```

Dependências em resumo: `contrato` e `audio` começam juntos; `net` depois de `contrato`; `media`
depois de `audio`; `servidor-teste` depois de `net`; `nucleo-palco` depois de `servidor-teste`;
`nucleo-mic` depois de `nucleo-palco` e `media`; `tauri` depois de `nucleo-mic`; `interface` anda a
partir do `contrato`, contra o mock, e é ligada ao núcleo no fechamento. O servidor de produção
(§7) não é frente do `zap-marco`: começa quando o `contrato` estiver no GitHub.
