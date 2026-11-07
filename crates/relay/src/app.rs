use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use std::sync::Arc;

use crate::cache::Cache;
use crate::config::Config;
use crate::rate_limit::RateLimiter;

const CLIENT_LIMIT: u32 = 30;
const CLIENT_WINDOW_SECONDS: i64 = 60;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub cache: Arc<Cache>,
    pub limiter: Arc<RateLimiter>,
}

impl AppState {
    pub fn new(config: Config, cache: Cache) -> Self {
        Self {
            config: Arc::new(config),
            cache: Arc::new(cache),
            limiter: Arc::new(RateLimiter::new(CLIENT_LIMIT, CLIENT_WINDOW_SECONDS)),
        }
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .with_state(state)
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
    tmdb: bool,
    igdb: bool,
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        tmdb: state.config.tmdb_configured(),
        igdb: state.config.igdb_configured(),
    })
}

#[cfg(test)]
mod tests {
    use super::{AppState, HealthResponse, health};
    use crate::cache::Cache;
    use crate::config::Config;
    use axum::extract::State;

    fn state(values: &[(&str, &str)]) -> AppState {
        let values: std::collections::HashMap<String, String> = values
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        let config = Config::from_lookup(|name| values.get(name).cloned()).unwrap();
        let cache = Cache::open_in_memory().unwrap();

        AppState::new(config, cache)
    }

    #[tokio::test]
    async fn health_reports_missing_sources() {
        let HealthResponse {
            status,
            version,
            tmdb,
            igdb,
        } = health(State(state(&[]))).await.0;

        assert_eq!(status, "ok");
        assert_eq!(version, env!("CARGO_PKG_VERSION"));
        assert!(!tmdb);
        assert!(!igdb);
    }

    #[tokio::test]
    async fn health_reports_configured_sources() {
        let HealthResponse { tmdb, igdb, .. } = health(State(state(&[
            ("TMDB_TOKEN", "tmdb-secret"),
            ("IGDB_CLIENT_ID", "igdb-id"),
            ("IGDB_CLIENT_SECRET", "igdb-secret"),
        ])))
        .await
        .0;

        assert!(tmdb);
        assert!(igdb);
    }
}
