# M1: núcleo portável (`zap-proto`, `zap-rate`, `zap-net`)

Plano do arquiteto. Base: `docs/arquitetura-v3.md` e `docs/api/`. Tudo aqui compila e roda no
Linux; nada de `windows`/`unsafe` neste marco.

## Escopo

Entra: tipos e formatos do protocolo 4; algoritmos de qualidade/latência com simulador; transporte
QUIC (aperto de mão, controle, quadros com prazo, datagramas, relatórios); descoberta LAN; teste de
ponta a ponta com mídia sintética e rede ruim simulada.

Fica de fora (outros marcos): captura/codificação real (M2), orquestração de sala e interface
(M3), cliente REST/WebSocket do servidor Online (M4).

## Frentes

| Id | Crate | Depende de | Resumo |
|---|---|---|---|
| `proto` | `zap-proto` | — | Tipos, mensagens de controle, cabeçalhos binários, HMAC, descoberta |
| `rate` | `zap-rate` | `proto` | Estimador de banda, alocador, escada, jitter buffer, simulador |
| `net` | `zap-net` | `proto`, `rate` | QUIC, controle, envio/recepção de mídia, relatórios, descoberta |

Convenção de tempo: em `zap-rate` todo instante é `u64` em microssegundos (`Us`) de um relógio
monotônico qualquer, passado como parâmetro. Em `zap-net`, `Clock` fornece esse número a partir
de `std::time::Instant` (origem fixada na criação).

---

## Frente `proto` (`crates/zap-proto`)

Dependências: `serde`, `serde_json`, `thiserror`, `hmac`, `sha2`, `base64`, `bytes`.

### `version.rs`
```rust
pub const PROTOCOL_VERSION: u32 = 4;
pub const ALPN: &[u8] = b"zap/4";
pub const DEFAULT_MEDIA_PORT: u16 = 47800;
pub const DISCOVERY_PORT: u16 = 47801;
pub const DISCOVERY_MAGIC: &str = "streamazap";
pub const MAX_CONTROL_MESSAGE: usize = 64 * 1024;
pub const MAX_DISCOVERY_DATAGRAM: usize = 1200;
```

### `ids.rs`
```rust
/// Código de convite `ZAP-XXXX` (alfabeto sem 0/O/1/I/L).
pub struct RoomCode(String);
impl RoomCode {
    pub const ALPHABET: &'static [u8] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";
    pub fn from_entropy(bytes: [u8; 4]) -> Self;          // cada byte % 31 escolhe um caractere
    pub fn parse(text: &str) -> Result<Self, ProtoError>; // aceita minúsculas, espaços nas pontas, sem o "ZAP-"
    pub fn as_str(&self) -> &str;                          // "ZAP-7K2Q"
}
```
`Display`, `Serialize`/`Deserialize` como string. `parse("zap-7k2q")`, `parse("7K2Q")` → `ZAP-7K2Q`;
`parse("ZAP-7K2O")` → erro (O fora do alfabeto); tamanho ≠ 4 → erro.

### `types.rs`
Tipos de `docs/api/mensagens.md#tipos-comuns`. Nomes Rust em inglês, nomes JSON em português
(`#[serde(rename = "...")]`), enums em minúsculas como na doc. Todos `Clone, Debug, PartialEq,
Serialize, Deserialize`. Campos opcionais com `#[serde(default, skip_serializing_if = "Option::is_none")]`.
Desserialização ignora campos desconhecidos (padrão do serde; não usar `deny_unknown_fields`).
```rust
pub enum Access { Open /* "aberta" */, Password /* "senha" */ }
pub enum RoomMode { Online, Local }
pub enum TrackKind { Screen /* "tela" */, Window /* "janela" */, Camera /* "camera" */, Audio /* "audio" */ }
pub enum TileSize { Thumbnail /* "miniatura" */, Main /* "principal" */, Full /* "cheia" */ }
pub struct Room { id, code: Option<RoomCode> /* "codigo" */, name /* "nome" */, owner_id /* "dono_id" */, access /* "acesso" */, mode /* "modo" */, created_at: String /* "criada_em" */ }
pub struct Participant { id, name, color /* "cor" */, role: Role /* "papel": dono|membro */, joined_at /* "entrou_em" */, app_version /* "versao_app" */, protocol /* "protocolo" */, latency_ms: Option<u32>, watching: Option<String> /* "assistindo" */ }
pub struct LayerSpec { layer: u8 /* "camada" */, width /* "largura" */: u16, height /* "altura" */: u16, fps: u8, kbps: u32 }
pub struct AudioSpec { rate /* "taxa" */: u32, channels /* "canais" */: u8, kbps: u32 }
pub struct MediaAddress { addresses /* "enderecos" */: Vec<String>, port /* "porta" */: u16, fingerprint: String }
pub struct Track { id, participant_id, number /* "numero" */: u16, kind /* "tipo" */, title /* "titulo" */, layers /* "camadas" */: Vec<LayerSpec>, audio: Option<AudioSpec>, address /* "endereco" */: Option<MediaAddress> }
```

### `control.rs`
`#[serde(tag = "t")] pub enum ControlMsg` com **todas** as mensagens de `docs/api/mensagens.md`
(conexão, aperto de mão, salas, chat, trilhas, assinaturas, rota direta, conexão reversa,
estatísticas) **e** `relatorio` de `docs/api/midia.md`. Nome de cada variante =
`#[serde(rename = "<t da doc>")]`. Campos `ref` opcionais onde a doc permite.
```rust
pub struct Report {                       // t = "relatorio"
    pub arrivals: Vec<(u16, u8, u32, u32, u64, u32)>, // "chegadas": trilha, camada, seq, envio_us, chegada_us, bytes
    pub probes: Vec<(u16, u32, u32, u64, u32)>,       // "sondas": cluster, seq, envio_us, chegada_us, bytes
    pub lost: Vec<(u16, u8, u32)>,                    // "perdidos"
    pub loss: f32,                                    // "perda"
    pub bandwidth_kbps: Option<u32>,                  // "banda_kbps"
    pub state: Option<DelayState>,                    // "estado": sobrecarga|normal|ociosa
    pub buffer_ms: Option<u32>,
}
pub enum DelayState { Overuse, Normal, Underuse }
```
Teste obrigatório: para cada variante, um JSON de exemplo **copiado da doc** desserializa e volta
igual (round-trip), e `t` sai com o nome da doc.

### `framing.rs`
```rust
pub fn encode_control(msg: &ControlMsg) -> Result<Vec<u8>, ProtoError>; // u32 BE + JSON
pub struct ControlDecoder { /* buffer interno */ }
impl ControlDecoder {
    pub fn new() -> Self;
    pub fn push(&mut self, bytes: &[u8]);
    pub fn next(&mut self) -> Result<Option<ControlMsg>, ProtoError>; // None = falta byte
}
```
Tamanho > `MAX_CONTROL_MESSAGE` → `ProtoError::TooLarge` (não aloca). Mensagem com `t`
desconhecido → `ProtoError::UnknownMessage(String)` e o decoder **segue** para a próxima
(compatibilidade para frente). JSON quebrado → `ProtoError::Json`.

### `media.rs`
Formatos binários de `docs/api/midia.md`, big-endian.
```rust
pub const VIDEO_HEADER_LEN: usize = 30;
pub const AUDIO_HEADER_LEN: usize = 20;
pub const PROBE_HEADER_LEN: usize = 12;
pub struct VideoFlags(u8); // KEYFRAME = 0x01, PARAMS = 0x02
pub struct VideoHeader { flags: VideoFlags, track: u16, layer: u8, width: u16, height: u16, seq: u32, capture_us: u64, send_us: u32, size: u32 }
impl VideoHeader { pub fn encode(&self, out: &mut [u8; 30]); pub fn decode(buf: &[u8]) -> Result<Self, ProtoError>; }
pub struct AudioHeader { track: u16, seq: u32, capture_us: u64, send_us: u32 }
pub struct ProbeHeader { cluster: u16, seq: u32, send_us: u32 }
pub enum Datagram<'a> { Audio(AudioHeader, &'a [u8]), Probe(ProbeHeader, usize /* bytes totais */) }
pub fn encode_audio(h: &AudioHeader, opus: &[u8]) -> Vec<u8>;
pub fn encode_probe(h: &ProbeHeader, total_len: usize) -> Vec<u8>; // completa com zeros; total_len >= 12
pub fn decode_datagram(buf: &[u8]) -> Result<Datagram<'_>, ProtoError>;
```
Erros: `tipo` errado, buffer curto, reservado ≠ 0 → aceitar (ignorar) para compatibilidade.

### `auth.rs`
```rust
pub fn password_hmac(password: &str, nonce: &[u8]) -> [u8; 32];             // HMAC-SHA256
pub fn verify_password_hmac(password: &str, nonce: &[u8], mac: &[u8]) -> bool; // tempo constante
pub fn b64(bytes: &[u8]) -> String; pub fn unb64(text: &str) -> Result<Vec<u8>, ProtoError>; // base64 padrão com padding
```
Teste com vetor fixo (calcule uma vez com Python `hmac.new(b"senha", nonce, sha256)` e cole).

### `discovery.rs`
```rust
pub enum Announcement { Room(RoomAnnouncement), Presence(PresenceAnnouncement) }
pub struct RoomAnnouncement { room: RoomSummary /* "sala" */, owner: OwnerSummary /* "dono" */, address: MediaAddress, participants: u32, highlight: Option<Highlight> /* "destaque" */, app_version: String }
pub struct PresenceAnnouncement { install_id: String, name: String, app_version: String, hears: Vec<String> /* "ouve" */ }
pub fn encode_announcement(a: &Announcement) -> Result<Vec<u8>, ProtoError>; // inclui "app" e "v"; erro se > 1200 bytes
pub enum ParsedAnnouncement { Ok(Announcement), OtherVersion { v: u32, app_version: Option<String> } }
pub fn parse_announcement(buf: &[u8]) -> Option<ParsedAnnouncement>; // None = não é do streamaZap
```

### `error.rs`
`#[derive(thiserror::Error)] pub enum ProtoError { TooLarge(usize), Truncated, BadType(u8), Json(#[from] serde_json::Error), UnknownMessage(String), InvalidRoomCode(String), Base64 }`.

---

## Frente `rate` (`crates/zap-rate`)

Sem dependências de runtime (só `zap-proto` para `LayerSpec`/`DelayState`). `pub type Us = u64;`

### `window.rs`
- `SlidingMin<T: PartialOrd + Copy>` e `SlidingMax`: `new(window_us)`, `push(now, value) -> T`
  (mínimo/máximo da janela), `get() -> Option<T>`, `reset()`. Deque monotônica, O(1) amortizado.
- `SlidingQuantile`: histograma de baldes de 1 ms de 0 a 2000 ms (acima de 2000 vai no último),
  com fila de (instante, balde) para expirar. `new(window_us)`, `push(now, value_ms: f64)`,
  `quantile(q: f64) -> Option<f64>` (interpolação dentro do balde), `len()`.
- `RateMeter`: bytes por segundo numa janela (`new(window_us)`, `add(now, bytes)`,
  `rate_bps(now) -> f64`).
- `LossMeter`: `new(window_us)`, `record(now, received: bool)`, `fraction(now) -> f32`.

### `unwrap.rs`
`SeqUnwrapper` e `TimeUnwrapper`: transformam `u32` que dá a volta (seq, `send_us`) em `u64`
crescente (diferença interpretada como `i32`).

### `trendline.rs` (detector de atraso, estilo GCC)
```rust
pub struct DelayDetector { /* … */ }
impl DelayDetector {
    pub fn new() -> Self;
    /// Um pacote/quadro: instante de envio (já desembrulhado) e de chegada, ambos em µs.
    pub fn on_packet(&mut self, send_us: Us, arrival_us: Us, bytes: u32) -> DelayState;
    pub fn state(&self) -> DelayState;
}
```
Regras (números fixos):
1. Agrupa pacotes cujo `send_us` cai em janelas de **5 ms** (rajada). Um grupo fecha quando chega
   pacote de outra janela.
2. Para grupos consecutivos: `d = (chegada_último_i − chegada_último_{i−1}) − (envio_último_i − envio_último_{i−1})` em ms.
   Atraso acumulado `acc += d`; suavizado `s = 0,9·s + 0,1·acc`.
3. Regressão linear de `(chegada_ms, s)` nos últimos **20** grupos → inclinação `k`.
   `m = k × min(n_grupos, 60) × 4` (ganho do GCC).
4. Limiar adaptativo `γ`, inicial **12,5**: a cada grupo `γ += Δt_ms × K × (|m| − γ)` com
   `K = 0,039` se `|m| > γ` senão `0,0087`; limitar `γ` em `[6, 600]`; ignorar a atualização se
   `|m| − γ > 15`.
5. `m > γ` por **≥ 10 ms** e `m` não caindo → `Overuse`; `m < −γ` → `Underuse`; senão `Normal`.

### `bwe.rs` (estimador de banda)
```rust
pub struct BweConfig { pub min_kbps: u32 /* 150 */, pub max_kbps: u32, pub start_kbps: u32 }
pub struct Bwe { /* DelayDetector + AIMD + perda */ }
impl Bwe {
    pub fn new(cfg: BweConfig) -> Self;
    pub fn on_packet(&mut self, send_us: Us, arrival_us: Us, bytes: u32);
    pub fn on_loss_fraction(&mut self, now: Us, loss: f32);
    pub fn update(&mut self, now: Us) -> u32;          // chamado a cada ≥ 25 ms; devolve estimativa kbps
    pub fn estimate_kbps(&self) -> u32;
    pub fn state(&self) -> DelayState;
}
```
Regras:
- Vazão medida = `RateMeter` de 500 ms dos bytes que chegaram.
- AIMD: `Overuse` → alvo = **0,85 × vazão medida** (no máximo 1 redução a cada 200 ms) e marca
  "última sobrecarga". `Underuse` → mantém. `Normal`: se a vazão medida < 0,5 × alvo (o enlace não
  está sendo usado: app-limited), **não sobe**; senão, longe da última sobrecarga (alvo > 1,5 ×
  vazão na última sobrecarga ou nenhuma) sobe **8% por segundo** (proporcional ao Δt);
  perto dela, sobe aditivo **+ 1200 bytes × 8 por RTT de 100 ms** (≈ +96 kbps/s).
- Perda (janela de 2 s): `> 10%` → alvo × (1 − 0,5 × perda) (no máximo 1 vez a cada 300 ms);
  `< 2%` → nada; entre os dois, mantém.
- Estimativa = `clamp(alvo, min_kbps, max_kbps)`.

### `ladder.rs`
```rust
pub enum Preset { P720p30, P720p60, P1080p30, P1080p60 }
pub fn screen_ladder(preset: Preset, source_w: u32, source_h: u32) -> Vec<LayerSpec>;
pub fn camera_ladder(source_w: u32, source_h: u32) -> Vec<LayerSpec>;
```
Tabela de `docs/api/balanceamento.md#1-escada-de-camadas`. Largura = altura × proporção da fonte,
arredondada para **par**; nunca aumenta a fonte (fonte 1280×720 com preset 1080p → camada 0 vira
1280×720 e o bitrate escala por pixels: `kbps × (w×h)/(1920×1080)`, mínimo o da camada 1); camadas
que ficariam iguais ou maiores que a de cima são removidas (exceto a 0).

### `allocator.rs`
Implementa `docs/api/balanceamento.md#3-algoritmo-a-cada-100-ms` **exatamente**, com os números de lá.
```rust
pub struct ViewerInput { pub id: u64, pub bandwidth_kbps: u32, pub state: DelayState, pub max_layer: u8 /* teto já combinado */, pub reserved_kbps: u32 /* áudio + outras trilhas */ }
pub enum Budget { Relay, Direct { upload_kbps: u32 } }
pub struct ViewerDecision { pub id: u64, pub current: u8, pub pending: Option<u8> /* aguardando keyframe */ }
pub struct LayerRequest { pub layer: u8, pub kbps: u32 }
pub struct Allocator { /* estado de histerese por espectador */ }
impl Allocator {
    pub fn new(layers: Vec<LayerSpec>) -> Self;
    pub fn set_layers(&mut self, layers: Vec<LayerSpec>);          // trilha.atualizar
    pub fn update(&mut self, now: Us, viewers: &[ViewerInput], budget: Budget) -> AllocOutput;
    pub fn on_keyframe(&mut self, layer: u8);                       // efetiva trocas pendentes para essa camada
}
pub struct AllocOutput { pub viewers: Vec<ViewerDecision>, pub requests: Vec<LayerRequest>, pub keyframes: Vec<u8> /* camadas que precisam de keyframe agora */ }
```
- Espectador novo começa com `current = 2` (ou a menor camada existente) e `pending = None`.
- `keyframes`: máx. 1 por camada a cada **1 s**.
- `requests` só muda quando o conjunto muda ou algum `kbps` varia > 10%, no máx. 2 vezes/s
  (senão devolve a última).
- Testes: os 3 exemplos de "Modo Local" e o exemplo da ponte da doc, histerese (não sobe antes de 3 s,
  não sobe duas vezes em 5 s, desce na hora com `Overuse`, desce após 500 ms de banda baixa).

### `jitter.rs` (relógio de reprodução / suavização)
```rust
pub enum MediaKind { Video, Audio }
pub struct PlayoutConfig { pub auto: bool, pub manual_delay_ms: u32 /* 200 */, pub max_delay_ms: u32 /* 1500 */, pub frame_interval_ms: f64 }
pub struct PlayoutClock { /* … */ }
impl PlayoutClock {
    pub fn new(cfg: PlayoutConfig) -> Self;
    pub fn observe(&mut self, now: Us, capture_us: Us, kind: MediaKind);
    /// Instante local em que essa mídia deve tocar.
    pub fn play_time(&self, capture_us: Us) -> Option<Us>;
    /// Velocidade de reprodução do áudio (0,98..=1,02) para seguir mudanças do alvo sem cortes.
    pub fn audio_rate(&self) -> f64;
    pub fn current_delay_ms(&self) -> f64; // suavização em uso
    pub fn tick(&mut self, now: Us);       // a cada ~10 ms: anda a rampa
    pub fn set_auto(&mut self, auto: bool); pub fn set_manual_delay(&mut self, ms: u32);
}
```
Regras:
- `trânsito = now − capture_us`; `base = SlidingMin(10 s)` do trânsito.
- `jitter_ms = (trânsito − base)/1000`, entra num `SlidingQuantile` de **5 s**.
- Alvo automático = `p95(jitter) + frame_interval_ms + 5 ms` (decodificação), limitado a
  `[frame_interval_ms, max_delay_ms]`.
- O atraso em uso persegue o alvo: se o alvo está acima, sobe a até **20 ms por segundo**
  (2%), mas se uma mídia chegou **atrasada** (depois do seu `play_time`) sobe na hora até cobrir
  esse atraso; se o alvo está abaixo há ≥ **5 s**, desce a **15 ms por segundo** (1,5%).
  Atraso em uso > alvo + 300 ms → pula direto para o alvo.
- `audio_rate()` = 1 + (velocidade da rampa em ms/s)/1000 (sobe o atraso → toca mais devagar).
- Manual: atraso = `manual_delay_ms`, sem rampa.
- `play_time = capture_us + base + atraso_em_uso`.

### `sim.rs` (simulador de enlace, público)
```rust
pub struct LinkConfig { pub base_delay_ms: f64, pub jitter_ms: f64 /* desvio, distribuição normal truncada ≥ 0 */, pub loss: f64, pub queue_bytes: usize /* drop-tail */ }
pub struct Link { /* fila FIFO com taxa variável */ }
impl Link {
    pub fn new(cfg: LinkConfig, seed: u64) -> Self;
    pub fn set_capacity_kbps(&mut self, now: Us, kbps: u32);
    /// Envia `bytes` em `now`; devolve o instante de chegada, ou None se perdeu/estourou a fila.
    pub fn send(&mut self, now: Us, bytes: u32) -> Option<Us>;
    pub fn queue_delay_us(&self, now: Us) -> Us;
}
```
PRNG próprio (xorshift64*), sem dependência. Determinístico pelo `seed`.

### Testes de aceite (`crates/zap-rate/tests/`)
Simulação: fonte de vídeo 60 fps cujo tamanho de quadro segue o bitrate alvo do `Bwe`
(keyframe 8× a cada 10 s), relatórios a cada 100 ms com 50 ms de atraso de volta.
1. **Convergência**: enlace 8 Mbps, 30 ms, jitter 2 ms → estimativa entre 6,8 e 8,4 Mbps em ≤ 10 s
   partindo de 1 Mbps; fila do enlace < 60 ms de média depois dos 10 s.
2. **Queda**: 8 → 3 Mbps no segundo 20 → `Overuse` em ≤ 500 ms; estimativa < 3,2 Mbps em ≤ 1,5 s;
   atraso de fila volta a < 100 ms em ≤ 3 s.
3. **Volta**: 3 → 8 Mbps no segundo 40 → estimativa > 6 Mbps em ≤ 15 s.
4. **Perda aleatória 1%** sem gargalo: estimativa não cai abaixo de 85% do máximo.
5. **Jitter**: enlace com jitter 20 ms → `PlayoutClock` auto estabiliza com < 1% de mídia atrasada
   e atraso em uso entre 35 e 80 ms.

---

## Frente `net` (`crates/zap-net`)

Dependências: `tokio` (rt-multi-thread, net, time, sync, macros), `quinn` (runtime-tokio,
rustls-ring), `rustls` 0.23 (ring), `rcgen`, `sha2`, `bytes`, `socket2`, `if-addrs`, `tracing`,
`thiserror`, `zap-proto`, `zap-rate`.

### `clock.rs`
`Clock` (`Clone`): `new()`, `now_us() -> u64` (desde a criação), `send_us() -> u32` (o mesmo,
truncado).

### `tls.rs`
```rust
pub struct Identity { pub cert_der: Vec<u8>, pub key_der: Vec<u8> }
impl Identity { pub fn generate(name: &str) -> Result<Self, NetError>; pub fn fingerprint(&self) -> String; } // SHA-256 hex minúsculo
pub fn fingerprint_of(cert_der: &[u8]) -> String;
pub fn server_config(id: &Identity) -> Result<quinn::ServerConfig, NetError>;
/// Cliente que só aceita o certificado com essa impressão digital (modo Local) ou, com `None`,
/// as raízes web do sistema (modo Online: usa `webpki-roots`).
pub fn client_config(pinned: Option<&str>) -> Result<quinn::ClientConfig, NetError>;
```
Os dois com ALPN `zap/4` e o `TransportConfig` de `docs/api/midia.md#parâmetros-de-transporte`
(BBR via `quinn::congestion::BbrConfig`, idle 10 s, 1024 streams uni, datagramas ligados, MTU
inicial 1200).

### `endpoint.rs`
```rust
pub async fn bind_server(id: &Identity, preferred_port: u16) -> Result<(quinn::Endpoint, u16), NetError>; // tenta porta, porta+1 … +20
pub fn client_endpoint() -> Result<quinn::Endpoint, NetError>;
```

### `control.rs`
```rust
pub struct ControlTx { /* SendStream + Mutex */ } // Clone (Arc)
impl ControlTx { pub async fn send(&self, msg: &ControlMsg) -> Result<(), NetError>; }
pub struct ControlRx { /* RecvStream + ControlDecoder */ }
impl ControlRx { pub async fn recv(&mut self) -> Result<Option<ControlMsg>, NetError>; } // None = fim
```
Mensagens desconhecidas são logadas e puladas.

### `handshake.rs`
```rust
pub enum ServerAuth { Open, Password(String), Ticket(Box<dyn Fn(&str) -> bool + Send + Sync>) }
pub struct Hello { /* campos de `ola` */ }
pub async fn accept_handshake(conn: &quinn::Connection, auth: &ServerAuth, room: &Room, participant_id: String) -> Result<(ControlTx, ControlRx, Hello), NetError>;
pub async fn connect_handshake(conn: &quinn::Connection, hello: Hello, password: Option<&str>) -> Result<(ControlTx, ControlRx, Welcome), NetError>;
```
Segue a tabela "Aperto de mão QUIC" de `mensagens.md`; tempo máximo 10 s; senha errada →
`recusado {codigo: "senha_sala"}` e `NetError::Rejected(codigo)`.

### `media_tx.rs`
```rust
pub struct MediaSender { /* conn, clock, métricas */ }
impl MediaSender {
    pub fn new(conn: quinn::Connection, clock: Clock) -> Self;
    /// Envia um quadro num stream novo; se não terminar em `deadline`, reseta (código 0x10) e
    /// devolve `Ok(false)`.
    pub async fn send_video(&self, header: VideoHeader, payload: bytes::Bytes, deadline: std::time::Duration) -> Result<bool, NetError>;
    pub fn send_audio(&self, header: AudioHeader, opus: &[u8]) -> Result<(), NetError>;
    pub fn send_probe(&self, cluster: u16, seq: u32, total_len: usize) -> Result<(), NetError>;
    pub fn rtt(&self) -> std::time::Duration;
    pub fn deadline(&self) -> std::time::Duration; // max(150 ms, 2 × RTT)
}
```
`send_us` do cabeçalho é preenchido aqui (no instante do envio). Prioridade de stream: keyframe 1,
demais 0.

### `media_rx.rs`
```rust
pub enum MediaEvent { Video { header: VideoHeader, data: bytes::Bytes, arrival_us: u64 }, Audio { header: AudioHeader, opus: bytes::Bytes, arrival_us: u64 }, Lost { track: u16, layer: u8, seq: u32 } }
pub fn spawn_receiver(conn: quinn::Connection, clock: Clock, tx: tokio::sync::mpsc::Sender<MediaEvent>, reports: ReportBuilderHandle) -> tokio::task::JoinHandle<()>;
```
Aceita streams uni (cada um vira uma task curta que lê cabeçalho + `tamanho` bytes com limite de
16 MiB) e datagramas. Stream resetado/incompleto → `Lost`. Buraco de `seq` numa (trilha, camada)
que não se fecha em 300 ms → `Lost`. Toda chegada (vídeo, áudio, sonda) vai para o relatório.

### `report.rs`
```rust
pub struct ReportBuilder { /* chegadas pendentes + Bwe (zap-rate) + LossMeter */ }
pub type ReportBuilderHandle = std::sync::Arc<parking_lot::Mutex<ReportBuilder>>;
impl ReportBuilder {
    pub fn new(estimate: bool /* app = true, ponte = false */) -> Self;
    pub fn on_video(&mut self, h: &VideoHeader, arrival_us: u64, bytes: u32);
    pub fn on_audio(&mut self, h: &AudioHeader, arrival_us: u64, bytes: u32);
    pub fn on_probe(&mut self, h: &ProbeHeader, arrival_us: u64, bytes: u32);
    pub fn on_lost(&mut self, track: u16, layer: u8, seq: u32);
    pub fn set_buffer_ms(&mut self, ms: u32);
    pub fn take(&mut self, now: u64) -> Report; // esvazia as listas
}
pub fn spawn_reporter(ctrl: ControlTx, rb: ReportBuilderHandle, clock: Clock) -> tokio::task::JoinHandle<()>; // a cada 100 ms, e na hora ao virar Overuse
```
(`parking_lot` entra como dependência; o lock é curto e nunca dentro de `await`.)

### `discovery.rs`
```rust
pub struct DiscoveryConfig { pub port: u16 /* 47801 */, pub interval: Duration /* 1,5 s */, pub room_timeout: Duration /* 5 s */, pub extra_targets: Vec<std::net::SocketAddr> /* testes: unicast */, pub broadcast: bool /* false nos testes */ }
pub struct Discovery { /* socket + tabela */ }
impl Discovery {
    pub async fn start(cfg: DiscoveryConfig, install_id: String) -> Result<Self, NetError>;
    pub fn set_room(&self, a: Option<RoomAnnouncement>);           // o que eu anuncio (None = nada)
    pub fn set_presence(&self, p: PresenceAnnouncement);
    pub fn rooms(&self) -> Vec<(RoomAnnouncement, std::net::IpAddr)>; // sem expiradas
    pub fn peers(&self) -> Vec<PeerInfo>;                           // presença: nome, versão, ip, "me ouve"
    pub fn other_versions(&self) -> Vec<(std::net::IpAddr, u32)>;
}
pub fn ipv4_interfaces() -> Vec<InterfaceInfo>; // Radmin (26.0.0.0/8) primeiro; ignora loopback
```
Envia em broadcast dirigido de cada interface (ex.: `26.255.255.255`) e em `255.255.255.255`.
Socket com `SO_REUSEADDR` e `SO_BROADCAST`. Ignora os próprios pacotes (`install_id`).

### Testes de aceite (`crates/zap-net/tests/`)
- `handshake.rs`: aberto, senha certa, senha errada (`senha_sala`), ingresso válido/ inválido,
  fingerprint errada no cliente → falha de TLS.
- `media_loopback.rs`: 300 quadros sintéticos a 60 fps + áudio a 50/s pelo loopback; todos chegam
  na ordem de `seq`, `captura_us` intacto, relatórios chegam a cada ~100 ms.
- `impaired.rs`: com um **proxy UDP de teste** (`tests/support/proxy.rs`: tokio, atraso fixo,
  perda, limite de taxa com fila de 50 ms) em 3 Mbps e fonte em 6 Mbps: quadros velhos são
  resetados (nenhum chega com > 400 ms de atraso), a `banda_kbps` reportada fica entre 2,2 e
  3,6 Mbps depois de 10 s.
- `discovery.rs`: duas instâncias no loopback (portas diferentes, envio unicast para 127.0.0.1 em
  modo de teste) se enxergam, sala expira em 5 s, versão diferente vai para `other_versions`.

## Riscos

| Risco | Mitigação |
|---|---|
| Testes de tempo instáveis no CI | Algoritmos testados com relógio simulado; testes de rede com margens largas e `#[tokio::test(flavor = "multi_thread")]` |
| BBR do quinn ainda é "experimental" | Opção de trocar para Cubic numa linha (`TransportConfig`); o estimador do app é quem manda no bitrate |
| Broadcast UDP bloqueado no CI | Teste de descoberta usa unicast no loopback (`DiscoveryConfig` aceita destinos extras em teste) |

## Estado de fechamento do M1

**Fechado.** Gates: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace` (todos os testes de aceite) e `cargo check --workspace --target x86_64-pc-windows-gnu`.

Os 3 testes que o integrador deixou para o planejador foram resolvidos com decisões registradas em
`docs/arquitetura-v3.md` (6.1 e 6.3) e `docs/api/midia.md`:
- Convergência e volta de banda: fase inicial de 50%/s, esquecer a última sobrecarga após 5 s,
  sobrecarga do GCC só com 2 grupos seguidos, filtro de rajada (keyframe) e fila persistente.
  A faixa do teste de convergência passou a 0,8–1,1 × a capacidade (é a amplitude do dente de
  serra do AIMD) e a detecção da queda, a 700 ms (filtro de rajada de 200 ms + relatório).
- Rede ruim (net-23): fim de quadro = ACK, FIFO entre streams, admissão no `MediaSender`, Cubic,
  `perda` só de datagramas de áudio, e descarte do emissor como sinal no estimador.
- O teste de loopback usa prazo de 2 s (ele confere ordem e integridade; o prazo é testado no
  `impaired.rs`).
