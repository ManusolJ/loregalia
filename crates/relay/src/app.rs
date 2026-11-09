use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::HeaderMap;
use axum::routing::get;
use axum::{Json, Router};
use loregalia_core::providers::Candidate;
use loregalia_core::providers::relay::SearchResponse;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use crate::cache::Cache;
use crate::config::Config;
use crate::error::RelayError;
use crate::rate_limit::RateLimiter;
use crate::tmdb::{TmdbClient, TmdbUpstream};

const CLIENT_LIMIT: u32 = 30;
const CLIENT_WINDOW_SECONDS: i64 = 60;
const SEARCH_TTL_SECONDS: i64 = 60 * 60;
const DETAILS_TTL_SECONDS: i64 = 30 * 24 * 60 * 60;
const MAX_CACHE_ENTRIES: usize = 5_000;
const MAX_QUERY_CHARS: usize = 100;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub cache: Arc<Cache>,
    pub limiter: Arc<RateLimiter>,
    pub tmdb: Option<Arc<dyn TmdbUpstream>>,
}

impl AppState {
    pub fn new(config: Config, cache: Cache) -> Self {
        let tmdb = config
            .tmdb_token
            .clone()
            .map(|token| Arc::new(TmdbClient::new(token)) as Arc<dyn TmdbUpstream>);

        Self::with_tmdb(config, cache, tmdb)
    }

    pub fn with_tmdb(config: Config, cache: Cache, tmdb: Option<Arc<dyn TmdbUpstream>>) -> Self {
        Self {
            config: Arc::new(config),
            cache: Arc::new(cache),
            limiter: Arc::new(RateLimiter::new(CLIENT_LIMIT, CLIENT_WINDOW_SECONDS)),
            tmdb,
        }
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/v1/movies/search", get(search_movies))
        .route("/v1/movies/{id}", get(movie_details))
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

#[derive(Debug, Deserialize)]
struct SearchParams {
    q: String,
}

async fn search_movies(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(params): Query<SearchParams>,
) -> Result<Json<SearchResponse>, RelayError> {
    let query = params.q.trim().to_owned();
    if query.is_empty() || query.chars().count() > MAX_QUERY_CHARS {
        return Err(RelayError::BadRequest);
    }

    let now = crate::cache::now();
    check_rate(
        &state,
        client_ip(peer, &headers, state.config.trust_proxy),
        now,
    )?;

    let key = format!("tmdb:search:{}", query.to_lowercase());
    if let Some(body) = state.cache.get(&key, now).map_err(|_| RelayError::Cache)?
        && let Ok(response) = serde_json::from_str::<SearchResponse>(&body)
    {
        return Ok(Json(response));
    }

    let upstream = state.tmdb.as_ref().ok_or(RelayError::NotConfigured)?;
    let results = upstream.search(query).await.map_err(RelayError::Upstream)?;
    let response = SearchResponse { results };
    let body = serde_json::to_string(&response).map_err(|_| RelayError::Internal)?;
    state
        .cache
        .put(
            &key,
            &body,
            now + SEARCH_TTL_SECONDS,
            now,
            MAX_CACHE_ENTRIES,
        )
        .map_err(|_| RelayError::Cache)?;

    Ok(Json(response))
}

async fn movie_details(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Candidate>, RelayError> {
    if id.is_empty() || id.len() > 20 || !id.chars().all(|character| character.is_ascii_digit()) {
        return Err(RelayError::BadRequest);
    }

    let now = crate::cache::now();
    check_rate(
        &state,
        client_ip(peer, &headers, state.config.trust_proxy),
        now,
    )?;

    let key = format!("tmdb:movie:{id}");
    if let Some(body) = state.cache.get(&key, now).map_err(|_| RelayError::Cache)?
        && let Ok(movie) = serde_json::from_str::<Candidate>(&body)
    {
        return Ok(Json(movie));
    }

    let upstream = state.tmdb.as_ref().ok_or(RelayError::NotConfigured)?;
    let movie = upstream.details(id).await.map_err(RelayError::Upstream)?;
    let body = serde_json::to_string(&movie).map_err(|_| RelayError::Internal)?;
    state
        .cache
        .put(
            &key,
            &body,
            now + DETAILS_TTL_SECONDS,
            now,
            MAX_CACHE_ENTRIES,
        )
        .map_err(|_| RelayError::Cache)?;

    Ok(Json(movie))
}

fn check_rate(state: &AppState, client: IpAddr, now: i64) -> Result<(), RelayError> {
    if state.limiter.check(client, now) {
        Ok(())
    } else {
        Err(RelayError::RateLimited)
    }
}

fn client_ip(peer: SocketAddr, headers: &HeaderMap, trust_proxy: bool) -> IpAddr {
    if trust_proxy
        && peer.ip().is_loopback()
        && let Some(value) = headers
            .get("x-forwarded-for")
            .and_then(|value| value.to_str().ok())
        && let Some(client) = value
            .split(',')
            .next()
            .and_then(|value| value.trim().parse::<IpAddr>().ok())
    {
        return client;
    }

    peer.ip()
}

#[cfg(test)]
mod tests {
    use super::{
        AppState, HealthResponse, MAX_QUERY_CHARS, SearchParams, client_ip, health, movie_details,
        search_movies,
    };
    use crate::cache::Cache;
    use crate::config::Config;
    use crate::error::RelayError;
    use crate::tmdb::{TmdbUpstream, UpstreamResult};
    use axum::Json;
    use axum::extract::{ConnectInfo, Path, Query, State};
    use axum::http::HeaderMap;
    use loregalia_core::Error;
    use loregalia_core::providers::relay::SearchResponse;
    use loregalia_core::providers::{Candidate, Source};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::sync::{Arc, Mutex};

    fn config(values: &[(&str, &str)]) -> Config {
        let values: std::collections::HashMap<String, String> = values
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        Config::from_lookup(|name| values.get(name).cloned()).unwrap()
    }

    fn candidate() -> Candidate {
        Candidate {
            source: Source::Tmdb,
            external_id: "438631".to_owned(),
            title: "Dune".to_owned(),
            release_year: Some(2021),
            cover_url: Some("https://image.example/dune.jpg".to_owned()),
        }
    }

    #[derive(Clone, Copy)]
    enum FakeError {
        Unauthorized,
    }

    struct FakeTmdb {
        searches: Mutex<usize>,
        details: Mutex<usize>,
        search_error: Option<FakeError>,
        detail_error: Option<FakeError>,
    }

    impl FakeTmdb {
        fn ok() -> Self {
            Self {
                searches: Mutex::new(0),
                details: Mutex::new(0),
                search_error: None,
                detail_error: None,
            }
        }

        fn with_search_error(error: FakeError) -> Self {
            Self {
                search_error: Some(error),
                ..Self::ok()
            }
        }
    }

    impl TmdbUpstream for FakeTmdb {
        fn search(&self, _query: String) -> UpstreamResult<Vec<Candidate>> {
            let mut calls = self.searches.lock().unwrap();
            *calls += 1;
            let error = self.search_error;
            Box::pin(async move {
                match error {
                    Some(FakeError::Unauthorized) => Err(Error::Unauthorized),
                    None => Ok(vec![candidate()]),
                }
            })
        }

        fn details(&self, _id: String) -> UpstreamResult<Candidate> {
            let mut calls = self.details.lock().unwrap();
            *calls += 1;
            let error = self.detail_error;
            Box::pin(async move {
                match error {
                    Some(FakeError::Unauthorized) => Err(Error::Unauthorized),
                    None => Ok(candidate()),
                }
            })
        }
    }

    fn state_with(tmdb: Option<Arc<dyn TmdbUpstream>>) -> AppState {
        AppState::with_tmdb(
            config(&[("TMDB_TOKEN", "token")]),
            Cache::open_in_memory().unwrap(),
            tmdb,
        )
    }

    fn peer() -> ConnectInfo<SocketAddr> {
        ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 1234)))
    }

    async fn search(state: AppState, query: &str) -> Result<SearchResponse, RelayError> {
        search_movies(
            State(state),
            peer(),
            HeaderMap::new(),
            Query(SearchParams {
                q: query.to_owned(),
            }),
        )
        .await
        .map(|Json(response)| response)
    }

    async fn details(state: AppState, id: &str) -> Result<Candidate, RelayError> {
        movie_details(State(state), peer(), HeaderMap::new(), Path(id.to_owned()))
            .await
            .map(|Json(candidate)| candidate)
    }

    #[tokio::test]
    async fn health_reports_missing_sources() {
        let config = config(&[]);
        let state = AppState::new(config, Cache::open_in_memory().unwrap());
        let HealthResponse {
            status,
            version,
            tmdb,
            igdb,
        } = health(State(state)).await.0;

        assert_eq!(status, "ok");
        assert_eq!(version, env!("CARGO_PKG_VERSION"));
        assert!(!tmdb);
        assert!(!igdb);
    }

    #[tokio::test]
    async fn health_reports_configured_sources() {
        let config = config(&[
            ("TMDB_TOKEN", "tmdb-secret"),
            ("IGDB_CLIENT_ID", "igdb-id"),
            ("IGDB_CLIENT_SECRET", "igdb-secret"),
        ]);
        let state = AppState::new(config, Cache::open_in_memory().unwrap());
        let HealthResponse { tmdb, igdb, .. } = health(State(state)).await.0;

        assert!(tmdb);
        assert!(igdb);
    }

    #[tokio::test]
    async fn search_returns_a_cached_result_and_calls_upstream_once() {
        let fake = Arc::new(FakeTmdb::ok());
        let state = state_with(Some(fake.clone()));

        assert_eq!(
            search(state.clone(), "dune").await.unwrap().results,
            vec![candidate()]
        );
        assert_eq!(
            search(state, "dune").await.unwrap().results,
            vec![candidate()]
        );
        assert_eq!(*fake.searches.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn details_returns_a_cached_candidate_and_calls_upstream_once() {
        let fake = Arc::new(FakeTmdb::ok());
        let state = state_with(Some(fake.clone()));

        assert_eq!(details(state.clone(), "438631").await.unwrap(), candidate());
        assert_eq!(details(state, "438631").await.unwrap(), candidate());
        assert_eq!(*fake.details.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn an_empty_query_is_rejected() {
        let state = state_with(Some(Arc::new(FakeTmdb::ok())));

        assert!(matches!(
            search(state, "   ").await,
            Err(RelayError::BadRequest)
        ));
    }

    #[tokio::test]
    async fn an_overlong_query_is_rejected() {
        let state = state_with(Some(Arc::new(FakeTmdb::ok())));
        let query = "a".repeat(MAX_QUERY_CHARS + 1);

        assert!(matches!(
            search(state, &query).await,
            Err(RelayError::BadRequest)
        ));
    }

    #[tokio::test]
    async fn an_unconfigured_relay_says_so() {
        let state = state_with(None);

        assert!(matches!(
            search(state, "dune").await,
            Err(RelayError::NotConfigured)
        ));
    }

    #[tokio::test]
    async fn an_invalid_movie_id_is_rejected() {
        let state = state_with(Some(Arc::new(FakeTmdb::ok())));

        assert!(matches!(
            details(state, "dune").await,
            Err(RelayError::BadRequest)
        ));
    }

    #[tokio::test]
    async fn an_upstream_failure_is_reported() {
        let state = state_with(Some(Arc::new(FakeTmdb::with_search_error(
            FakeError::Unauthorized,
        ))));

        assert!(matches!(
            search(state, "dune").await,
            Err(RelayError::Upstream(Error::Unauthorized))
        ));
    }

    #[tokio::test]
    async fn the_client_is_rate_limited() {
        let state = state_with(Some(Arc::new(FakeTmdb::ok())));
        for _ in 0..30 {
            let _ = search(state.clone(), "dune").await;
        }

        assert!(matches!(
            search(state, "dune").await,
            Err(RelayError::RateLimited)
        ));
    }

    #[test]
    fn a_proxy_header_is_used_only_when_trusted() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "203.0.113.7, 10.0.0.1".parse().unwrap());
        let peer = SocketAddr::from(([127, 0, 0, 1], 1234));

        assert_eq!(
            client_ip(peer, &headers, true),
            IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7))
        );
        assert_eq!(client_ip(peer, &headers, false), peer.ip());
    }

    #[test]
    fn a_proxy_header_is_ignored_for_a_remote_peer() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "203.0.113.7".parse().unwrap());
        let peer = SocketAddr::from(([192, 0, 2, 1], 1234));

        assert_eq!(client_ip(peer, &headers, true), peer.ip());
    }
}
