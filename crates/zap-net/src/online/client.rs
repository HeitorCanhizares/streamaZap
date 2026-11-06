//! `ServerClient`: chamadas REST autenticadas.
use super::error::ApiError;
use super::types::*;
use super::ws::{WsRx, WsTx};
use parking_lot::Mutex;
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;
use std::time::Duration;
use zap_proto::RoomCode;

/// Base sem barra final (`https://h:7443/` vira `https://h:7443`).
fn sem_barra(base: &str) -> &str {
    base.trim_end_matches('/')
}

/// Falha de transporte (DNS, TLS, conexão, tempo esgotado).
fn rede(e: reqwest::Error) -> ApiError {
    ApiError::Network(e.to_string())
}

/// Token vigente (trocado pela renovação).
#[derive(Debug, Clone)]
pub(crate) struct Session {
    pub token: String,
    pub expires_at: String,
    pub participant_id: String,
}

#[derive(Debug)]
pub(crate) struct Inner {
    /// Sem barra final, ex.: `https://zap.exemplo.com:7443`.
    pub base_url: String,
    pub http: reqwest::Client,
    pub session: Mutex<Session>,
}

/// Cliente da API do servidor. Barato de clonar.
#[derive(Debug, Clone)]
pub struct ServerClient {
    pub(crate) inner: Arc<Inner>,
}

impl ServerClient {
    /// `GET /v1/info` (sem autenticação).
    pub async fn info(base: &str) -> Result<ServerInfo, ApiError> {
        // Um cliente novo e curto: o "Testar conexão" não pode ficar preso num servidor mudo.
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| ApiError::Network(e.to_string()))?;
        let url = format!("{}/v1/info", sem_barra(base));
        let resp = http.get(url).send().await.map_err(rede)?;
        let status = resp.status();
        let corpo = resp.bytes().await.map_err(rede)?.to_vec();
        if !status.is_success() {
            return Err(ApiError::from_body(status.as_u16(), &corpo));
        }
        serde_json::from_slice(&corpo).map_err(|e| ApiError::Protocol(e.to_string()))
    }

    /// `POST /v1/sessoes`; inicia a tarefa de renovação automática (ver `refresh`).
    pub async fn login(base: &str, req: LoginRequest) -> Result<Self, ApiError> {
        Self::login_with_clock(base, req, Arc::new(super::refresh::SystemClock)).await
    }

    /// Como `login`, com o relógio da renovação dado (testes). `login` usa `SystemClock`.
    pub(crate) async fn login_with_clock(
        base: &str,
        req: LoginRequest,
        clock: Arc<dyn super::refresh::RenewClock>,
    ) -> Result<Self, ApiError> {
        let base_url = sem_barra(base).to_string();
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(rede)?;
        let resp = http
            .post(format!("{base_url}/v1/sessoes"))
            .json(&req)
            .send()
            .await
            .map_err(rede)?;
        let status = resp.status();
        let corpo = resp.bytes().await.map_err(rede)?.to_vec();
        if status.as_u16() != 201 {
            // 2xx inesperado também é falha: o contrato do login é 201.
            if !status.is_success() {
                return Err(ApiError::from_body(status.as_u16(), &corpo));
            }
            return Err(ApiError::Protocol(format!("status inesperado no login: {status}")));
        }
        let sessao: SessionResponse = serde_json::from_slice(&corpo).map_err(|e| ApiError::Protocol(e.to_string()))?;
        let inner = Arc::new(Inner {
            base_url,
            http,
            session: Mutex::new(Session {
                token: sessao.token,
                expires_at: sessao.expires_at,
                participant_id: sessao.participant_id,
            }),
        });
        tokio::spawn(renovacao_automatica(Arc::downgrade(&inner), clock));
        Ok(Self { inner })
    }

    /// Token atual.
    pub fn token(&self) -> String {
        self.inner.session.lock().token.clone()
    }

    /// Id do participante dado pelo servidor no login.
    pub fn participant_id(&self) -> String {
        self.inner.session.lock().participant_id.clone()
    }

    /// `GET /v1/salas`: lista de salas visíveis.
    pub async fn rooms(&self) -> Result<Vec<RoomListing>, ApiError> {
        let lista: RoomListResponse = self.call(reqwest::Method::GET, "/salas", None::<&()>).await?;
        Ok(lista.rooms)
    }

    /// `POST /v1/salas`: cria a sala e devolve o formato da lista.
    pub async fn create_room(&self, req: CreateRoomRequest) -> Result<RoomListing, ApiError> {
        let resposta: CreateRoomResponse = self.call(reqwest::Method::POST, "/salas", Some(&req)).await?;
        Ok(resposta.room)
    }

    /// `GET /v1/salas/{codigo}`: detalhes, com participantes quando a sala é aberta.
    pub async fn room(&self, code: &RoomCode) -> Result<RoomDetails, ApiError> {
        let caminho = format!("/salas/{}", code.as_str());
        self.call(reqwest::Method::GET, &caminho, None::<&()>).await
    }

    /// `POST /v1/salas/{codigo}/entrar`. A senha vai no corpo, nunca na URL.
    pub async fn join(&self, code: &RoomCode, password: Option<&str>) -> Result<JoinResponse, ApiError> {
        let corpo = JoinRequest {
            password: password.map(str::to_string),
        };
        let caminho = format!("/salas/{}/entrar", code.as_str());
        self.call(reqwest::Method::POST, &caminho, Some(&corpo)).await
    }

    /// `POST /v1/salas/{codigo}/sair`: resposta 204.
    pub async fn leave(&self, code: &RoomCode) -> Result<(), ApiError> {
        let caminho = format!("/salas/{}/sair", code.as_str());
        self.call_empty(reqwest::Method::POST, &caminho, None::<&()>).await
    }

    /// `POST /v1/salas/{codigo}/previa`: ingresso e ponte para pré-visualizar a sala.
    pub async fn preview(&self, code: &RoomCode) -> Result<PreviewResponse, ApiError> {
        let caminho = format!("/salas/{}/previa", code.as_str());
        self.call(reqwest::Method::POST, &caminho, None::<&()>).await
    }

    /// `DELETE /v1/salas/{codigo}`: resposta 204.
    pub async fn delete_room(&self, code: &RoomCode) -> Result<(), ApiError> {
        let caminho = format!("/salas/{}", code.as_str());
        self.call_empty(reqwest::Method::DELETE, &caminho, None::<&()>).await
    }

    /// `GET /v1/diagnostico`: IP público e estado do protocolo visto pelo servidor.
    pub async fn diagnostics(&self) -> Result<ServerDiagnostics, ApiError> {
        self.call(reqwest::Method::GET, "/diagnostico", None::<&()>).await
    }

    /// Abre o WebSocket `GET /v1/ws?token=…` (ver `ws`). Se o token venceu (`nao_autenticado`),
    /// renova uma vez e tenta de novo; a segunda falha volta como está.
    pub async fn connect_ws(&self) -> Result<(WsTx, WsRx), ApiError> {
        match super::ws::connect(&self.inner.base_url, &self.token()).await {
            Err(e) if e.code() == Some("nao_autenticado") => {
                self.renew().await?;
                super::ws::connect(&self.inner.base_url, &self.token()).await
            }
            resultado => resultado,
        }
    }

    /// Chamada autenticada genérica: `Authorization: Bearer`, corpo JSON opcional, resposta JSON.
    /// Status fora de 2xx vira `ApiError::from_body`. Falha de transporte = `Network`;
    /// JSON inválido = `Protocol`.
    pub(crate) async fn call<B: Serialize, R: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<R, ApiError> {
        let corpo = self.envia(method, path, body).await?;
        serde_json::from_slice(&corpo).map_err(|e| ApiError::Protocol(e.to_string()))
    }

    /// Como `call`, para respostas sem corpo (204).
    pub(crate) async fn call_empty<B: Serialize>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<(), ApiError> {
        self.envia(method, path, body).await.map(|_| ())
    }

    /// Envia a requisição autenticada e devolve o corpo bruto de uma resposta 2xx.
    /// O 401 `nao_autenticado` só é devolvido aqui; a renovação fica com `renew` (api-05).
    async fn envia<B: Serialize>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<Vec<u8>, ApiError> {
        // Copia o token e solta o lock antes de qualquer `await`.
        let token = self.inner.session.lock().token.clone();
        let url = format!("{}/v1{}", sem_barra(&self.inner.base_url), path);
        let mut req = self.inner.http.request(method, url).bearer_auth(token);
        if let Some(corpo) = body {
            req = req.json(corpo);
        }
        let resp = req.send().await.map_err(rede)?;
        let status = resp.status();
        let corpo = resp.bytes().await.map_err(rede)?.to_vec();
        if !status.is_success() {
            return Err(ApiError::from_body(status.as_u16(), &corpo));
        }
        Ok(corpo)
    }

    /// `POST /v1/sessoes/renovar` e troca do token em `inner.session`.
    pub(crate) async fn renew(&self) -> Result<(), ApiError> {
        let nova: SessionResponse = self
            .call(reqwest::Method::POST, "/sessoes/renovar", None::<&()>)
            .await?;
        // Um único lock: quem lê vê sempre token, validade e id da mesma sessão.
        *self.inner.session.lock() = Session {
            token: nova.token,
            expires_at: nova.expires_at,
            participant_id: nova.participant_id,
        };
        Ok(())
    }
}

#[cfg(test)]
fn agora_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Tarefa de fundo: renova o token antes de expirar. Só guarda um `Weak`, então termina
/// quando o último cliente for descartado.
async fn renovacao_automatica(fraco: std::sync::Weak<Inner>, clock: Arc<dyn super::refresh::RenewClock>) {
    const ESPERA_INICIAL: Duration = Duration::from_secs(30);
    const ESPERA_MAXIMA: Duration = Duration::from_secs(300);
    // Sem data legível, assume 23 h.
    const SEM_DATA: Duration = Duration::from_secs(23 * 3600);
    let mut backoff = ESPERA_INICIAL;
    let mut falhou = false;
    loop {
        let espera = if falhou {
            let atual = backoff;
            backoff = (backoff * 2).min(ESPERA_MAXIMA);
            atual
        } else {
            let Some(inner) = fraco.upgrade() else { return };
            let expira = inner.session.lock().expires_at.clone();
            drop(inner);
            match super::refresh::parse_iso_utc(&expira) {
                Some(e) => super::refresh::renew_delay(e, clock.now_unix()),
                None => SEM_DATA,
            }
        };
        clock.sleep(espera).await;
        let Some(inner) = fraco.upgrade() else { return };
        let cliente = ServerClient { inner };
        match cliente.renew().await {
            Ok(()) => {
                falhou = false;
                backoff = ESPERA_INICIAL;
            }
            Err(e) => {
                tracing::warn!("falha ao renovar o token: {e}");
                falhou = true;
                // Backoff dobrado a cada falha, limitado a 5 min.
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{extract::State, Json};
    use axum::{
        http::{header::AUTHORIZATION, HeaderMap, StatusCode},
        routing::{delete, get, post},
        Router,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::net::TcpListener;

    /// Sobe o app numa porta livre de loopback e devolve a base `http://127.0.0.1:porta`.
    async fn sobe(app: Router) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    /// Cliente de teste com o token dado (monta `Inner` direto, como `login` faria).
    fn cliente(base: &str, token: &str) -> ServerClient {
        ServerClient {
            inner: Arc::new(Inner {
                base_url: base.to_string(),
                http: reqwest::Client::new(),
                session: Mutex::new(Session {
                    token: token.to_string(),
                    expires_at: "2026-11-05T21:30:00Z".to_string(),
                    participant_id: "p_teste".to_string(),
                }),
            }),
        }
    }

    /// Ecoa o `Authorization` recebido como string JSON.
    async fn eco(headers: HeaderMap) -> String {
        let auth = headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        serde_json::to_string(&auth).unwrap()
    }

    async fn recusada() -> (StatusCode, &'static str) {
        (StatusCode::FORBIDDEN, r#"{"erro":"senha_sala","mensagem":"x"}"#)
    }

    async fn sem_conteudo() -> StatusCode {
        StatusCode::NO_CONTENT
    }

    #[tokio::test]
    async fn call_envia_bearer() {
        let app = Router::new().route("/v1/eco", get(eco));
        let base = sobe(app).await;
        let c = cliente(&base, "tok-123");
        let recebido: String = c.call(reqwest::Method::GET, "/eco", None::<&()>).await.unwrap();
        assert_eq!(recebido, "Bearer tok-123");
    }

    #[tokio::test]
    async fn call_erro_http_mapeia_codigo() {
        let app = Router::new().route("/v1/sala", post(recusada));
        let base = sobe(app).await;
        let c = cliente(&base, "t");
        let r: Result<String, ApiError> = c
            .call(reqwest::Method::POST, "/sala", Some(&serde_json::json!({})))
            .await;
        assert_eq!(
            r,
            Err(ApiError::Http {
                status: 403,
                code: "senha_sala".into(),
                message: "x".into(),
            })
        );
    }

    #[tokio::test]
    async fn call_empty_204() {
        let app = Router::new().route("/v1/sair", post(sem_conteudo));
        let base = sobe(app).await;
        let c = cliente(&base, "t");
        c.call_empty::<()>(reqwest::Method::POST, "/sair", None).await.unwrap();
    }

    fn iso(unix: u64) -> String {
        // Inverso de days_from_civil (Hinnant), só para os testes.
        let dias = (unix / 86_400) as i64;
        let seg = unix % 86_400;
        let z = dias + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = yoe + era * 400 + i64::from(m <= 2);
        format!(
            "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
            seg / 3600,
            seg % 3600 / 60,
            seg % 60
        )
    }

    fn pedido() -> LoginRequest {
        LoginRequest {
            name: "Heitor".into(),
            color: "#3BF08A".into(),
            install_id: "i1".into(),
            protocol: 4,
            app_version: "3.0.0".into(),
            server_password: None,
            account_token: None,
        }
    }

    #[derive(Clone)]
    struct Estado {
        expira: String,
        renovacoes: Arc<AtomicUsize>,
    }

    async fn h_login(State(e): State<Estado>) -> (StatusCode, Json<serde_json::Value>) {
        (
            StatusCode::CREATED,
            Json(serde_json::json!({"token":"t0","expira_em":e.expira,"participante_id":"p_1"})),
        )
    }

    async fn h_renovar(State(e): State<Estado>, headers: HeaderMap) -> Json<serde_json::Value> {
        let n = e.renovacoes.fetch_add(1, Ordering::SeqCst) + 1;
        assert_eq!(
            headers.get(AUTHORIZATION).and_then(|v| v.to_str().ok()),
            Some(if n == 1 { "Bearer t0" } else { "Bearer t1" })
        );
        Json(serde_json::json!({"token":"t1","expira_em":e.expira,"participante_id":"p_2"}))
    }

    fn app_sessoes(expira: String) -> (Router, Arc<AtomicUsize>) {
        let renovacoes = Arc::new(AtomicUsize::new(0));
        let estado = Estado {
            expira,
            renovacoes: renovacoes.clone(),
        };
        let app = Router::new()
            .route("/v1/sessoes", post(h_login))
            .route("/v1/sessoes/renovar", post(h_renovar))
            .with_state(estado);
        (app, renovacoes)
    }

    #[tokio::test]
    async fn login_ok_guarda_token() {
        let (app, _) = app_sessoes("2099-01-01T00:00:00Z".into());
        let base = sobe(app).await;
        let c = ServerClient::login(&format!("{base}/"), pedido()).await.unwrap();
        assert_eq!(c.token(), "t0");
        assert_eq!(c.participant_id(), "p_1");
        assert_eq!(c.inner.base_url, base);
    }

    #[tokio::test]
    async fn login_senha_servidor_errada() {
        async fn nega() -> (StatusCode, &'static str) {
            (StatusCode::FORBIDDEN, r#"{"erro":"senha_servidor","mensagem":"x"}"#)
        }
        let base = sobe(Router::new().route("/v1/sessoes", post(nega))).await;
        let r = ServerClient::login(&base, pedido()).await;
        match r {
            Err(ApiError::Http { code, .. }) => assert_eq!(code, "senha_servidor"),
            outro => panic!("inesperado: {outro:?}"),
        }
    }

    #[tokio::test]
    async fn renew_troca_token() {
        let (app, n) = app_sessoes("2099-01-01T00:00:00Z".into());
        let base = sobe(app).await;
        let c = ServerClient::login(&base, pedido()).await.unwrap();
        c.renew().await.unwrap();
        assert_eq!(c.token(), "t1");
        assert_eq!(c.participant_id(), "p_2");
        assert_eq!(n.load(Ordering::SeqCst), 1);
    }

    /// Relógio de teste: hora real; `sleep` avisa a duração pelo canal e espera uma permissão.
    struct RelogioTeste {
        tx: tokio::sync::mpsc::UnboundedSender<Duration>,
        libera: Arc<tokio::sync::Semaphore>,
    }

    impl super::super::refresh::RenewClock for RelogioTeste {
        fn now_unix(&self) -> u64 {
            agora_unix()
        }
        fn sleep(&self, d: Duration) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
            let _ = self.tx.send(d);
            let libera = self.libera.clone();
            Box::pin(async move {
                if let Ok(p) = libera.acquire().await {
                    p.forget();
                }
            })
        }
    }

    fn relogio_teste() -> (
        Arc<RelogioTeste>,
        tokio::sync::mpsc::UnboundedReceiver<Duration>,
        Arc<tokio::sync::Semaphore>,
    ) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let libera = Arc::new(tokio::sync::Semaphore::new(0));
        (
            Arc::new(RelogioTeste {
                tx,
                libera: libera.clone(),
            }),
            rx,
            libera,
        )
    }

    async fn recebe(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Duration>) -> Option<Duration> {
        tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("tempo esgotado esperando o relógio")
    }

    #[tokio::test]
    async fn renovacao_automatica_dispara() {
        let expira = iso(agora_unix() + 3600 + 10);
        let (app, n) = app_sessoes(expira);
        let base = sobe(app).await;
        let (relogio, mut rx, libera) = relogio_teste();
        let c = ServerClient::login_with_clock(&base, pedido(), relogio).await.unwrap();
        let d1 = recebe(&mut rx).await.unwrap();
        assert!(d1 >= Duration::from_secs(9) && d1 <= Duration::from_secs(10), "{d1:?}");
        assert_eq!(n.load(Ordering::SeqCst), 0);
        libera.add_permits(1);
        // O segundo sleep só começa depois que a renovação terminou.
        let d2 = recebe(&mut rx).await.unwrap();
        assert_eq!(n.load(Ordering::SeqCst), 1);
        assert_eq!(c.token(), "t1");
        assert!(d2 >= Duration::from_secs(9) && d2 <= Duration::from_secs(10), "{d2:?}");
    }

    #[tokio::test]
    async fn tarefa_termina_ao_descartar_cliente() {
        let expira = iso(agora_unix() + 3600 + 10);
        let (app, n) = app_sessoes(expira);
        let base = sobe(app).await;
        let (relogio, mut rx, libera) = relogio_teste();
        let c = ServerClient::login_with_clock(&base, pedido(), relogio).await.unwrap();
        recebe(&mut rx).await.unwrap();
        drop(c);
        libera.add_permits(1);
        // A tarefa termina e solta o relógio (e o `Sender`): o canal fecha.
        assert!(recebe(&mut rx).await.is_none());
        assert_eq!(n.load(Ordering::SeqCst), 0);
    }

    /// Sala no formato da lista, com os campos obrigatórios do contrato.
    fn sala_json(codigo: &str, nome: &str) -> serde_json::Value {
        serde_json::json!({
            "id": "s_teste1",
            "codigo": codigo,
            "nome": nome,
            "dono": { "id": "p_1", "nome": "Ju", "cor": "#7CF2C0" },
            "acesso": "aberta",
            "criada_em": "2026-11-04T21:30:00Z",
            "participantes": 1
        })
    }

    /// Guarda o último corpo recebido, para o teste conferir o que o cliente enviou.
    type Visto = Arc<Mutex<Option<serde_json::Value>>>;

    async fn h_salas_lista() -> Json<serde_json::Value> {
        Json(serde_json::json!({
            "salas": [sala_json("ZAP-7K2Q", "Sexta"), sala_json("ZAP-2345", "Outra")]
        }))
    }

    async fn h_criar_sala(
        State(visto): State<Visto>,
        Json(corpo): Json<serde_json::Value>,
    ) -> (StatusCode, Json<serde_json::Value>) {
        let nome = corpo["nome"].as_str().unwrap_or("").to_string();
        *visto.lock() = Some(corpo);
        (
            StatusCode::CREATED,
            Json(serde_json::json!({ "sala": sala_json("ZAP-7K2Q", &nome) })),
        )
    }

    /// Aceita só a senha "abc"; qualquer outra vira 403 `senha_sala`.
    async fn h_entrar(Json(corpo): Json<serde_json::Value>) -> (StatusCode, Json<serde_json::Value>) {
        if corpo.get("senha").and_then(|s| s.as_str()) != Some("abc") {
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({ "erro": "senha_sala", "mensagem": "Senha incorreta." })),
            );
        }
        (
            StatusCode::OK,
            Json(serde_json::json!({
                "sala": sala_json("ZAP-7K2Q", "Sexta"),
                "participante_id": "p_3",
                "ingresso": "ing-1",
                "ingresso_expira_em": "2026-11-04T22:30:00Z",
                "midia": { "direta_permitida": false }
            })),
        )
    }

    async fn h_previa() -> Json<serde_json::Value> {
        Json(serde_json::json!({
            "ingresso": "ing-previa",
            "expira_em": "2026-11-04T22:30:00Z",
            "midia": { "ponte": { "endereco": "zap.exemplo:7443", "alpn": "zap/4" } }
        }))
    }

    async fn h_diagnostico() -> Json<serde_json::Value> {
        Json(serde_json::json!({
            "ip_publico": "177.12.34.56",
            "porta_publica_udp": 51234,
            "protocolo_ok": true,
            "agora": "2026-11-04T21:30:00Z"
        }))
    }

    async fn h_detalhes() -> Json<serde_json::Value> {
        let mut sala = sala_json("ZAP-7K2Q", "Sexta");
        sala["participantes_lista"] = serde_json::json!([
            { "id": "p_1", "nome": "Ju", "cor": "#7CF2C0", "compartilhando": ["camera"] }
        ]);
        Json(sala)
    }

    #[tokio::test]
    async fn rooms_lista() {
        let base = sobe(Router::new().route("/v1/salas", get(h_salas_lista))).await;
        let c = cliente(&base, "t");
        let salas = c.rooms().await.unwrap();
        assert_eq!(salas.len(), 2);
        assert_eq!(salas[0].code.as_str(), "ZAP-7K2Q");
        assert_eq!(salas[1].name, "Outra");
    }

    #[tokio::test]
    async fn create_room_envia_corpo_e_devolve_sala() {
        use zap_proto::Access;
        let visto: Visto = Default::default();
        let app = Router::new()
            .route("/v1/salas", post(h_criar_sala))
            .with_state(visto.clone());
        let base = sobe(app).await;
        let c = cliente(&base, "t");
        let pedido = CreateRoomRequest {
            name: "Sala nova".into(),
            access: Access::Password,
            password: Some("1234".into()),
        };
        let sala = c.create_room(pedido).await.unwrap();
        assert_eq!(sala.name, "Sala nova");
        let enviado = visto.lock().clone().unwrap();
        assert_eq!(enviado["nome"], "Sala nova");
        assert_eq!(enviado["acesso"], "senha");
        assert_eq!(enviado["senha"], "1234");
    }

    #[tokio::test]
    async fn join_com_senha_envia_senha() {
        let base = sobe(Router::new().route("/v1/salas/ZAP-7K2Q/entrar", post(h_entrar))).await;
        let c = cliente(&base, "t");
        let code = RoomCode::parse("ZAP-7K2Q").unwrap();
        let r = c.join(&code, Some("abc")).await.unwrap();
        assert_eq!(r.room.code.as_str(), "ZAP-7K2Q");
        assert_eq!(r.participant_id, "p_3");
        assert_eq!(r.ticket, "ing-1");
    }

    #[tokio::test]
    async fn join_senha_errada_http_senha_sala() {
        let base = sobe(Router::new().route("/v1/salas/ZAP-7K2Q/entrar", post(h_entrar))).await;
        let c = cliente(&base, "t");
        let code = RoomCode::parse("ZAP-7K2Q").unwrap();
        let r = c.join(&code, Some("errada")).await;
        assert_eq!(
            r,
            Err(ApiError::Http {
                status: 403,
                code: "senha_sala".into(),
                message: "Senha incorreta.".into(),
            })
        );
    }

    #[tokio::test]
    async fn leave_204() {
        let base = sobe(Router::new().route("/v1/salas/ZAP-7K2Q/sair", post(sem_conteudo))).await;
        let c = cliente(&base, "t");
        let code = RoomCode::parse("ZAP-7K2Q").unwrap();
        c.leave(&code).await.unwrap();
    }

    #[tokio::test]
    async fn preview_ok() {
        let base = sobe(Router::new().route("/v1/salas/ZAP-7K2Q/previa", post(h_previa))).await;
        let c = cliente(&base, "t");
        let code = RoomCode::parse("ZAP-7K2Q").unwrap();
        let r = c.preview(&code).await.unwrap();
        assert_eq!(r.ticket, "ing-previa");
        assert_eq!(r.media.bridge.address, "zap.exemplo:7443");
    }

    #[tokio::test]
    async fn diagnostics_ok() {
        let base = sobe(Router::new().route("/v1/diagnostico", get(h_diagnostico))).await;
        let c = cliente(&base, "t");
        let d = c.diagnostics().await.unwrap();
        assert_eq!(d.public_ip, "177.12.34.56");
        assert_eq!(d.public_udp_port, Some(51234));
        assert!(d.protocol_ok);
    }

    #[tokio::test]
    async fn room_detalhes() {
        let base = sobe(Router::new().route("/v1/salas/ZAP-7K2Q", get(h_detalhes))).await;
        let c = cliente(&base, "t");
        let code = RoomCode::parse("ZAP-7K2Q").unwrap();
        let d = c.room(&code).await.unwrap();
        assert_eq!(d.room.name, "Sexta");
        let participantes = d.participant_list.unwrap();
        assert_eq!(participantes[0].name, "Ju");
        assert_eq!(participantes[0].sharing, vec!["camera".to_string()]);
    }

    #[tokio::test]
    async fn delete_room_204() {
        let base = sobe(Router::new().route("/v1/salas/ZAP-7K2Q", delete(sem_conteudo))).await;
        let c = cliente(&base, "t");
        let code = RoomCode::parse("ZAP-7K2Q").unwrap();
        c.delete_room(&code).await.unwrap();
    }

    #[derive(Clone)]
    struct EstadoRenova {
        /// Token que o WebSocket aceita; qualquer outro recebe `nao_autenticado`.
        aceito: &'static str,
        /// Token devolvido pela renovação.
        novo: &'static str,
        renovacoes: Arc<AtomicUsize>,
    }

    async fn renova_ws(
        State(e): State<EstadoRenova>,
        ws: axum::extract::ws::WebSocketUpgrade,
        axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
    ) -> axum::response::Response {
        use axum::response::IntoResponse;
        if q.get("token").map(String::as_str) != Some(e.aceito) {
            let corpo = Json(serde_json::json!({"erro": "nao_autenticado", "mensagem": "token vencido"}));
            return (StatusCode::UNAUTHORIZED, corpo).into_response();
        }
        ws.on_upgrade(|_| async {}).into_response()
    }

    async fn renova_renovar(State(e): State<EstadoRenova>) -> Json<serde_json::Value> {
        e.renovacoes.fetch_add(1, Ordering::SeqCst);
        Json(serde_json::json!({
            "token": e.novo,
            "expira_em": "2026-11-05T21:30:00Z",
            "participante_id": "p_teste",
        }))
    }

    /// Sobe `/v1/ws` e `/v1/sessoes/renovar` com o token aceito e o novo dados.
    async fn sobe_renova(aceito: &'static str, novo: &'static str) -> (String, Arc<AtomicUsize>) {
        let renovacoes = Arc::new(AtomicUsize::new(0));
        let estado = EstadoRenova {
            aceito,
            novo,
            renovacoes: renovacoes.clone(),
        };
        let app = Router::new()
            .route("/v1/ws", get(renova_ws))
            .route("/v1/sessoes/renovar", post(renova_renovar))
            .with_state(estado);
        (sobe(app).await, renovacoes)
    }

    #[tokio::test]
    async fn connect_ws_renova_token_quando_nao_autenticado() {
        // Token velho é recusado; a renovação troca por `tok-novo`, que o WebSocket aceita.
        let (base, renovacoes) = sobe_renova("tok-novo", "tok-novo").await;
        let c = cliente(&base, "tok-velho");
        let _ws = c.connect_ws().await.unwrap();
        assert_eq!(renovacoes.load(Ordering::SeqCst), 1);
        assert_eq!(c.token(), "tok-novo");

        // Se a segunda tentativa também falhar, o erro volta sem nova renovação.
        let (base, renovacoes) = sobe_renova("nunca", "tok-velho").await;
        let c = cliente(&base, "tok-velho");
        let r = c.connect_ws().await;
        assert!(
            matches!(&r, Err(e) if e.code() == Some("nao_autenticado")),
            "veio {r:?}"
        );
        assert_eq!(renovacoes.load(Ordering::SeqCst), 1);
    }
}
