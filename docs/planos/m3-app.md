# M3: `zap-core` + app Tauri + nova interface (modo Local de ponta a ponta)

Plano do arquiteto. Base: `docs/arquitetura-v3.md` (seções 5, 7.2 e 9), `docs/api/` e
`docs/design/`. Ao fim do marco, dois PCs na mesma rede (ou Radmin) criam/entram numa sala,
compartilham tela/janela/câmera com som por app, assistem um ao outro e conversam, pela interface
nova. Modo Online fica para o M4; updater, firewall e instalador para o M5.

## Frentes

| Id | Pasta / crate | Depende de | Resumo |
|---|---|---|---|
| `core` | `crates/zap-core` | M1, M2 | Sala Local, publicação, assinatura, playout, configurações, snapshots |
| `tauri` | `app/src-tauri` (crate `streamazap`) | `core` | Comandos, eventos, canais de vídeo, janela sem moldura |
| `ui-base` | `app/src/{theme,components,bridge,video}` | — | Tokens, componentes, ponte tipada com o núcleo (+ mock), player WebCodecs |
| `ui-telas` | `app/src/screens` | `ui-base` | As 5 telas do design ligadas à ponte |

Trava da agenda: `ui-base` e `ui-telas` usam a trava `app-ui` (o `tsc` checa o projeto inteiro).

---

## Frente `core` (`crates/zap-core`)

### Configurações (`settings.rs`)
```rust
#[derive(Serialize, Deserialize, Clone, Default)] #[serde(default)]
pub struct Settings {
    pub name: String, pub color: String /* "#3BF08A" */, pub install_id: String /* uuid v4, gerado uma vez */,
    pub open_in: RoomMode /* tela inicial: online|local */,
    pub server: ServerSettings { address: String, password: String, use_relay: bool /* true */ },
    pub local: LocalSettings { interface: Option<String> /* None = todas */, port: u16 /* 47800 */, announce: bool /* true */ },
    pub video: VideoSettings { camera_id: Option<String>, mirror_preview: bool /* true */, default_preset: Preset /* 720p30 */, encoder: EncoderPref /* auto */, smoothing_auto: bool /* true */, smoothing_ms: u32 /* 200 */ },
    pub sound: SoundSettings { initial_volume: f32 /* 0,8 */, message_sound: bool, notify_room_open: bool, mute_when_minimized: bool },
    pub update: UpdateSettings { auto: bool /* true */, channel: Channel /* estavel */ },
}
pub fn load(dir: &Path) -> Settings;                 // arquivo ausente/corrompido → padrão (e loga)
pub fn save(dir: &Path, s: &Settings) -> io::Result<()>; // escrita atômica (tmp + rename)
```
Também persiste a identidade TLS (`identity.der`) no mesmo diretório.

### Núcleo (`core.rs`)
```rust
pub trait UiSink: Send + Sync + 'static {
    fn state(&self, snapshot: Arc<Snapshot>);           // no máx. 4/s, e na hora em mudança de tela
    fn chat(&self, msg: ChatLine);
    fn video(&self, tile: TileId, packet: VideoPacket);  // já no instante de exibir
}
pub struct Core { /* runtime tokio, settings, discovery, sala atual */ }
impl Core {
    pub fn start(data_dir: PathBuf, ui: Arc<dyn UiSink>) -> Result<Self, CoreError>;
    pub fn settings(&self) -> Settings; pub fn update_settings(&self, f: impl FnOnce(&mut Settings));
    pub fn sources(&self) -> Sources;                    // monitores, janelas, câmeras, apps com som, codificadores
    pub fn thumbnail(&self, src: &SourceRef, w: u32, h: u32) -> Result<Rgba, CoreError>;
    pub async fn create_room(&self, req: CreateRoom) -> Result<(), CoreError>;
    pub async fn join_room(&self, target: JoinTarget /* Discovered{room_id} | Address{ip, port} */, password: Option<String>) -> Result<(), CoreError>;
    pub async fn leave_room(&self);
    pub async fn start_share(&self, req: ShareRequest) -> Result<(), CoreError>;  // tela/janela + áudio + preset + codificador
    pub async fn update_share(&self, req: ShareRequest) -> Result<(), CoreError>; // ao vivo, sem derrubar ninguém
    pub async fn stop_share(&self);
    pub async fn set_camera(&self, on: bool) -> Result<(), CoreError>;
    pub fn watch(&self, track_id: &str, tile: TileId, size: TileSize, max_layer: Option<u8>) -> Result<(), CoreError>;
    pub fn unwatch(&self, tile: TileId);
    pub fn set_volume(&self, participant_id: &str, volume: f32, muted: bool);
    pub fn set_smoothing(&self, auto: bool, ms: u32);
    pub async fn send_chat(&self, text: String) -> Result<(), CoreError>;
    pub fn snapshot(&self) -> Arc<Snapshot>;
}
```

### Sala Local (`room/`)
- **Quem cria** abre o endpoint QUIC (`zap_net::bind_server`, porta das configurações), anuncia
  na descoberta e aceita conexões com `accept_handshake` (aberta/senha). Mantém participantes,
  histórico de chat (200), trilhas com `numero` sequencial, e repassa os eventos de
  `docs/api/mensagens.md` a todos. Entrega `segredo_midia` no `bem_vindo` (adicionar o campo em
  `zap-proto`, tarefa `core-01`).
- **Membro** conecta no endereço anunciado (`connect_handshake`), com reconexão automática
  (backoff 0,5 s → 8 s, até 6 tentativas; volta a assinar o que assistia).
- **Conexão reversa**: se `connect` falhar em 6 s, pede `conexao.reversa` pelo dono da sala (ou,
  sem sala, por anúncio de descoberta como no app Python) e espera a conexão de volta por 12 s.
- Quem cria a sala sai → `sala.encerrada` para todos.

### Publicação (`publish.rs`)
Por trilha própria: `zap_media::VideoPipeline` (ou `AudioPipeline`) + `Allocator` + um
`ViewerLink` por espectador (`MediaSender`, preferências do `assinar`, último `relatorio`).
- Tarefa de envio: lê `EncodedFrame` do pipeline; para cada espectador cuja camada atual é a do
  quadro, `send_video` com o prazo do `MediaSender` (tarefa curta por quadro, sem bloquear os
  outros). Troca pendente + keyframe da camada nova → troca e manda `camada.atual`.
- A cada 100 ms: `Allocator::update` com `Budget::Direct { upload_kbps }` (estimativa de upload =
  soma das vazões entregues quando há sobrecarga em algum enlace; sem sinal = 50 Mbps), aplica
  `set_requests`/`request_keyframe` no pipeline.
- Áudio: cada `EncodedAudio` vai em datagrama para quem assina com `audio: true`.
- Câmera: trilha `camera` separada (escada de câmera). O "mostrar minha webcam no canto" é só
  composição na interface de quem assiste (PiP), não mistura no vídeo.

### Assinatura e reprodução (`subscribe.rs`, `playout.rs`)
Por trilha assistida: eventos do `spawn_receiver` daquela conexão (roteados por `trilha`) →
`PlayoutClock` (um por participante, compartilhado entre vídeo e áudio dele).
- Thread de playout (`THREAD_PRIORITY_ABOVE_NORMAL`, laço de 2 ms): entrega cada quadro ao
  `UiSink::video` no `play_time − 8 ms` (antecedência para decodificar). Vários vencidos de uma vez
  → entrega todos (o decodificador precisa das referências); o JS só desenha o último.
- Áudio: Opus → PCM na própria tarefa de rede → `TrackInput::push`; `set_rate(clock.audio_rate())`
  a cada 50 ms; volume por participante.
- `buffer_ms` do relatório = `clock.current_delay_ms()`.
- Sem quadro decodificável por 300 ms → `keyframe.pedir`.

### Snapshot (`snapshot.rs`)
```rust
pub struct Snapshot {
    pub version: u64,
    pub me: Me { name, color, install_id },
    pub network: NetworkStatus { radmin_ip: Option<String>, interfaces: Vec<String>, server: ServerStatus /* M4 */ },
    pub rooms: Vec<RoomListing>,           // descoberta (+ online no M4)
    pub peers: Vec<PeerListing>,           // presença: nome, versão, ip, "recebe você"
    pub room: Option<RoomView>,            // sala atual: participantes, trilhas, minhas trilhas, chat não lido
    pub tiles: Vec<TileView>,              // o que estou assistindo: camada, resolução, fps, kbps, ping, suavização, motivo
    pub share: Option<ShareView>,          // o que estou publicando: fonte, áudio, preset, codificador, camadas ativas, espectadores por camada, tempos
    pub errors: Vec<String>,
}
```
`Serialize` com nomes em `snake_case` (é consumido pelo TypeScript; os tipos TS são escritos à
mão em `app/src/bridge/types.ts`, e um teste compara com um JSON gerado por
`cargo test -p zap-core snapshot_exemplo -- --nocapture`).

### Testes de aceite do `core` (Linux, backend sintético)
1. Dono + 2 membros em processos-tarefa no loopback: entram, chat chega a todos, membro A publica
   (sintético 1080p60), membro B assiste e recebe quadros em ordem com atraso de reprodução
   < 120 ms; dono assiste em miniatura e recebe a camada 2.
2. Senha errada → erro `senha_sala`.
3. Membro cai (conexão fechada) → `participante.saiu {motivo: "caiu"}` em ≤ 6 s.
4. Troca de preset ao vivo (`update_share`) → espectadores continuam recebendo, com keyframe.

---

## Frente `tauri` (`app/src-tauri`, crate `streamazap`)

- Tauri 2 (`tauri = "2"`, `tauri-build = "2"`), plugins: `single-instance`, `notification`,
  `process` (o `updater` entra no M5). Entra no workspace (`members` += `app/src-tauri`).
- Janela: `decorations: false`, 1440×900, mínimo 1100×700, fundo `#0A100E`, título `streamaZap`.
  A barra de título é do design (arrastar por `data-tauri-drag-region`).
- `UiSink` → `AppHandle::emit("zap://state", snapshot)`, `emit("zap://chat", …)`; vídeo por
  `tauri::ipc::Channel<InvokeResponseBody>` registrado em `watch`, enviando bytes crus
  (`InvokeResponseBody::Raw`) no formato abaixo.
- Comandos (todos `async`, erro = `String` em português para a interface): `app_info`,
  `settings_get`, `settings_set(patch: serde_json::Value)`, `sources_list`, `source_thumbnail`,
  `room_create`, `room_join`, `room_leave`, `share_start`, `share_update`, `share_stop`,
  `camera_set`, `watch(track_id, tile, size, max_layer, channel)`, `unwatch(tile)`, `set_volume`,
  `set_smoothing`, `chat_send`, `chat_typing`, `diagnostics_run` (M5 completa), `window_*`.
- Pacote de vídeo para o JS (little-endian, 24 bytes + H.264 Annex B):
  `u16 trilha, u8 camada, u8 flags (bit0 keyframe), u16 largura, u16 altura, u32 seq,
  u64 captura_us, u32 reservado`.

## Frente `ui-base` (`app/src`)

- Vite + React 19 + TypeScript; `@fontsource/unbounded`, `@fontsource/space-grotesk`,
  `@fontsource/jetbrains-mono` (offline). CSS com os tokens de `docs/design/README.md` em
  `theme.css` (variáveis CSS) e classes utilitárias equivalentes às `zp-*` do design.
- Componentes (`components/`): `TitleBar`, `NavRail`, `Button` (primário, fantasma, perigo),
  `Toggle`, `Segmented`, `Card`, `Avatar` (inicial + cor), `Badge`, `Ping` (cores por faixa),
  `Slider`, `Select`, `TextField`, `VideoTile` (canvas + rótulos + volume), `ScenePreview`
  (as cenas ilustradas do design para prévias sem vídeo).
- Ponte (`bridge/`): `types.ts` (espelho do `Snapshot` e dos pedidos), `api.ts` (wrappers tipados
  de `invoke`), `store.ts` (`useSnapshot()` com `useSyncExternalStore` sobre `listen("zap://state")`),
  `mock.ts` (dados do design: salas "Sexta do Rush", "Filminho de domingo", "Live de Python",
  participantes Marcão/Ju/Rafa/Dudu) usado quando não está dentro do Tauri (`npm run dev` no
  navegador e testes).
- Player (`video/player.ts`): `VideoPlayer` por tile: recebe os pacotes do `Channel`, cria/
  reconfigura o `VideoDecoder` (`codec` a partir do SPS: `avc1.PPCCLL`; `hardwareAcceleration:
  "prefer-hardware"`; `optimizeForLatency: true`) quando muda camada/tamanho ou chega keyframe
  depois de erro; descarta delta antes do primeiro keyframe; desenha o `VideoFrame` mais recente no
  canvas (`drawImage`) no próximo `requestAnimationFrame` e fecha os demais; conta quadros
  desenhados/descartados. Testes com `VideoDecoder` falso (vitest).

## Frente `ui-telas` (`app/src/screens`)

Porte fiel de `docs/design/*.html` (comparar com `docs/design/prints/`), trocando os dados fixos
pela ponte:
1. **Salas**: lista (descoberta; aba Online desativada com "em breve" até o M4), busca, filtros
   (Todas/Abertas/Com senha), detalhe da sala selecionada com prévia (assinatura em miniatura,
   camada 2, sem áudio), "Entrar na sala", "Entrar com código/IP", nome do usuário.
2. **Criar sala**: modo (Local; Online desativado até o M4), nome, acesso/senha, o que
   compartilhar (Telas/Janelas/Webcam com miniaturas de `source_thumbnail`), webcam no canto,
   som (Sem som / Todo o PC / Só alguns apps), qualidade (4 presets), codificador, resumo à direita
   ("É isso que a galera vai ver"), botão "Iniciar transmissão".
3. **Dentro da sala**: vídeo principal com barra de controle (volume, câmera, compartilhar tela,
   suavização auto, tela cheia, sair), PiP da câmera, faixa de tiles (assistindo/volume/mudo/fechar,
   "Você" com Câmera/Tela), painel Chat/Pessoas, métricas no topo (`via … · ping · suav. · camada ·
   Mbps`), atalhos F11/Esc/M/C.
4. **Configurações**: perfil (nome, cor, prévia do chat), sobre/atualização (botões ficam prontos;
   ação real no M5), conexão (Online desativado até o M4; Local: rede, porta, anunciar), vídeo e
   câmera (câmera, espelhar prévia, qualidade padrão, suavização), som e avisos, atalhos.
   "Tudo salva sozinho": cada mudança chama `settings_set`.
5. **Diagnóstico**: estrutura e textos do design; dados reais de rede/amigos/salas da descoberta
   neste marco; servidor e firewall no M4/M5.

## Testes de aceite do marco

1. `cargo test --workspace` e `npm test` verdes; `npm run check` sem erros.
2. `npm run dev` (navegador, mock) mostra as 5 telas iguais aos prints (verificação visual com
   Playwright: diferença pequena de pixels nas áreas sem dados dinâmicos).
3. Num PC com Windows: `npm run tauri dev`, criar sala, entrar de outro PC/instância, compartilhar
   tela e câmera, assistir, chat. Registrar latência e qualidade na tela de diagnóstico.
