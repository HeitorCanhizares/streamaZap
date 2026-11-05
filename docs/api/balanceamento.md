# Balanceamento: qualidade individual por espectador

Objetivo: **cada pessoa recebe a melhor qualidade que a conexão dela aguenta, nunca acima da
qualidade escolhida por quem transmite**, sem que um espectador ruim piore os outros.

Quem roda este algoritmo:
- **Online com ponte**: o servidor, para cada trilha de vídeo da sala (descida), e o publicador
  só ajusta a própria subida.
- **Local / Online direto**: o próprio publicador (ele é o "servidor" das próprias trilhas),
  com uma restrição a mais: o upload dele é dividido entre todos os espectadores.

A implementação de referência está em `crates/zap-rate` (`Allocator`, `Ladder`). Os números
abaixo são os mesmos de lá; mudar um aqui exige mudar lá.

## 1. Escada de camadas

Quem publica anuncia a escada em `trilha.publicar`. **A camada 0 é sempre a qualidade escolhida
por quem transmite.** Escada padrão do app:

| Escolha de quem transmite | Camada 0 | Camada 1 | Camada 2 |
|---|---|---|---|
| 1080p60 | 1920×1080 · 60 fps · 8000 kbps | 1280×720 · 30 · 2500 | 640×360 · 15 · 400 |
| 1080p30 | 1920×1080 · 30 · 5000 | 1280×720 · 30 · 2500 | 640×360 · 15 · 400 |
| 720p60 | 1280×720 · 60 · 4000 | 960×540 · 30 · 1200 | 640×360 · 15 · 400 |
| 720p30 | 1280×720 · 30 · 2500 | 960×540 · 30 · 1200 | 640×360 · 15 · 400 |
| Câmera | 1280×720 · 30 · 1500 | — | 640×360 · 15 · 300 |

(Largura real segue a proporção da tela/janela; a tabela supõe 16:9.)

Cada camada pode ser codificada entre **50% e 100%** do seu `kbps` nominal (`piso` e `teto`).

## 2. Entradas por espectador

| Entrada | De onde vem |
|---|---|
| `banda_kbps` | `relatorio.banda_kbps` do espectador (descida); no modo direto, a estimativa do próprio publicador para aquele enlace |
| `estado` | `relatorio.estado` (`sobrecarga` derruba na hora) |
| `teto_pedido` | `assinar.camada_max` (ausente = 0) |
| `teto_tamanho` | `assinar.tamanho`: miniatura → 2; principal/cheia → 0 |
| `audio_kbps` | soma das trilhas de áudio que ele recebe (reserva, sai da banda antes do vídeo) |

`teto(e) = max(teto_pedido, teto_tamanho)` (número maior = camada menor).

## 3. Algoritmo (a cada 100 ms)

```
para cada trilha de vídeo T:
  para cada espectador e de T:
     disponivel(e) = banda_kbps(e) × 0,9 − audio_kbps(e) − (vídeo de outras trilhas que e recebe)
  -- 1) começa todo mundo na camada mais barata que precisa receber
  alvo(e) = 2   (ou a única camada que existir)
  -- 2) sobe um degrau por vez, em rodízio (justiça max-min)
  repetir até nenhuma subida acontecer:
     para cada e em ordem de menor camada recebida primeiro:
        c = alvo(e) − 1
        se c < teto(e): continue           -- já está no teto dele
        se disponivel(e) < 1,15 × kbps(c): continue
        se modo_direto e upload_usado + kbps(c) − kbps(alvo(e)) > upload_publicador × 0,9: continue
        alvo(e) = c
  -- 3) histerese (por espectador)
  se alvo(e) < atual(e)  (subir):  só aplica se a condição de subida vale há ≥ 3 s
                                   e a última subida de e foi há ≥ 5 s
  se alvo(e) > atual(e)  (descer): aplica na hora se estado(e) = sobrecarga,
                                   ou se disponivel(e) < 0,95 × kbps(atual) há ≥ 500 ms
  -- 4) troca efetiva no próximo keyframe da camada nova (pede um: máx. 1/s por camada)
```

Depois, para o publicador:

```
camadas_em_uso = { atual(e) e alvo(e) pendente, de todos os espectadores }
para cada camada c em uso:
   kbps_pedido(c) = clamp( min(disponivel(e) para e em c) / 1,15 , piso(c), teto(c) )
enviar camadas.pedidas { trilha_id, camadas: [{camada: c, kbps: kbps_pedido(c)}] }
   -- só quando mudar mais de 10% ou mudar o conjunto; no máximo 2 vezes por segundo
```

Camada sem ninguém = fora da lista = o publicador para de codificar (GPU livre).

## 4. Sondagem

**Por que existe.** O estimador do app só mede o que chega. Quem recebe pouco vídeo (camada 2,
tela parada) fica com `banda_kbps` perto de 2× o que recebe: o estimador não sobe a estimativa
quando a vazão está abaixo da metade dela (fonte limitada). Sem sondas, quem começa na camada 2
**nunca** sai dela. Por isso quem envia para o espectador manda **sondas** (datagramas `0x03`,
formato em [`midia.md`](midia.md#sonda-datagrama)) e o app adota a taxa que mediu.

**Quem sonda.** Quem roda o alocador para aquele espectador: a ponte (Online com ponte); o
publicador (Local e Online direto). Um espectador = uma conexão QUIC = um estado de sondagem
(se ele recebe várias trilhas pela mesma conexão, as sondas são uma só).

**Quando** (avaliado a cada passo de 100 ms, por espectador `e`):
1. Existe trilha `T` que `e` recebe com `atual(e,T) > teto(e,T)` (há camada melhor permitida) e
   sem troca pendente. `c(T)` = a camada existente logo acima de `atual(e,T)` (camera sem camada 1:
   de 2 vai para 0). Se houver mais de uma trilha assim, vale a de menor `kbps(c)`.
2. Já chegou relatório de `e` com `banda_kbps > 0`.
3. Passou 1 s desde que a condição 1 começou a valer (ou mudou de `c`), e já chegou a hora da
   próxima rajada para `e`: `fim da última rajada + intervalo(e)` (ou `fim + 1 s` depois de um
   **degrau**, ver "Resultado"). `intervalo(e)` começa em 5 s.
4. Nenhuma rajada para `e` está em andamento ou esperando resultado.

**Quanto.** A rajada dura **200 ms**. Durante ela, a taxa total que chega a `e` (o que já vai para
ele + as sondas) deve ser `alvo(e)`, que sobe **em degraus** guiados pela última medida:

```
alvo_camada(e) = 1,5 × (kbps(c) + reserva(e))     -- reserva = §3: áudio + vídeo das outras trilhas
teto(e)        = 1,5 × M(e)    se a última medida M(e) (de até 60 s atrás) não saturou
               = 1,25 × M(e)   se ela saturou
               = sem teto      se não há medida de e
alvo(e)        = min(alvo_camada(e), teto(e))
sonda(e)       = alvo(e) − enviado(e)             -- não sonda se sonda(e) < 100 kbps
```

`enviado(e)` é a taxa que já sai para `e`, medida no último 1 s (bytes UDP da conexão; no
`quinn`, `Connection::stats().udp_tx.bytes`). Sem essa medida, use o piso `0,5 × kbps(atual)`.

`M(e)` é a taxa medida do último grupo de sondas de `e` (ver "Medida"); **saturou** quando
`M(e) < 0,85 × alvo(e)` daquela rajada (o caminho não entregou o que recebeu). A medida vale para
o caminho, não para a camada: trocar `c` ou o teto não apaga `M(e)`.

Por que 1,5 × (kbps + reserva): a camada `c` só sobe com `banda × 0,9 − reserva ≥ 1,15 × kbps(c)`,
ou seja, `banda ≥ 1,28 × kbps(c) + 1,11 × reserva`; o app adota 0,95 × a taxa medida, então a
rajada precisa chegar a `≥ 1,35 × kbps(c) + 1,17 × reserva`. O 1,5 deixa ~10% para erro de medida.

Por que em degraus: uma rajada acima da capacidade do gargalo enche a fila dele e o excesso é
descartado **junto com o vídeo** (que vai em streams do QUIC: retransmissão, quadros resetados
por prazo, janela do controle de congestionamento cortada). Medido no M4c: no enlace de teste de
4 Mbps com fila de 100 ms, a rajada de 12000 kbps (alvo antigo da 1 para a 0) perdeu ~70 pacotes
e fez resetar 16 quadros de quem estava estável na camada 1, que caiu para a 2. Um caminho que
entregou `M` sem saturar tem capacidade `≥ M`; uma rajada de `1,5 × M` por 200 ms põe na fila no
máximo `(1,5 M − M) × 0,2 s = 0,1 s × M`, que cabe numa fila de 100 ms. Depois de saturar, a
capacidade é `≈ M` e a tentativa seguinte (`1,25 × M`) põe no máximo 50 ms na fila: só confere se
a rota melhorou.

Exemplo (escada 1080p60, sem áudio): da 2 para a 1, primeira rajada sem medida,
`alvo = 1,5 × 2500 = 3750` (sonda ≈ 3750 − 400 = 3350). Quem tem 4 Mbps: `M = 3750` (não saturou),
sobe para a 1; da 1 para a 0, `alvo = min(12000, 1,5 × 3750) = 5625`, mede `M ≈ 4000` (saturou:
4000 < 0,85 × 5625), falha; a seguinte, 10 s depois, usa `1,25 × 4000 = 5000`. Quem não tem limite:
3750 → 5625 → 8437 → 12000, três degraus com 1 s entre eles, e sobe para a 0.

**Como enviar.** Datagramas de `min(1200, maior datagrama da conexão)` bytes. Ritmo por crédito,
não por relógio fixo: a cada tique (alvo 5 ms; vale o tempo real decorrido),
`crédito += sonda(e) × dt` e saem `⌊crédito / tamanho⌋` datagramas. Todos com o mesmo `cluster`
(u16, +1 por rajada na conexão, dá a volta) e `seq` de 0 em diante.

**Medida.** Quem sonda mede o grupo com o próprio `relatorio` de `e` (`sondas` e `chegadas`; os
instantes de chegada estão todos no relógio de `e`), pela mesma regra do app (abaixo): bytes de
**todos** os pacotes (vídeo, áudio e sonda) que chegaram depois da primeira sonda do `cluster` e
até a última, divididos pelo tempo entre as duas; vale com ≥ 5 sondas e ≥ 100 ms. O grupo fecha
quando aparece outro `cluster`, quando um relatório chega sem sondas, ou 50 ms (no relógio de `e`)
depois da última sonda. A referência é `zap-net::probe::Prober::on_report`.

**Resultado** (avaliado até 1 s depois do fim da rajada, na ordem):
1. **Sucesso**: chegou relatório de `e` com `banda_kbps × 0,9 − reserva ≥ 1,15 × kbps(c)`.
   `intervalo(e)` volta a 5 s e a subida segue o passo 3 (histerese de 3 s).
2. **Degrau**: não houve sucesso, mas a medida desta rajada chegou, **não saturou** e a rajada foi
   limitada pelo teto (`alvo(e) < alvo_camada(e)`). A próxima rajada sai 1 s depois do fim desta
   (com o teto novo, `1,5 × M`); `intervalo(e)` não muda.
3. **Falha**: qualquer outro caso (saturou, a medida não chegou, ou a rajada já era a da camada e
   não bastou). `intervalo(e) = min(2 × intervalo(e), 60 s)`.

Mudar `atual(e,T)`, `c(T)` ou o teto (novo `assinar`) volta `intervalo(e)` a 5 s.

**O que o app faz ao receber** (a ponte e o publicador não precisam implementar, só saber):
- agrupa as sondas pelo `cluster` e mede a taxa do grupo como em "Medida";
- se a perda (`relatorio.perda`) está ≤ 10%: `banda_kbps = max(banda_kbps, 0,95 × taxa do grupo)`,
  já no próximo relatório;
- **carência de rajada própria**: rajadas que o próprio sistema manda (sondas e keyframes) criam
  fila de propósito. Ela abre quando chega uma sonda, quando chega o cabeçalho de um keyframe e
  quando um keyframe termina de chegar, e fecha quando (a) passaram 300 ms da última sonda (para
  keyframe não há esse mínimo) **e** a fila escoou (atraso do último grupo ≤ menor atraso dos
  últimos 10 s + 30 ms), ou (b) passou 1 s do último desses eventos, o que vier antes. Durante
  ela o atraso não derruba a estimativa nem vira `estado = sobrecarga`, e quadros descartados por
  quem envia cujo buraco de `seq` **foi visto** dentro dela não contam como descarte. Fila que
  sobra depois de 1 s é congestionamento de verdade e volta a contar. Uma sobrecarga do detector
  que **começou** dentro da carência continua sendo da rajada depois que a fila escoa, até 1 s do
  último evento de rajada e até 1 s do início dela: a fila esvazia antes de a tendência do atraso
  voltar ao normal, e esse resto cortava a estimativa logo depois de cada keyframe;
- **troca de camada não é descarte**: o `seq` de uma camada anda enquanto o espectador recebe
  outra. Ao voltar para ela, o salto de `seq` não vai em `perdidos` nem conta como quadro
  descartado (só buracos entre quadros seguidos da mesma camada contam).

Por que o keyframe também: num enlace com pouca folga (camada 1, 2500 kbps, num enlace de 4 Mbps)
o keyframe (até ~8 × o quadro médio, ~83 KB) leva ~170 ms para passar e a fila leva ~0,5 s para
esvaziar. Sem carência o estimador chama isso de sobrecarga, a estimativa cai para
`0,85 × vazão ≈ 0,85 × 2500` e o espectador desce uma camada a cada keyframe (medido no M4c; ver
também o prazo do keyframe em [`midia.md`](midia.md#quadro-de-vídeo-stream-unidirecional)).

Uma rajada acima da capacidade ainda custa alguns quadros atrasados; os degraus limitam o excesso
a uma fila de ~100 ms e o `intervalo(e)` crescente (5 → 60 s) limita a frequência a quem de fato
não aguenta a camada seguinte.

## 5. Exemplos

**Sala com 3 espectadores, Marcão transmitindo 1080p60 pela ponte:**

| Espectador | banda_kbps | Tamanho | Camada recebida |
|---|---|---|---|
| Ju (fibra) | 30000 | principal | 0 (1080p60, 8 Mbps) |
| Rafa (4G) | 3400 | principal | 1 (720p30, 2,5 Mbps) |
| Dudu | 20000 | miniatura | 2 (360p15) — está vendo outra pessoa em destaque |

O publicador recebe `camadas.pedidas` com as camadas 0, 1 e 2. Se Dudu clicar na miniatura,
`assinar` com `tamanho: "principal"` e ele sobe para 1 e depois 0 (histerese de 3 s).

**Modo Local, upload do publicador 10 Mbps (orçamento 9000 kbps), espectadores com banda folgada:**

- 3 espectadores: começam na 2 (3 × 400 = 1200). O rodízio sobe os três para a 1
  (3 × 2500 = 7500). Subir qualquer um para a 0 custaria +5500 (13000 > 9000): ninguém sobe.
  Resultado: os três em 720p30.
- 2 espectadores: os dois sobem para a 1 (5000); subir um para a 0 daria 10500 > 9000.
  Resultado: os dois em 720p30. O rodízio prefere todos iguais a um com 1080p e outro sem nada.
- 1 espectador: sobe até a 0 (8000 ≤ 9000). Resultado: 1080p60.

## 6. Requisitos mínimos para um servidor

Um servidor que implemente só isto já fica compatível:
1. Usar `banda_kbps` e `estado` dos relatórios dos espectadores.
2. Seguir o passo 3 com os números acima (ou mais conservador).
3. Mandar `camadas.pedidas` ao publicador e `camada.atual` ao espectador.
4. Trocar de camada só em keyframe.
5. Sondar (§4) quem está abaixo do teto, **em degraus** (teto `1,5 × M` / `1,25 × M` depois de
   saturar) e com a medida tirada do relatório do espectador. Sem sondas o servidor funciona, mas
   ninguém que assiste pela ponte sai da camada em que começou (a 2); sem os degraus, quem está
   estável numa camada com pouca folga cai dela a cada sondagem.
6. Aplicar o prazo de keyframe e a extensão do prazo dos quadros atrás dele
   ([`midia.md`](midia.md#quadro-de-vídeo-stream-unidirecional)). Sem isso o keyframe da troca de
   camada é resetado em enlaces com pouca folga e quem assiste nunca consegue decodificar a camada
   nova.

A implementação de referência das sondas está em `crates/zap-rate/src/probe.rs` (decisão, degraus
e ritmo, sem I/O) e `crates/zap-net/src/probe.rs` (envio e medida pelo relatório); o servidor de
teste (`crates/zap-test-server`) as usa na ponte. O prazo está em `crates/zap-net/src/media_tx`.
