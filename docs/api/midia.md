# Mídia: conexão QUIC `zap/4`

Toda mídia (vídeo, áudio) anda em QUIC com ALPN **`zap/4`**, com estes papéis:

| Modo | Lado "cliente" QUIC | Lado "servidor" QUIC |
|---|---|---|
| Local | quem assiste | quem publica (porta UDP 47800, ou a próxima livre) |
| Online, ponte | o app (publica e/ou assiste na mesma conexão) | servidor streamaZap (UDP 7443) |
| Online, direta | quem assiste | quem publica |

Uma conexão leva **todas** as trilhas entre os dois lados, nos dois sentidos. No modo Online, cada
participante tem uma conexão de mídia com o servidor: uma conexão nova substitui a antiga. O
servidor manda `ping` de controle a cada 2 s nessa conexão.

**Queda da conexão de mídia (Online, ponte).** Quando a conexão QUIC de um participante fecha ou
morre, o servidor espera **5 s** antes de tirar as trilhas e as assinaturas dele da sala. Se ele
reconectar à mesma sala nesse prazo, as trilhas continuam (nada de `trilha.removida`) e a conexão
nova recebe de novo `camadas.pedidas` e `camada.atual`. Sair da sala (`sala.sair`) tira as trilhas
na hora.

## Parâmetros de transporte

| Parâmetro | Valor |
|---|---|
| `max_idle_timeout` | 10 s (com `ping` de controle a cada 1 s: queda detectada em ~5 s) |
| `initial_max_streams_uni` | ≥ 1024 |
| Datagramas | obrigatórios (`max_datagram_frame_size` ≥ 1200) |
| MTU inicial | 1200 (Radmin e VPNs), descoberta de MTU ligada |
| Controle de congestionamento | Cubic (o app usa Cubic; o BBR do quinn 0.11 ainda enche a fila do gargalo). Quem manda na taxa é o estimador de banda do app |
| Ordem entre streams | FIFO (sem rodízio entre streams): o quadro mais velho termina primeiro |
| RTT inicial | 100 ms |

## Streams e datagramas

```
stream bidirecional aberto primeiro pelo cliente  → CONTROLE (mensagens.md, prefixo u32 + JSON)
stream unidirecional (um por quadro de vídeo)     → QUADRO
datagrama tipo 0x02                               → ÁUDIO (um pacote Opus de 20 ms)
datagrama tipo 0x03                               → SONDA (enchimento para medir banda)
```

Todos os inteiros são **big-endian**.

### Quadro de vídeo (stream unidirecional)

Cabeçalho de 30 bytes + um *access unit* H.264 completo em Annex B (com start codes):

| Offset | Tam. | Campo | Descrição |
|---|---|---|---|
| 0 | 1 | `tipo` | `0x01` |
| 1 | 1 | `flags` | bit0 = keyframe (IDR); bit1 = traz SPS/PPS; demais = 0 |
| 2 | 2 | `trilha` | `Trilha.numero` (u16, único na sala) |
| 4 | 1 | `camada` | 0, 1 ou 2 |
| 5 | 1 | reservado | 0 |
| 6 | 2 | `largura` | pixels |
| 8 | 2 | `altura` | pixels |
| 10 | 4 | `seq` | u32 por (trilha, camada), +1 por quadro, começa em 0 |
| 14 | 8 | `captura_us` | u64, relógio monotônico de **quem capturou** (atravessa a ponte sem mudar) |
| 22 | 4 | `envio_us` | u32, relógio de **quem envia neste salto**, em µs (dá a volta; só diferenças importam). A ponte reescreve |
| 26 | 4 | `tamanho` | bytes do H.264 a seguir |
| 30 | … | H.264 | Annex B, perfil High ou Main, sem B-frames |

Regras:
- Todo keyframe traz SPS/PPS (`flags` = `0x03`).
- **Prazo**: quem envia reseta o stream (código `0x10`, "atrasado") se o quadro não foi
  **confirmado pelo outro lado** (todos os bytes com ACK) em `max(150 ms, 2 × RTT)` desde que
  entrou na fila. Antes de abrir o stream, quem envia também pode descartar o quadro na hora
  (admissão) se a fila à frente não esvazia dentro do prazo. Quem recebe descarta streams
  resetados ou incompletos; o `seq` que faltou vai em `perdidos`.
- Depois de um quadro perdido, a camada só volta a ser decodificável no próximo keyframe. Quem
  envia já sabe (resetou) e manda keyframe; quem recebe, se ficar 300 ms sem quadro decodificável,
  manda `keyframe.pedir`.
- Troca de camada para um espectador acontece **sempre num keyframe** da camada nova.

### Áudio (datagrama)

| Offset | Tam. | Campo |
|---|---|---|
| 0 | 1 | `tipo` = `0x02` |
| 1 | 1 | `flags` = 0 |
| 2 | 2 | `trilha` |
| 4 | 4 | `seq` (u32, +1 a cada 20 ms) |
| 8 | 8 | `captura_us` |
| 16 | 4 | `envio_us` |
| 20 | … | pacote Opus (48 kHz, 20 ms, FEC em banda ligado) |

Perdas são cobertas pelo FEC do Opus (pacote seguinte) e pelo PLC do decodificador.

### Sonda (datagrama)

| Offset | Tam. | Campo |
|---|---|---|
| 0 | 1 | `tipo` = `0x03` |
| 1 | 1 | 0 |
| 2 | 2 | `cluster` (u16, id da rajada) |
| 4 | 4 | `seq` |
| 8 | 4 | `envio_us` |
| 12 | … | zeros até o tamanho do datagrama |

Quem roda o alocador para um espectador (a ponte, ou o publicador no Local/direto) manda rajadas
de 200 ms para ele; quando e em que taxa está em [`balanceamento.md`](balanceamento.md#4-sondagem).
Quem recebe registra a chegada (vai em `relatorio.sondas`) e o app mede a taxa do grupo para a
própria `banda_kbps`.

## Relatório do receptor (`relatorio`)

Mensagem de controle mandada por **quem recebe** mídia neste salto, a cada 100 ms (e na hora,
quando o estado vira `sobrecarga`):

```json
{
  "t": "relatorio",
  "chegadas": [[2, 0, 1841, 3021440, 99812345, 21844], [2, 0, 1842, 3038110, 99829101, 9120]],
  "sondas": [[7, 0, 3101000, 99830000, 1200]],
  "perdidos": [[2, 0, 1840]],
  "perda": 0.004,
  "banda_kbps": 5400,
  "estado": "normal",
  "buffer_ms": 120
}
```

| Campo | Obrigatório | Descrição |
|---|---|---|
| `chegadas` | sim | `[trilha, camada, seq, envio_us, chegada_us, bytes]` de cada quadro (último byte) e pacote de áudio (`camada` = 0) recebidos desde o último relatório. `chegada_us` é o relógio de quem recebe |
| `sondas` | se houve | `[cluster, seq, envio_us, chegada_us, bytes]` |
| `perdidos` | sim | quadros resetados/incompletos |
| `perda` | sim | fração de **datagramas de áudio** perdidos na rede nos últimos 2 s (0–1), por buraco de `seq`. Quadros de vídeo descartados por quem envia **não** contam (vão em `perdidos`) |
| `banda_kbps` | app: sim; servidor: não | estimativa de banda **de recepção** feita por quem recebe. Leva em conta atraso (tendência e fila persistente) e quadros descartados pelo emissor (buracos de `seq`): com ≥ 3 por segundo, vai para 0,95 × a vazão que chega |
| `estado` | app: sim; servidor: não | `sobrecarga` \| `normal` \| `ociosa` (detector de atraso) |
| `buffer_ms` | app: sim | suavização atual de quem assiste |

Como isso divide o trabalho:
- **Descida (servidor → espectador)**: o app que assiste roda o estimador e manda `banda_kbps`
  pronto. O servidor só precisa usar o número (ver [`balanceamento.md`](balanceamento.md)).
- **Subida (publicador → servidor)**: o servidor só registra chegadas e manda `chegadas` e `perda`
  ao publicador, que roda o mesmo estimador do lado dele. O servidor **não precisa** implementar
  estimador de banda. O `relatorio` do servidor só sai quando houve chegadas, sondas ou perdas, e
  não traz `banda_kbps` nem `estado`.
- **Sondas**: a ponte manda sondas para quem assiste abaixo do teto
  ([`balanceamento.md`](balanceamento.md#4-sondagem)); é o único jeito de a `banda_kbps` de quem
  recebe uma camada barata subir até a camada seguinte. O publicador não sonda a própria subida
  pela ponte (no modo ponte ele codifica o que `camadas.pedidas` manda).

## Ponte (servidor) — o que repassar

1. Recebe cada camada de cada trilha uma vez do publicador.
2. Para cada espectador e trilha assinada, escolhe a camada (algoritmo em
   [`balanceamento.md`](balanceamento.md)) e repassa os quadros dela **sem alterar** nada além de
   `envio_us`. Áudio idem.
3. Mantém por espectador a regra de prazo (reset de quadro atrasado) e a troca de camada só em
   keyframe.
4. Pede ao publicador só as camadas em uso (`camadas.pedidas`) e repassa `keyframe.pedir`
   respeitando 1 por segundo por camada.
5. Mantém o relógio `captura_us` intacto: é ele que sincroniza áudio e vídeo no espectador.
