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
3. Passou 1 s desde que a condição 1 começou a valer (ou mudou de `c`), e passou `intervalo(e)`
   desde o fim da última rajada para `e`. `intervalo(e)` começa em 5 s.
4. Nenhuma rajada para `e` está em andamento ou esperando resultado.

**Quanto.** A rajada dura **200 ms**. Durante ela, a taxa total que chega a `e` (o que já vai para
ele + as sondas) deve ser

```
alvo(e)  = 1,5 × (kbps(c) + reserva(e))          -- reserva = §3: áudio + vídeo das outras trilhas
sonda(e) = alvo(e) − enviado(e)                   -- não sonda se sonda(e) < 100 kbps
```

`enviado(e)` é a taxa que já sai para `e`, medida no último 1 s (bytes UDP da conexão; no
`quinn`, `Connection::stats().udp_tx.bytes`). Sem essa medida, use o piso `0,5 × kbps(atual)`.

Por que 1,5: a camada `c` só sobe com `banda × 0,9 − reserva ≥ 1,15 × kbps(c)`, ou seja,
`banda ≥ 1,28 × kbps(c) + 1,11 × reserva`; o app adota 0,95 × a taxa medida, então a rajada
precisa chegar a `≥ 1,35 × kbps(c) + 1,17 × reserva`. O 1,5 deixa ~10% para erro de medida.
Exemplo (escada 1080p60, sem áudio): da 2 para a 1, `alvo = 1,5 × 2500 = 3750 kbps`
(sonda ≈ 3750 − 400 = 3350); da 1 para a 0, `alvo = 12000` (sonda ≈ 9500).

**Como enviar.** Datagramas de `min(1200, maior datagrama da conexão)` bytes. Ritmo por crédito,
não por relógio fixo: a cada tique (alvo 5 ms; vale o tempo real decorrido),
`crédito += sonda(e) × dt` e saem `⌊crédito / tamanho⌋` datagramas. Todos com o mesmo `cluster`
(u16, +1 por rajada na conexão, dá a volta) e `seq` de 0 em diante.

**Resultado.** Sucesso se, até 1 s depois do fim da rajada, chegar relatório de `e` com
`banda_kbps × 0,9 − reserva ≥ 1,15 × kbps(c)`: `intervalo(e)` volta a 5 s e a subida segue o passo 3
(histerese de 3 s). Senão, falhou (a rota não aguenta): `intervalo(e) = min(2 × intervalo(e), 60 s)`.
Mudar `atual(e,T)`, `c(T)` ou o teto (novo `assinar`) volta `intervalo(e)` a 5 s.

**O que o app faz ao receber** (a ponte e o publicador não precisam implementar, só saber):
- agrupa as sondas pelo `cluster`; o grupo fecha quando chega outro `cluster` ou 50 ms sem sonda;
- taxa do grupo = bytes de **todos** os pacotes (vídeo, áudio e sonda) que chegaram depois da
  primeira sonda e até a última, divididos pelo tempo entre as duas; o grupo só vale com ≥ 5
  sondas e ≥ 100 ms;
- se a perda (`relatorio.perda`) está ≤ 10%: `banda_kbps = max(banda_kbps, 0,95 × taxa do grupo)`,
  já no próximo relatório;
- enquanto chegam sondas e até 300 ms depois da última, o atraso que a própria rajada cria não
  derruba a estimativa nem vira `estado = sobrecarga`, e quadros descartados por quem envia não
  contam como descarte (a fila é da sonda; se a rota não aguenta, a taxa medida já diz isso).

Uma rajada acima da capacidade custa algumas perdas de áudio e quadros atrasados por ~200 ms; o
`intervalo(e)` crescente (5 → 60 s) limita isso a quem de fato não aguenta a camada seguinte.

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
5. Sondar (§4) quem está abaixo do teto. Sem isso o servidor funciona, mas ninguém que assiste
   pela ponte sai da camada em que começou (a 2).

A implementação de referência das sondas está em `crates/zap-rate/src/probe.rs` (decisão e ritmo,
sem I/O) e `crates/zap-net/src/probe.rs` (envio); o servidor de teste (`crates/zap-test-server`) as
usa na ponte.
