//! Binário do servidor de teste: sobe `spawn(Config::default())`, imprime os endereços e espera
//! Ctrl-C. Variável opcional `ZAP_TEST_PASSWORD` define `server_password`.

use tracing_subscriber::EnvFilter;
use zap_test_server::{spawn, Config};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let config = Config {
        server_password: std::env::var("ZAP_TEST_PASSWORD").ok().filter(|s| !s.is_empty()),
        ..Config::default()
    };
    let handle = match spawn(config).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("não foi possível subir o servidor de teste: {e}");
            std::process::exit(1);
        }
    };
    println!("REST/WS: {}", handle.base_url);
    println!("QUIC: {}", handle.quic_addr);
    println!("Impressão digital (--dev-cert-fp): {}", handle.cert_fingerprint);

    if let Err(e) = tokio::signal::ctrl_c().await {
        eprintln!("falha ao esperar Ctrl-C: {e}");
    }
}
