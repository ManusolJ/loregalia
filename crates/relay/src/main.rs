use std::process::ExitCode;

use loregalia_relay::{AppState, Cache, Config, router};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("relay failed: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let bind_address = config.bind_address;
    let cache = Cache::open(&config.database_path)?;
    let state = AppState::new(config, cache);
    let listener = TcpListener::bind(bind_address).await?;

    println!("Loregalia relay listening on {bind_address}");
    axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown())
        .await?;

    Ok(())
}

async fn shutdown() {
    let _ = tokio::signal::ctrl_c().await;
}
