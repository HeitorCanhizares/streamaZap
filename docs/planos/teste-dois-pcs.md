# Roteiro: primeiro teste da v3 entre dois PCs

Até aqui a transmissão só rodou nos testes automáticos. Este roteiro é para o primeiro teste de
verdade, com duas máquinas e duas pessoas (ou uma pessoa andando entre os PCs). Comece pelo modo
**Local**, que não depende do servidor. O modo **Online** só vale a pena depois que o servidor de
produção seguir o contrato novo (`docs/ESTADO.md`, "O que o servidor de produção precisa mudar").

## Preparação (nos dois PCs)

1. **Executável.** Use o `target/release/streamazap.exe`, gerado com
   `cd app && npx tauri build --no-bundle`. É um arquivo só: copie para os dois PCs (pen drive,
   rede, o que for). Ainda não tem instalador porque falta a chave do updater, e ele também não
   remove o app Python.
2. **WebView2.** O Windows 10/11 atualizado já tem. Se o app não abrir janela nenhuma, instale o
   "WebView2 Runtime" da Microsoft.
3. **Feche o app Python** nos dois PCs, para não confundir qual app está em qual sala.
4. **Radmin ou mesma rede.** Os dois PCs precisam estar na mesma rede do Radmin (ou na mesma rede
   local).
5. **Firewall.** Abra o app, vá em **Diagnóstico** e clique **Corrigir firewall**. Ele pede
   administrador uma vez. As portas são UDP 47800 (mídia) e 47801 (descoberta).
6. Em **Configurações → Perfil → Seu nome na sala**, ponha um nome diferente em cada PC.

## Teste 1: Local, uma pessoa compartilha

| Passo | PC A (compartilha) | PC B (assiste) | Deu certo se |
|---|---|---|---|
| 1 | **Nova sala**, modo **Local**, fonte **Tela inteira** (a miniatura tem que mostrar a tela de verdade e se atualizar) | — | A sala abre e o cartão **Você** mostra a prévia ao vivo da tela de A |
| 2 | — | Na lista **Radmin / rede local**, a sala de A aparece. **Entrar** | B entra em até ~3 s |
| 3 | — | — | B vê a tela de A, sem travar e sem borrão parado |
| 4 | Abra um cronômetro com milésimos (um site serve) e fotografe as duas telas juntas com o celular | — | A diferença entre os dois cronômetros é o atraso. Anote |
| 5 | Mexa rápido numa janela, role uma página, toque um vídeo | — | A imagem acompanha; quando muda muito pode borrar um pouco, mas volta |
| 6 | Crie a sala com o som em **Só alguns apps**, marcando o navegador, e toque um vídeo nele | — | B ouve só o som do navegador, sincronizado com a imagem |
| 7 | Mande uma mensagem no **Chat** | Responda | As duas chegam |

Se a sala não aparecer no passo 2: em B, use **Entrar por IP** com o IP do Radmin de A (aparece
na barra da janela de A, como `Radmin · 26.x.x.x`) e anote que a descoberta falhou.

## Teste 2: trocar dentro da sala

Em A, clique **Transmissão** (na barra do vídeo) ou **Tela** (no cartão Você). No painel, troque e
confira em B a cada **Aplicar**:
- **Janela**: uma janela só, inclusive com ela atrás de outra ou minimizada;
- **Só som**: B ouve, mas não vê imagem nenhuma de A;
- **Som**: de "Todo o PC" para "Só alguns apps" e de volta;
- **Qualidade**: de 720p30 para 1080p60;
- **Minha câmera**: ligue, troque o **Dispositivo** (se A tiver duas câmeras) e veja a câmera de A no
  canto do cartão Você e na tela de B;
- **Tela 2**, se A tiver dois monitores.

## Teste 3: os dois compartilham

B também clica **Compartilhar** e escolhe uma tela. Os dois devem ver a tela um do outro ao mesmo tempo, e a
imagem de A não pode piorar muito por causa disso.

## Teste 4: queda e volta

1. Com B assistindo, desligue o Radmin (ou o Wi-Fi) de B por uns 3 s e religue. A imagem deve
   voltar sozinha, sem sair da sala.
2. Faça o mesmo por uns 20 s. Aqui pode cair da sala; anote o que o app mostrou.
3. B sai da sala e entra de novo.
4. A fecha o app com B assistindo. B deve ver um aviso, não uma tela congelada para sempre.
5. No modo Online: A (o dono) sai. A sala tem que sumir da lista de B na hora (antes ficava até
   2 minutos).

## O que me mandar depois

- Em cada PC: **Diagnóstico → Copiar relatório**, colado na conversa (ele traz placa de vídeo,
  conversor GPU/CPU, codificador por camada e as checagens de rede).
- Para cada teste: passou ou não, e o que apareceu na tela quando não passou (um print ajuda).
- O atraso medido no passo 4 do Teste 1.
- Modelo da placa de vídeo dos dois PCs, e se algum deles é notebook.

## Depois: Online

Quando o servidor de produção estiver no contrato novo, repita os testes 1, 3 e 4 com o modo
**Online** (os dois PCs fora do Radmin, de preferência em redes diferentes). Em **Configurações →
Conexão**, **Testar conexão** tem que mostrar o servidor `streamazap.heitorcanhizares.net`.
