//! Configurações persistidas (`settings.json`) e identidade TLS (`identity.der`).

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use zap_net::tls::Identity;

use crate::types::{EncoderPref, Preset, RoomMode, UpdateChannel};
use crate::CoreError;

/// Endereço do servidor Online de produção, **sem `/v1`** (o `ServerClient` acrescenta sozinho).
///
/// Não entra em `ServerSettings::default()`: o padrão continua vazio para os testes nunca
/// falarem com a produção. Só o app de verdade (`Core::start`) o grava, na primeira execução,
/// por `CoreOptions::default_server`.
pub const PRODUCTION_SERVER: &str = "https://streamazap.heitorcanhizares.net:7443";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct ServerSettings {
    /// Modo Online ligado. Desligado (padrão), o app não abre nenhuma conexão com o servidor.
    /// Arquivo antigo sem o campo: `settings::load` liga se já havia senha (migração).
    pub enabled: bool,
    pub address: String,
    pub password: String,
    pub use_relay: bool,
}
impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            address: String::new(),
            password: String::new(),
            use_relay: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct LocalSettings {
    /// `None` = todas as interfaces.
    pub interface: Option<String>,
    pub port: u16,
    pub announce: bool,
}
impl Default for LocalSettings {
    fn default() -> Self {
        Self {
            interface: None,
            port: 47800,
            announce: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct VideoSettings {
    pub camera_id: Option<String>,
    pub mirror_preview: bool,
    pub default_preset: Preset,
    /// Qualidade da câmera, à parte da tela (`camera_ladder`). Padrão 720p30.
    pub camera_preset: Preset,
    /// Jeito da câmera para quem assiste (canto da tela ou solta). Padrão: canto inferior direito.
    pub camera_layout: zap_proto::CameraLayout,
    pub encoder: EncoderPref,
    pub smoothing_auto: bool,
    pub smoothing_ms: u32,
}
impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            camera_id: None,
            mirror_preview: true,
            default_preset: Preset::P720p30,
            camera_preset: Preset::P720p30,
            camera_layout: zap_proto::CameraLayout::default(),
            encoder: EncoderPref::Auto,
            smoothing_auto: true,
            smoothing_ms: 200,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct SoundSettings {
    pub initial_volume: f32,
    pub message_sound: bool,
    pub notify_room_open: bool,
    pub mute_when_minimized: bool,
}
impl Default for SoundSettings {
    fn default() -> Self {
        Self {
            initial_volume: 0.8,
            message_sound: true,
            notify_room_open: true,
            mute_when_minimized: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct UpdateSettings {
    /// Procurar atualização sozinho.
    pub check: bool,
    /// Baixar sozinho quando houver.
    pub download: bool,
    /// Quando instalar o que foi baixado.
    pub install: InstallMode,
    pub channel: UpdateChannel,
}
impl Default for UpdateSettings {
    fn default() -> Self {
        Self {
            check: true,
            download: true,
            install: InstallMode::Auto,
            channel: UpdateChannel::Estavel,
        }
    }
}

/// Quando instalar a atualização já baixada. Serde: `"ask"` | `"on_exit"` | `"auto"`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstallMode {
    /// Pergunta ao usuário.
    Ask,
    /// Instala ao fechar o app.
    OnExit,
    /// Instala sozinho quando o usuário não estiver numa sala (o app decide pelo tempo ocioso).
    #[default]
    Auto,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub name: String,
    /// "#RRGGBB".
    pub color: String,
    /// uuid v4, gerado uma vez (`load` preenche se vazio).
    pub install_id: String,
    pub open_in: RoomMode,
    pub server: ServerSettings,
    pub local: LocalSettings,
    pub video: VideoSettings,
    pub sound: SoundSettings,
    pub update: UpdateSettings,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            name: String::new(),
            color: "#3BF08A".into(),
            install_id: String::new(),
            open_in: RoomMode::Local,
            server: Default::default(),
            local: Default::default(),
            video: Default::default(),
            sound: Default::default(),
            update: Default::default(),
        }
    }
}

impl Settings {
    /// Regras que dependem de mais de um campo, aplicadas por `load` e por `Core::update_settings`:
    /// com o Online desligado, `open_in` é sempre `Local`.
    pub fn normalize(&mut self) {
        if !self.server.enabled {
            self.open_in = RoomMode::Local;
        }
    }
}

/// Migra um `settings.json` antigo (já lido como JSON) para o formato atual, **antes** de
/// desserializar em `Settings`. Tarefa core-01. Regras:
/// - `server` sem a chave `enabled`: `enabled = !server.password.is_empty()` (sem `server`, nada);
/// - `update` com a chave `auto` e sem `check`: `auto: true` vira `check = true`, `download = true`,
///   `install = "auto"`; `auto: false` vira `check = false`, `download = false`, `install = "ask"`.
///   A chave `auto` é removida. `update` sem `auto` nem `check` fica como está (o padrão do struct,
///   tudo ligado e `auto`, vale para instalação nova).
pub(crate) fn migrate(raw: &mut serde_json::Value) {
    let Some(raiz) = raw.as_object_mut() else {
        return;
    };
    if let Some(server) = raiz.get_mut("server").and_then(|v| v.as_object_mut()) {
        if !server.contains_key("enabled") {
            // Quem já tinha senha usava o Online: continua ligado.
            let tem_senha = server
                .get("password")
                .and_then(|v| v.as_str())
                .is_some_and(|p| !p.is_empty());
            server.insert("enabled".into(), serde_json::Value::Bool(tem_senha));
        }
    }
    if let Some(update) = raiz.get_mut("update").and_then(|v| v.as_object_mut()) {
        if update.contains_key("auto") && !update.contains_key("check") {
            // Valor não booleano conta como ligado (o padrão antigo).
            let auto = update.get("auto").and_then(|v| v.as_bool()).unwrap_or(true);
            update.insert("check".into(), serde_json::Value::Bool(auto));
            update.insert("download".into(), serde_json::Value::Bool(auto));
            update.insert(
                "install".into(),
                serde_json::Value::String(if auto { "auto" } else { "ask" }.into()),
            );
            update.remove("auto");
        }
    }
}

/// Lê `<dir>/settings.json`. Ausente ou corrompido: padrão (e loga). Sempre devolve
/// `install_id` e `name` preenchidos (nome padrão = "Jogador" + 4 dígitos aleatórios).
pub fn load(dir: &Path) -> Settings {
    let caminho = dir.join("settings.json");
    let mut s = match std::fs::read(&caminho) {
        Ok(bytes) => match serde_json::from_slice::<serde_json::Value>(&bytes).and_then(|mut raw| {
            migrate(&mut raw);
            serde_json::from_value::<Settings>(raw)
        }) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("settings.json inválido ({e}); usando padrão");
                Settings::default()
            }
        },
        // Primeira execução: sem arquivo ainda é o caso normal, não loga.
        Err(e) if e.kind() == io::ErrorKind::NotFound => Settings::default(),
        Err(e) => {
            tracing::warn!("não foi possível ler settings.json ({e}); usando padrão");
            Settings::default()
        }
    };
    if s.install_id.is_empty() {
        s.install_id = uuid::Uuid::new_v4().to_string();
    }
    if s.name.is_empty() {
        s.name = format!("Jogador{:04}", rand::random::<u16>() % 10000);
    }
    s.normalize();
    s
}

/// Escrita atômica: grava `settings.json.tmp` e renomeia para `settings.json`.
pub fn save(dir: &Path, s: &Settings) -> io::Result<()> {
    let json = serde_json::to_vec_pretty(s).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    gravar_atomico(dir, "settings.json", &json)
}

/// Aplica um patch JSON parcial (mescla recursiva de objetos; valor não objeto substitui) sobre
/// `s` e valida o resultado desserializando em `Settings`. `install_id` nunca muda. Erro de
/// formato → `CoreError::Invalid`.
pub fn apply_patch(s: &Settings, patch: &serde_json::Value) -> Result<Settings, CoreError> {
    let mut base = serde_json::to_value(s).map_err(|e| CoreError::Invalid(e.to_string()))?;
    mesclar(&mut base, patch);
    let mut novo: Settings = serde_json::from_value(base).map_err(|e| CoreError::Invalid(e.to_string()))?;
    // A identidade da instalação não pode ser trocada por um patch.
    novo.install_id = s.install_id.clone();
    novo.normalize();
    Ok(novo)
}

/// Mescla `patch` em `alvo`: objeto sobre objeto mescla chave a chave; o resto substitui.
fn mesclar(alvo: &mut serde_json::Value, patch: &serde_json::Value) {
    if let (serde_json::Value::Object(a), serde_json::Value::Object(p)) = (&mut *alvo, patch) {
        for (k, v) in p {
            mesclar(a.entry(k.clone()).or_insert(serde_json::Value::Null), v);
        }
        return;
    }
    *alvo = patch.clone();
}

/// Carrega `<dir>/identity.der` (formato: `u32 BE` tamanho do certificado + certificado +
/// chave) ou gera uma identidade nova, grava e devolve. Arquivo corrompido: gera de novo.
pub fn load_or_create_identity(dir: &Path, name: &str) -> Result<Identity, CoreError> {
    if let Ok(bytes) = std::fs::read(dir.join("identity.der")) {
        if let Some(id) = decodificar_identidade(&bytes) {
            return Ok(id);
        }
        tracing::warn!("identity.der corrompido; gerando identidade nova");
    }
    let id = Identity::generate(name)?;
    let tamanho =
        u32::try_from(id.cert_der.len()).map_err(|_| CoreError::Invalid("certificado grande demais".into()))?;
    let mut bytes = Vec::with_capacity(4 + id.cert_der.len() + id.key_der.len());
    bytes.extend_from_slice(&tamanho.to_be_bytes());
    bytes.extend_from_slice(&id.cert_der);
    bytes.extend_from_slice(&id.key_der);
    gravar_atomico(dir, "identity.der", &bytes)?;
    Ok(id)
}

/// Decodifica `u32 BE` + certificado + chave. Qualquer inconsistência → `None`.
fn decodificar_identidade(bytes: &[u8]) -> Option<Identity> {
    let cabeca: [u8; 4] = bytes.get(..4)?.try_into().ok()?;
    let tamanho = u32::from_be_bytes(cabeca) as usize;
    let resto = bytes.get(4..)?;
    if tamanho == 0 || tamanho >= resto.len() {
        // Chave vazia ou certificado ocupando o arquivo todo: inválido.
        return None;
    }
    let (cert, chave) = resto.split_at(tamanho);
    Some(Identity {
        cert_der: cert.to_vec(),
        key_der: chave.to_vec(),
    })
}

/// Grava em `<dir>/<nome>.tmp`, força para o disco e renomeia para `<dir>/<nome>`.
/// O rename é atômico no mesmo diretório: a leitura nunca vê arquivo pela metade.
fn gravar_atomico(dir: &Path, nome: &str, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("{nome}.tmp"));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, dir.join(nome))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    /// Diretório temporário único, removido ao sair de escopo (mesmo se um assert falhar).
    struct DirTemp(PathBuf);
    impl DirTemp {
        fn novo() -> Self {
            let p = std::env::temp_dir().join(format!("zapcore-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&p).expect("criar diretório temporário");
            Self(p)
        }
    }
    impl Drop for DirTemp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn load_ausente_gera_padrao_com_ids() {
        let t = DirTemp::novo();
        let dir = t.0.join("inexistente");
        let s = load(&dir);
        assert!(s.name.starts_with("Jogador") && s.name.len() == 11, "nome: {}", s.name);
        assert!(uuid::Uuid::parse_str(&s.install_id).is_ok());
        assert_eq!(s.color, "#3BF08A");
        assert_eq!(s.local, LocalSettings::default());
    }

    #[test]
    fn load_corrompido_volta_padrao() {
        let t = DirTemp::novo();
        std::fs::write(t.0.join("settings.json"), "{ isto não é json").expect("escrever");
        let s = load(&t.0);
        assert_eq!(s.color, "#3BF08A");
        assert_eq!(s.video, VideoSettings::default());
        assert!(!s.name.is_empty());
        assert!(uuid::Uuid::parse_str(&s.install_id).is_ok());
    }

    #[test]
    fn save_load_ida_e_volta() {
        let t = DirTemp::novo();
        let mut s = load(&t.0);
        s.name = "Teste".into();
        s.local.port = 50123;
        save(&t.0, &s).expect("salvar");
        let lido = load(&t.0);
        assert_eq!(lido.name, "Teste");
        assert_eq!(lido.local.port, 50123);
        assert_eq!(lido, s);
    }

    #[test]
    fn save_nao_deixa_tmp() {
        let t = DirTemp::novo();
        save(&t.0, &load(&t.0)).expect("salvar");
        assert!(t.0.join("settings.json").exists());
        assert!(!t.0.join("settings.json.tmp").exists());
    }

    #[test]
    fn patch_mescla_aninhado() {
        let s = load(&DirTemp::novo().0);
        let r = apply_patch(&s, &json!({"video": {"smoothing_ms": 350}})).expect("patch válido");
        assert_eq!(r.video.smoothing_ms, 350);
        assert_eq!(r.video.mirror_preview, s.video.mirror_preview);
        assert_eq!(r.video.default_preset, s.video.default_preset);
        assert_eq!(r.name, s.name);
        assert_eq!(r.local, s.local);
        assert_eq!(r.sound, s.sound);
    }

    #[test]
    fn patch_nao_muda_install_id() {
        let s = load(&DirTemp::novo().0);
        let r = apply_patch(&s, &json!({"install_id": "outro-id"})).expect("patch válido");
        assert_eq!(r.install_id, s.install_id);
    }

    #[test]
    fn patch_invalido_erro() {
        let s = load(&DirTemp::novo().0);
        let r = apply_patch(&s, &json!({"local": {"port": "x"}}));
        assert!(matches!(r, Err(CoreError::Invalid(_))));
    }

    /// Grava um `settings.json` bruto num diretório temporário e carrega.
    fn carregar_json(v: serde_json::Value) -> Settings {
        let t = DirTemp::novo();
        std::fs::write(t.0.join("settings.json"), v.to_string()).expect("escrever");
        load(&t.0)
    }

    #[test]
    fn migra_server_sem_enabled_com_senha_liga() {
        let s = carregar_json(json!({"server": {"address": "a:1", "password": "x"}}));
        assert!(s.server.enabled);
    }

    #[test]
    fn migra_server_sem_enabled_sem_senha_fica_desligado() {
        let s = carregar_json(json!({"server": {"address": "a:1", "password": ""}}));
        assert!(!s.server.enabled);
    }

    #[test]
    fn server_com_enabled_explicito_nao_e_alterado() {
        let a = carregar_json(json!({"server": {"enabled": false, "password": "x"}}));
        assert!(!a.server.enabled);
        let b = carregar_json(json!({"server": {"enabled": true, "password": ""}}));
        assert!(b.server.enabled);
    }

    #[test]
    fn migra_update_auto_true() {
        let s = carregar_json(json!({"update": {"auto": true, "channel": "estavel"}}));
        assert!(s.update.check && s.update.download);
        assert_eq!(s.update.install, InstallMode::Auto);
    }

    #[test]
    fn migra_update_auto_false() {
        let s = carregar_json(json!({"update": {"auto": false, "channel": "estavel"}}));
        assert!(!s.update.check && !s.update.download);
        assert_eq!(s.update.install, InstallMode::Ask);
    }

    #[test]
    fn update_novo_formato_nao_migra() {
        let s = carregar_json(json!({"update": {"check": true, "download": false, "install": "on_exit"}}));
        assert!(s.update.check && !s.update.download);
        assert_eq!(s.update.install, InstallMode::OnExit);
    }

    #[test]
    fn instalacao_nova_padrao_tudo_ligado_e_auto() {
        let t = DirTemp::novo();
        let s = load(&t.0.join("inexistente"));
        assert!(s.update.check && s.update.download);
        assert_eq!(s.update.install, InstallMode::Auto);
        assert!(!s.server.enabled);
    }

    #[test]
    fn install_mode_serde() {
        assert_eq!(
            serde_json::from_str::<InstallMode>("\"on_exit\"").expect("ler"),
            InstallMode::OnExit
        );
        for (m, txt) in [
            (InstallMode::Ask, "\"ask\""),
            (InstallMode::OnExit, "\"on_exit\""),
            (InstallMode::Auto, "\"auto\""),
        ] {
            assert_eq!(serde_json::to_string(&m).expect("gravar"), txt);
            assert_eq!(serde_json::from_str::<InstallMode>(txt).expect("ler"), m);
        }
    }

    #[test]
    fn open_in_vira_local_com_online_desligado() {
        let desligado = carregar_json(json!({"open_in": "online", "server": {"enabled": false}}));
        assert_eq!(desligado.open_in, RoomMode::Local);
        let ligado = carregar_json(json!({"open_in": "online", "server": {"enabled": true}}));
        assert_eq!(ligado.open_in, RoomMode::Online);
    }

    #[test]
    fn apply_patch_desligando_online_normaliza() {
        let mut s = Settings::default();
        s.server.enabled = true;
        s.open_in = RoomMode::Online;
        let r = apply_patch(&s, &json!({"server": {"enabled": false}})).expect("patch válido");
        assert!(!r.server.enabled);
        assert_eq!(r.open_in, RoomMode::Local);
    }

    #[test]
    fn camera_defaults() {
        let v = VideoSettings::default();
        assert_eq!(v.camera_preset, Preset::P720p30);
        assert_eq!(v.camera_layout, zap_proto::CameraLayout::default());
        // Arquivo antigo sem os campos de câmera carrega com os padrões.
        let s = carregar_json(json!({"video": {"smoothing_ms": 300}}));
        assert_eq!(s.video.smoothing_ms, 300);
        assert_eq!(s.video.camera_preset, Preset::P720p30);
        assert_eq!(s.video.camera_layout, zap_proto::CameraLayout::default());
    }

    #[test]
    fn identidade_persiste() {
        let t = DirTemp::novo();
        let a = load_or_create_identity(&t.0, "zap").expect("criar identidade");
        let b = load_or_create_identity(&t.0, "zap").expect("ler identidade");
        assert_eq!(a.fingerprint(), b.fingerprint());
        assert_eq!(a.key_der, b.key_der);
    }

    #[test]
    fn identidade_corrompida_regenera() {
        let t = DirTemp::novo();
        // Tamanho de certificado maior que o restante do arquivo.
        std::fs::write(t.0.join("identity.der"), [0u8, 0, 1, 0, 1, 2, 3]).expect("escrever");
        let a = load_or_create_identity(&t.0, "zap").expect("regenerar");
        assert!(!a.cert_der.is_empty() && !a.key_der.is_empty());
        // Depois de regenerada, o arquivo é válido e persiste.
        let b = load_or_create_identity(&t.0, "zap").expect("ler regenerada");
        assert_eq!(a.fingerprint(), b.fingerprint());
    }
}
