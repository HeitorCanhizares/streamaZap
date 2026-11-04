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

Para subir alguém de camada sem arriscar travar a imagem, quem envia para aquele espectador
pode mandar **sondas** (datagramas `0x03`) por 200 ms na taxa `kbps(próxima camada) − kbps(atual)`,
no máximo uma rajada a cada 5 s por espectador. Se o próximo relatório vier com `estado = normal`
e `banda_kbps` suficiente, a subida segue o passo 3. Servidores podem não implementar sondas: o
estimador do app sobe sozinho (8%/s) quando a conexão está folgada, só que mais devagar.

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
