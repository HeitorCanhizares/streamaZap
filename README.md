<p align="center"><img src="app/public/icone.png" width="160" alt="streamaZap"></p>

# streamaZap

Compartilhe a sua **tela**, uma **janela**, a **webcam** ou **só o som** com os amigos, pelo
**Radmin VPN** ou pela rede de casa. App open source para Windows, escrito em Rust + Tauri.

- 🖥 **Tela inteira, uma janela ou a webcam.** A janela é capturada mesmo atrás de outras.
- 🔊 **O som do jeito que você quiser:** o do PC inteiro (menos o do próprio app), só o de alguns
  apps (ex.: só o jogo ou só o navegador), nenhum, ou **só o som**, sem imagem.
- 📡 **As salas aparecem sozinhas** para quem está na mesma rede do Radmin ou na rede de casa.
- 👥 **Várias transmissões ao mesmo tempo:** cada uma vai para a **tela principal** (lado a lado)
  ou fica pequena na **faixa de baixo**; a câmera de quem está com a tela na principal aparece no
  canto dela. Quem entra vê e ouve na hora, sem clicar em nada.
- 🎚 **Qualidade que se ajusta a cada pessoa:** quem transmite manda três versões do vídeo
  (simulcast) e cada amigo recebe a que a conexão dele aguenta, sem piorar a dos outros.
  Até **1080p60**.
- 🌊 **Vídeo liso:** a suavização se ajusta sozinha à rede de cada um, e o som acompanha a imagem.
- ✏️ **Trocar sem sair da sala:** fonte, som, qualidade e câmera, pelo botão **Transmissão**.
- 🔒 Senha opcional (sala com senha não mostra prévia na lista), 💬 chat, volume por pessoa e
  volume geral, tela cheia (F11).
- 🩺 **Diagnóstico** que explica o que está errado (firewall, Radmin, placa de vídeo) e corrige o
  firewall com um clique.
- ⚡ Vídeo H.264 pela placa de vídeo (Media Foundation: NVIDIA, Intel, AMD), com reserva no
  processador; som Opus.
- 🔄 **Atualiza sozinho**, conferindo a assinatura, e nunca reinicia com você numa sala.

## Download

Baixe a versão mais nova em **[Releases](https://github.com/HeitorCanhizares/streamaZap/releases/latest)**:

- `StreamaZap-Setup-X.Y.Z.exe`: instalador. Não pede administrador.
- `StreamaZap-Portatil-X.Y.Z.zip`: versão portátil, é só descompactar e abrir.

**Requisitos:** Windows 10 (versão 2004 ou mais nova, para o som por app) ou Windows 11, 64 bits.

**Vindo da versão antiga (Python)?** O instalador remove a antiga sozinho (essa remoção pede
administrador uma vez). As configurações da versão antiga não vêm junto.

## Como usar com amigos

1. Todos instalam o [Radmin VPN](https://www.radmin-vpn.com/) e entram na **mesma rede** do Radmin
   (ou ficam na mesma rede de casa).
2. Na primeira vez, abra **Diagnóstico** e clique **Corrigir firewall**.
3. Quem vai transmitir clica **Nova sala**, escolhe o modo **Local**, a fonte (tela, janela, webcam
   ou só o som), o som e a qualidade.
4. Os amigos veem a sala na lista em poucos segundos e clicam **Entrar na sala**.

Se a sala não aparecer, use **Entrar por IP** (embaixo da lista, no modo Local) com o IP do Radmin
de quem está transmitindo (aparece na barra de cima do app dele, como `Radmin · 26.x.x.x`).

> **Dica:** no Radmin a banda costuma ser limitada; **720p 30 fps** é o mais estável.

**Na sala:** os botões da chamada somem com o mouse parado e voltam ao mexer. Atalhos: **F11** tela
cheia, **Esc** sai da tela cheia, **M** silencia a sala, **C** liga ou desliga a sua câmera.

### Quando não funciona

- **A sala aparece mas não dá para entrar**, ou **um vê o outro e o outro não**: quase sempre é o
  firewall do PC de quem não recebe nada. Abra **Diagnóstico → Corrigir firewall** nesse PC.
  Antivírus com firewall próprio (Kaspersky, Avast, ESET, Norton…) precisam liberar o streamaZap
  neles.
- **Amigo longe travando:** no Radmin, veja se a conexão com ele é **direta** (relay tem bem menos
  banda). O app já manda para ele uma versão mais leve do vídeo, sem piorar a dos outros.
- **Todos na mesma versão:** o app avisa quando alguém na rede tem uma versão mais nova.
- Para pedir ajuda, mande o **Diagnóstico → Copiar relatório**.

### Música da sala

Na aba **Música** do painel da sala, cole um link do YouTube (vídeo, live, shorts ou
`music.youtube.com`): ele entra numa fila que todo mundo ouve junto. Cada um toca no próprio app,
pelo player do YouTube, no mesmo ponto da sala; ninguém precisa de conta. A música pula quando
metade da sala vota, e quem pôs (ou quem criou a sala) pula direto. Cada um vê os próprios anúncios
do YouTube; quando o anúncio acaba, o player volta para onde a sala está.

Funciona nas salas **Locais**. Nas salas Online depende de o servidor ter a música (ainda não tem:
ver `docs/ESTADO.md`, "Pendente no servidor de produção").

### Modo Online

O modo **Online** (salas pelo servidor, com código, sem precisar de Radmin) está quase pronto: o
servidor já foi atualizado para a v3 e é fechado com senha. Quem tem a senha digita em
**Configurações → Conexão**. A música da sala ainda não funciona no Online (falta o servidor). Sem a senha, use o modo **Local**.

## Como funciona

| Parte | Tecnologia |
|---|---|
| Interface | Tauri 2 + React + TypeScript; vídeo decodificado com WebCodecs |
| Captura | Monitor por Desktop Duplication (DXGI), janela por Windows Graphics Capture, webcam por Media Foundation |
| Conversão de cor | NV12 pela placa de vídeo (Video Processor do D3D11), com reserva no processador |
| Vídeo | H.264 por Media Foundation (hardware ou software), sem B-frames; 3 camadas (simulcast) |
| Áudio | Opus 48 kHz estéreo; som por app via WASAPI *process loopback* |
| Rede | QUIC (quinn): controle em streams, mídia em datagramas; tudo em UDP |
| Descoberta | Anúncio UDP na porta **47801** em cada rede do PC (inclusive a do Radmin) |
| Portas | Sala na UDP **47800** (ou a próxima livre); a mídia usa a porta seguinte |
| Qualidade | Cada espectador manda relatórios; o alocador escolhe a camada de cada um pela banda medida |
| Suavização | Relógio de reprodução por participante: o atraso cobre a oscilação recente da rede |

Detalhes no [plano da v3](docs/arquitetura-v3.md) e no [contrato do protocolo](docs/api/).

## Desenvolvimento

Precisa de Rust (estável) e Node.js.

```bash
cd app
npm install
npm run tauri dev        # abre o app
npm run dev              # só a interface no navegador, com dados de exemplo
```

Verificação (a mesma do CI):

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd app && npm run check && npm test
```

Fora do Windows o núcleo compila com stubs e um backend sintético (útil para os testes), mas não
captura nada.

### Estrutura

```
crates/zap-proto    tipos e formatos do protocolo (sem I/O)
crates/zap-rate     qualidade e latência: alocador de camadas, relógio, simulador de enlace
crates/zap-net      QUIC, descoberta na rede local, cliente da API do servidor
crates/zap-gpu      D3D11, texturas, conversão para NV12
crates/zap-capture  monitor (DXGI), janela (WGC), câmera
crates/zap-encode   H.264 por Media Foundation
crates/zap-audio    Opus, som por app e saída de áudio
crates/zap-media    pipelines de vídeo e áudio + backend sintético
crates/zap-sys      firewall, interfaces de rede, Radmin
crates/zap-core     a sala, as transmissões e o estado para a interface
app/src-tauri       app Tauri (comandos, eventos, atualizador)
app/src             interface React
docs/               estado atual, arquitetura, protocolo e design
streamazap/         app antigo em Python (sai no marco M6)
```

O estado atual e os próximos passos ficam em [`docs/ESTADO.md`](docs/ESTADO.md).

### Publicar uma versão

1. Ajuste a versão em `Cargo.toml` (workspace), `app/src-tauri/tauri.conf.json`,
   `app/package.json` e no `VERSAO` de `App.tsx` e `ConfigSobre.tsx`.
2. Envie a tag: `git tag v3.X.Y && git push origin v3.X.Y`.

O workflow `release-v3.yml` gera o instalador, o zip portátil, o `latest.json` e as assinaturas, e
publica a Release. Push no `master` só roda o CI (não publica nada).

- **Canal beta:** tag com sufixo (ex.: `v3.2.0-beta.1`). A Release sai como prerelease e só chega
  a quem escolheu o canal beta nas Configurações.
- **Assinatura do atualizador:** a chave pública fica em `plugins.updater.pubkey` do
  `tauri.conf.json`; a privada e a senha ficam nos segredos do GitHub
  `TAURI_SIGNING_PRIVATE_KEY` e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Sem a chave privada, quem já
  tem o app não recebe mais atualizações: guarde um backup.

## Licença

[MIT](LICENSE)
