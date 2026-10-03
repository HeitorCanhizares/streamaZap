<p align="center"><img src="streamazap/assets/icon.png" width="160" alt="StreamaZap"></p>

# StreamaZap

Compartilhe sua **tela** (ou uma **janela**) e o **som dos aplicativos que você escolher**
com amigos na rede local ou pelo **Radmin VPN**. Aplicativo open source escrito em Python.

- 🖥 **Tela inteira ou uma janela específica.** A janela é capturada mesmo atrás de outras.
- 🔊 **Som do jeito que você quiser:** sem som, todo o som do PC (exceto o do próprio StreamaZap)
  ou só os aplicativos marcados (ex.: apenas o jogo, ou apenas o navegador).
- 📡 **As salas aparecem sozinhas.** Quem cria uma sala anuncia a sala em todas as redes do PC,
  incluindo a do Radmin VPN. Quem estiver na mesma rede do Radmin vê a sala na lista e clica para entrar.
- 👥 **Todo mundo pode compartilhar:** qualquer participante clica em *Compartilhar minha tela*
  e os outros escolhem em *Assistindo:* quem querem ver. Cada um envia o próprio vídeo direto
  para quem assiste (P2P pelo Radmin), dividindo a carga.
- ✏️ **Editar a transmissão ao vivo** (trocar tela/janela, som, qualidade) sem derrubar ninguém.
- 🌊 **Vídeo liso:** buffer de suavização ajustável (*Suavização*, padrão 200 ms) e qualidade
  adaptável, que reduz o bitrate sozinha quando alguém está com a conexão lenta e volta a subir depois.
- 🌍 **Feito para amigos longe:** mede a latência de cada um, reconecta sozinho se a conexão
  cair e não acumula atraso (descarta vídeo velho em vez de deixar o atraso crescer).
- 🔒 Senha opcional na sala, 💬 chat e lista de quem está assistindo com a latência de cada um.
- ⚡ Vídeo H.264 pela GPU (NVIDIA NVENC, Intel QuickSync, AMD AMF) com fallback para CPU; áudio Opus.

## Download

Baixe o instalador mais recente em **[Releases](https://github.com/HeitorCanhizares/streamaZap/releases/latest)**:

- `StreamaZap-Setup-X.Y.Z.exe` — instalador (cria atalhos e já libera o app no Firewall do Windows);
- `StreamaZap-X.Y.Z-portable.zip` — versão portátil, é só extrair e abrir `StreamaZap.exe`.

**Atualização automática:** ao abrir, o app verifica se há versão nova nas Releases. Se
houver, baixa o instalador (conferindo o SHA-256), instala em modo silencioso e reabre já
atualizado — o Windows pede a permissão de administrador uma vez. Dá para desligar em
*"Atualizar automaticamente ao abrir"* na tela inicial. Na versão portátil o app apenas
mostra o link para baixar.

**Requisitos:** Windows 10 versão 2004 ou mais recente (para capturar o som por aplicativo), 64 bits.

## Como usar com amigos (Radmin VPN)

1. Todos instalam o [Radmin VPN](https://www.radmin-vpn.com/) e entram na **mesma rede** do Radmin.
2. Todos abrem o StreamaZap. A barra inferior mostra `✅ Radmin VPN conectado: 26.x.x.x`.
3. Quem vai transmitir clica em **Criar sala**, escolhe a tela ou janela, o som e a qualidade,
   e clica em **Iniciar transmissão**.
4. Os amigos veem a sala aparecer na lista em poucos segundos e dão **duplo clique** para entrar.

Se a sala não aparecer, use **Entrar por IP…** com o IP do Radmin de quem está transmitindo
(ex.: `26.12.34.56`). A janela de transmissão mostra os seus endereços.

> **Dica:** no Radmin a banda costuma ser limitada; o preset **720p 30fps** é o mais estável.

### Amigo em outro país dando timeout ou travando

- No Radmin, veja se a conexão com o amigo é **direta** (e não "relay"/retransmitida): relay
  tem banda bem menor. O ping aparece ao lado do nome dele na janela de transmissão.
- Deixe **Qualidade adaptável** ligada (em *Editar transmissão*): o host ajusta o bitrate
  pela conexão mais lenta da sala.
- Quem está longe pode aumentar a **Suavização** (ex.: 500 ms): troca um pouco de atraso por
  um vídeo bem mais liso.
- Se a conexão cair, o app tenta reconectar sozinho (faixa laranja em cima do vídeo).

Atalhos do player: **F11** ou duplo clique = tela cheia, **Esc** = sai da tela cheia.

## Como funciona

| Parte | Tecnologia |
|---|---|
| Interface | PySide6 (Qt) |
| Captura de tela/janela | GDI `BitBlt` / `PrintWindow` (Windows), `mss` (outros sistemas) |
| Captura de som por app | WASAPI *process loopback* (Windows 10 2004+) via `ctypes` |
| Vídeo | H.264 via PyAV/FFmpeg (NVENC / QSV / AMF / x264), baixa latência, sem B-frames |
| Áudio | Opus 48 kHz estéreo, quadros de 20 ms |
| Descoberta de salas | Broadcast UDP na porta **47801** em cada interface (ex.: `26.255.255.255` no Radmin) |
| Transmissão | TCP na porta **47800** (usa as próximas portas se estiver ocupada; quem compartilha dentro da sala usa 47801+) |
| Suavização | Jitter buffer: cada quadro leva o horário de captura e é exibido em ritmo constante |
| Congestionamento | Atraso medido pela fila do host + latência vs. mínima recente (pega *bufferbloat* da VPN); bitrate cai/volta sozinho |

Quando a conexão de um espectador não acompanha, o host descarta o vídeo atrasado
dele e envia um novo keyframe, então o atraso não acumula. Numa simulação de link de
1,5 Mbit/s com 150 ms de latência e até 80 ms de variação, a qualidade adaptável derrubou a
latência de ~10 s para ~200 ms, mantendo 30 fps sem travadas.

Firewall: o instalador cria a regra `StreamaZap` liberando o programa. Na versão portátil,
aceite o aviso do Firewall do Windows (marque redes **públicas** também, pois é assim que o
Windows costuma classificar o Radmin).

## Desenvolvimento

```bash
python -m venv .venv
.venv\Scripts\activate            # Linux/macOS: source .venv/bin/activate
pip install -r requirements-dev.txt
python -m streamazap               # abre o app
pytest                             # testes
ruff check .                       # lint
```

Fora do Windows o app roda (útil para assistir e para desenvolver), mas compartilha apenas
o monitor inteiro e sem som.

### Estrutura

```
streamazap/
  app.py              ponto de entrada
  discovery.py        anúncio/descoberta de salas (UDP)
  netutil.py          interfaces de rede, detecção do Radmin
  protocol.py         protocolo TCP (handshake, vídeo, áudio, chat)
  host.py             servidor da sala
  client.py           espectador
  player.py           reprodução de áudio com buffer
  capture/screen.py   captura de tela e janelas
  capture/audio.py    escolha de apps e mixagem
  capture/audio_win.py  WASAPI process loopback
  media/              codificadores H.264 e Opus
  ui/                 janelas Qt
packaging/            PyInstaller e Inno Setup
.github/workflows/    CI e geração do instalador
```

### Ícone

A arte fica em `packaging/icon-source.*` (png, jpg ou webp, quadrada). Para trocar,
substitua esse arquivo: a build roda `packaging/make_icon.py`, que recorta o fundo
branco fora do quadrado arredondado e gera o `.ico` do executável/instalador e o
`.png` das janelas. Localmente: `pip install pillow && python packaging/make_icon.py`.

### Releases automáticas

Cada push no branch `master` compila o app no GitHub Actions (Windows), gera o
instalador e publica uma **Release** automaticamente. É de lá que o app baixa as
atualizações.

- A versão é `MAJOR.MINOR` de `streamazap/__init__.py` + o número da build
  (ex.: `0.1.42`). Para mudar MAJOR/MINOR, edite `__version__`.
- Push em outros branches só compila: o instalador fica em **Actions → (build) → Artifacts**.
- Para não publicar um push específico, escreva `[skip release]` na mensagem do commit.
- Commits que só mexem em documentação (`*.md`) não geram build.
- Versão específica: `git tag v1.0.0 && git push origin v1.0.0`, ou
  **Actions → Build & Release → Run workflow**.

## Licença

[MIT](LICENSE)
