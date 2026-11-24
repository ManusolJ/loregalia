use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::HeaderMap;
use axum::routing::get;
use axum::{Json, Router};
use loregalia_core::providers::relay::SearchResponse;
use loregalia_core::providers::{Candidate, igdb};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use crate::cache::Cache;
use crate::coalesce::Coalescer;
use crate::config::Config;
use crate::error::RelayError;
use crate::igdb::{IgdbClient, IgdbUpstream};
use crate::rate_limit::RateLimiter;
use crate::throttle::Throttle;
use crate::tmdb::{TmdbClient, TmdbUpstream};

const CLIENT_LIMIT: u32 = 30;
const CLIENT_WINDOW_SECONDS: i64 = 60;
const DAY_SECONDS: i64 = 24 * 60 * 60;
const SEARCH_TTL_SECONDS: i64 = 7 * DAY_SECONDS;
const DETAILS_TTL_SECONDS: i64 = 30 * DAY_SECONDS;
const DETAILS_VERSION: &str = "v2";
const SEARCH_VERSION: &str = "v2";
const MAX_CACHE_ENTRIES: usize = 20_000;
const MAX_QUERY_CHARS: usize = 100;
const MAX_ID_DIGITS: usize = 20;
const QUEUE_WAIT: Duration = Duration::from_secs(8);
const TMDB_OPEN_REQUESTS: usize = 20;
const TMDB_SPACING: Duration = Duration::from_millis(50);
const IGDB_OPEN_REQUESTS: usize = 8;
const IGDB_SPACING: Duration = Duration::from_millis(250);

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub cache: Arc<Cache>,
    pub limiter: Arc<RateLimiter>,
    pub tmdb: Option<Arc<dyn TmdbUpstream>>,
    pub igdb: Option<Arc<dyn IgdbUpstream>>,
    in_flight: Arc<Coalescer>,
    tmdb_throttle: Arc<Throttle>,
    igdb_throttle: Arc<Throttle>,
}

impl AppState {
    pub fn new(config: Config, cache: Cache) -> Self {
        let tmdb = config
            .tmdb_token
            .clone()
            .map(|token| Arc::new(TmdbClient::new(token)) as Arc<dyn TmdbUpstream>);
        let igdb = config
            .igdb_client_id
            .clone()
            .zip(config.igdb_client_secret.clone())
            .map(|(client_id, client_secret)| {
                Arc::new(IgdbClient::new(client_id, client_secret)) as Arc<dyn IgdbUpstream>
            });

        Self::with_upstreams(config, cache, tmdb, igdb)
    }

    pub fn with_tmdb(config: Config, cache: Cache, tmdb: Option<Arc<dyn TmdbUpstream>>) -> Self {
        Self::with_upstreams(config, cache, tmdb, None)
    }

    pub fn with_upstreams(
        config: Config,
        cache: Cache,
        tmdb: Option<Arc<dyn TmdbUpstream>>,
        igdb: Option<Arc<dyn IgdbUpstream>>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            cache: Arc::new(cache),
            limiter: Arc::new(RateLimiter::new(CLIENT_LIMIT, CLIENT_WINDOW_SECONDS)),
            tmdb,
            igdb,
            in_flight: Arc::new(Coalescer::default()),
            tmdb_throttle: Arc::new(Throttle::new(TMDB_OPEN_REQUESTS, TMDB_SPACING, QUEUE_WAIT)),
            igdb_throttle: Arc::new(Throttle::new(IGDB_OPEN_REQUESTS, IGDB_SPACING, QUEUE_WAIT)),
        }
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/v1/movies/search", get(search_movies))
        .route("/v1/movies/{id}", get(movie_details))
        .route("/v1/games/search", get(search_games))
        .route("/v1/games/{id}", get(game_details))
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

fn normalized_query(raw: &str) -> Option<String> {
    let query = raw
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    (!query.is_empty() && query.chars().count() <= MAX_QUERY_CHARS).then_some(query)
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_ID_DIGITS && id.chars().all(|c| c.is_ascii_digit())
}

async fn search_movies(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(params): Query<SearchParams>,
) -> Result<Json<SearchResponse>, RelayError> {
    let query = normalized_query(&params.q).ok_or(RelayError::BadRequest)?;
    let now = admit(&state, peer, &headers)?;
    let upstream = state.tmdb.clone();
    let throttle = state.tmdb_throttle.clone();

    cached(
        &state,
        format!("tmdb:search:{query}"),
        now,
        SEARCH_TTL_SECONDS,
        async move {
            let upstream = upstream.ok_or(RelayError::NotConfigured)?;
            let results = throttle
                .run(upstream.search(query))
                .await?
                .map_err(RelayError::upstream)?;
            Ok(SearchResponse { results })
        },
    )
    .await
    .map(Json)
}

async fn movie_details(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Candidate>, RelayError> {
    if !valid_id(&id) {
        return Err(RelayError::BadRequest);
    }
    let now = admit(&state, peer, &headers)?;
    let upstream = state.tmdb.clone();
    let throttle = state.tmdb_throttle.clone();

    cached(
        &state,
        format!("tmdb:movie:{DETAILS_VERSION}:{id}"),
        now,
        DETAILS_TTL_SECONDS,
        async move {
            let upstream = upstream.ok_or(RelayError::NotConfigured)?;
            throttle
                .run(upstream.details(id))
                .await?
                .map_err(RelayError::upstream)
        },
    )
    .await
    .map(Json)
}

async fn search_games(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(params): Query<SearchParams>,
) -> Result<Json<SearchResponse>, RelayError> {
    let query = normalized_query(&params.q).ok_or(RelayError::BadRequest)?;
    let now = admit(&state, peer, &headers)?;
    let upstream = state.igdb.clone();
    let throttle = state.igdb_throttle.clone();

    cached(
        &state,
        format!("igdb:search:{SEARCH_VERSION}:{query}"),
        now,
        SEARCH_TTL_SECONDS,
        async move {
            let upstream = upstream.ok_or(RelayError::NotConfigured)?;
            let results = throttle
                .run(upstream.search(query))
                .await?
                .map_err(RelayError::upstream)?;
            Ok(SearchResponse { results })
        },
    )
    .await
    .map(Json)
}

async fn game_details(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Candidate>, RelayError> {
    if !valid_id(&id) {
        return Err(RelayError::BadRequest);
    }
    let now = admit(&state, peer, &headers)?;
    let upstream = state.igdb.clone();
    let throttle = state.igdb_throttle.clone();

    cached(
        &state,
        format!("igdb:game:{DETAILS_VERSION}:{id}"),
        now,
        DETAILS_TTL_SECONDS,
        async move {
            let upstream = upstream.ok_or(RelayError::NotConfigured)?;
            let game = throttle
                .run(upstream.details(id.clone()))
                .await?
                .map_err(RelayError::upstream)?;
            let minutes = throttle
                .run(upstream.time_to_beat(id))
                .await?
                .map_err(RelayError::upstream)?;
            Ok(igdb::with_time(game, minutes))
        },
    )
    .await
    .map(Json)
}

async fn cached<T>(
    state: &AppState,
    key: String,
    now: i64,
    ttl_seconds: i64,
    fetch: impl Future<Output = Result<T, RelayError>>,
) -> Result<T, RelayError>
where
    T: Serialize + DeserializeOwned,
{
    if let Some(body) = cache_get(&state.cache, &key, now).await?
        && let Ok(value) = serde_json::from_str::<T>(&body)
    {
        return Ok(value);
    }

    let cache = state.cache.clone();
    let body = state
        .in_flight
        .run(&key, async {
            let value = fetch.await?;
            let body = serde_json::to_string(&value).map_err(|_| RelayError::Internal)?;
            cache_put(&cache, &key, &body, now + ttl_seconds, now).await?;
            Ok(body)
        })
        .await?;

    serde_json::from_str(&body).map_err(|_| RelayError::Internal)
}

async fn cache_get(cache: &Arc<Cache>, key: &str, now: i64) -> Result<Option<String>, RelayError> {
    let cache = cache.clone();
    let key = key.to_owned();
    tokio::task::spawn_blocking(move || cache.get(&key, now))
        .await
        .map_err(|_| RelayError::Internal)?
        .map_err(|_| RelayError::Cache)
}

async fn cache_put(
    cache: &Arc<Cache>,
    key: &str,
    body: &str,
    expires_at: i64,
    now: i64,
) -> Result<(), RelayError> {
    let cache = cache.clone();
    let key = key.to_owned();
    let body = body.to_owned();
    tokio::task::spawn_blocking(move || cache.put(&key, &body, expires_at, now, MAX_CACHE_ENTRIES))
        .await
        .map_err(|_| RelayError::Internal)?
        .map_err(|_| RelayError::Cache)
}

fn admit(state: &AppState, peer: SocketAddr, headers: &HeaderMap) -> Result<i64, RelayError> {
    let now = crate::cache::now();
    if state
        .limiter
        .check(client_ip(peer, headers, state.config.trust_proxy), now)
    {
        Ok(now)
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
            .rsplit(',')
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
        AppState, HealthResponse, MAX_QUERY_CHARS, SearchParams, client_ip, game_details, health,
        movie_details, normalized_query, search_games, search_movies,
    };
    use crate::cache::Cache;
    use crate::config::Config;
    use crate::error::RelayError;
    use crate::igdb::IgdbUpstream;
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
            details: None,
            edition: None,
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
            Err(RelayError::UpstreamRejected)
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
        headers.insert("x-forwarded-for", "203.0.113.7".parse().unwrap());
        let peer = SocketAddr::from(([127, 0, 0, 1], 1234));

        assert_eq!(
            client_ip(peer, &headers, true),
            IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7))
        );
        assert_eq!(client_ip(peer, &headers, false), peer.ip());
    }

    #[test]
    fn only_the_address_the_proxy_appended_is_trusted() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            "198.51.100.9, 203.0.113.7".parse().unwrap(),
        );
        let peer = SocketAddr::from(([127, 0, 0, 1], 1234));

        assert_eq!(
            client_ip(peer, &headers, true),
            IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7))
        );
    }

    #[test]
    fn a_malformed_proxy_header_falls_back_to_the_peer() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "not-an-address".parse().unwrap());
        let peer = SocketAddr::from(([127, 0, 0, 1], 1234));

        assert_eq!(client_ip(peer, &headers, true), peer.ip());
    }

    #[test]
    fn a_proxy_header_is_ignored_for_a_remote_peer() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "203.0.113.7".parse().unwrap());
        let peer = SocketAddr::from(([192, 0, 2, 1], 1234));

        assert_eq!(client_ip(peer, &headers, true), peer.ip());
    }

    fn game_candidate() -> Candidate {
        Candidate {
            source: Source::Igdb,
            external_id: "11737".to_owned(),
            title: "Outer Wilds".to_owned(),
            release_year: Some(2019),
            cover_url: Some(
                "https://images.igdb.com/igdb/image/upload/t_cover_big/co65ac.jpg".to_owned(),
            ),
            details: None,
            edition: None,
        }
    }

    struct FakeIgdb {
        searches: Mutex<usize>,
        details: Mutex<usize>,
        times: Mutex<usize>,
        times_fail: bool,
    }

    impl FakeIgdb {
        fn ok() -> Self {
            Self {
                searches: Mutex::new(0),
                details: Mutex::new(0),
                times: Mutex::new(0),
                times_fail: false,
            }
        }
    }

    impl IgdbUpstream for FakeIgdb {
        fn search(&self, _query: String) -> UpstreamResult<Vec<Candidate>> {
            let mut calls = self.searches.lock().unwrap();
            *calls += 1;
            Box::pin(async move { Ok(vec![game_candidate()]) })
        }

        fn details(&self, _id: String) -> UpstreamResult<Candidate> {
            let mut calls = self.details.lock().unwrap();
            *calls += 1;
            Box::pin(async move { Ok(game_candidate()) })
        }

        fn time_to_beat(&self, _id: String) -> UpstreamResult<Option<u32>> {
            let mut calls = self.times.lock().unwrap();
            *calls += 1;
            let failed = self.times_fail;
            Box::pin(async move {
                if failed {
                    Err(Error::RateLimited)
                } else {
                    Ok(Some(1_310))
                }
            })
        }
    }

    fn state_with_igdb(igdb: Option<Arc<dyn IgdbUpstream>>) -> AppState {
        AppState::with_upstreams(
            config(&[("IGDB_CLIENT_ID", "id"), ("IGDB_CLIENT_SECRET", "secret")]),
            Cache::open_in_memory().unwrap(),
            None,
            igdb,
        )
    }

    async fn game_search(state: AppState, query: &str) -> Result<SearchResponse, RelayError> {
        search_games(
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

    async fn game_detail(state: AppState, id: &str) -> Result<Candidate, RelayError> {
        game_details(State(state), peer(), HeaderMap::new(), Path(id.to_owned()))
            .await
            .map(|Json(candidate)| candidate)
    }

    #[tokio::test]
    async fn a_game_search_returns_a_cached_result_and_calls_upstream_once() {
        let fake = Arc::new(FakeIgdb::ok());
        let state = state_with_igdb(Some(fake.clone()));

        assert_eq!(
            game_search(state.clone(), "outer wilds")
                .await
                .unwrap()
                .results,
            vec![game_candidate()]
        );
        assert_eq!(
            game_search(state, "outer wilds").await.unwrap().results,
            vec![game_candidate()]
        );
        assert_eq!(*fake.searches.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn a_game_details_returns_a_cached_candidate_and_calls_upstream_once() {
        let fake = Arc::new(FakeIgdb::ok());
        let state = state_with_igdb(Some(fake.clone()));

        assert_eq!(
            game_detail(state.clone(), "11737").await.unwrap(),
            timed_game()
        );
        assert_eq!(game_detail(state, "11737").await.unwrap(), timed_game());
        assert_eq!(*fake.details.lock().unwrap(), 1);
        assert_eq!(*fake.times.lock().unwrap(), 1);
    }

    fn timed_game() -> Candidate {
        loregalia_core::providers::igdb::with_time(game_candidate(), Some(1_310))
    }

    #[tokio::test]
    async fn game_details_cached_before_details_existed_are_fetched_again() {
        let fake = Arc::new(FakeIgdb::ok());
        let state = state_with_igdb(Some(fake.clone()));
        let old = serde_json::to_string(&game_candidate()).unwrap();
        let now = crate::cache::now();
        state
            .cache
            .put("igdb:game:11737", &old, now + 1_000, now, 100)
            .unwrap();

        assert_eq!(game_detail(state, "11737").await.unwrap(), timed_game());
        assert_eq!(*fake.details.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn a_game_without_its_play_time_is_not_cached() {
        let fake = Arc::new(FakeIgdb {
            times_fail: true,
            ..FakeIgdb::ok()
        });
        let state = state_with_igdb(Some(fake.clone()));

        assert!(game_detail(state.clone(), "11737").await.is_err());
        assert!(game_detail(state, "11737").await.is_err());
        assert_eq!(*fake.details.lock().unwrap(), 2);
    }

    #[tokio::test]
    async fn an_unconfigured_game_relay_says_so() {
        let state = state_with_igdb(None);

        assert!(matches!(
            game_search(state, "outer wilds").await,
            Err(RelayError::NotConfigured)
        ));
    }

    #[tokio::test]
    async fn an_invalid_game_id_is_rejected() {
        let state = state_with_igdb(Some(Arc::new(FakeIgdb::ok())));

        assert!(matches!(
            game_detail(state, "outer wilds").await,
            Err(RelayError::BadRequest)
        ));
    }

    struct SlowTmdb {
        searches: Mutex<usize>,
        results: Vec<Candidate>,
    }

    impl TmdbUpstream for SlowTmdb {
        fn search(&self, _query: String) -> UpstreamResult<Vec<Candidate>> {
            *self.searches.lock().unwrap() += 1;
            let results = self.results.clone();
            Box::pin(async move {
                tokio::time::sleep(std::time::Duration::from_millis(40)).await;
                Ok(results)
            })
        }

        fn details(&self, _id: String) -> UpstreamResult<Candidate> {
            Box::pin(async move { Ok(candidate()) })
        }
    }

    #[tokio::test]
    async fn spellings_of_one_search_share_a_cache_entry() {
        let fake = Arc::new(FakeTmdb::ok());
        let state = state_with(Some(fake.clone()));

        search(state.clone(), "Dune  Part Two").await.unwrap();
        search(state.clone(), "  dune part two ").await.unwrap();
        search(state, "DUNE PART TWO").await.unwrap();

        assert_eq!(*fake.searches.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn simultaneous_identical_searches_call_upstream_once() {
        let fake = Arc::new(SlowTmdb {
            searches: Mutex::new(0),
            results: vec![candidate()],
        });
        let state = state_with(Some(fake.clone()));

        let tasks: Vec<_> = (0..5)
            .map(|_| {
                let state = state.clone();
                tokio::spawn(async move { search(state, "dune").await })
            })
            .collect();
        for task in tasks {
            assert_eq!(task.await.unwrap().unwrap().results, vec![candidate()]);
        }

        assert_eq!(*fake.searches.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn a_search_with_no_results_is_cached_too() {
        let fake = Arc::new(SlowTmdb {
            searches: Mutex::new(0),
            results: Vec::new(),
        });
        let state = state_with(Some(fake.clone()));

        assert!(
            search(state.clone(), "qwxyz")
                .await
                .unwrap()
                .results
                .is_empty()
        );
        assert!(search(state, "qwxyz").await.unwrap().results.is_empty());

        assert_eq!(*fake.searches.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn a_failed_search_is_not_cached() {
        let fake = Arc::new(FakeTmdb::with_search_error(FakeError::Unauthorized));
        let state = state_with(Some(fake.clone()));

        assert!(search(state.clone(), "dune").await.is_err());
        assert!(search(state, "dune").await.is_err());

        assert_eq!(*fake.searches.lock().unwrap(), 2);
    }

    #[test]
    fn queries_are_normalized_and_bounded() {
        assert_eq!(
            normalized_query("  Outer   Wilds "),
            Some("outer wilds".to_owned())
        );
        assert_eq!(normalized_query("   "), None);
        assert_eq!(normalized_query(&"a".repeat(MAX_QUERY_CHARS + 1)), None);
    }

    #[tokio::test]
    async fn game_searches_cached_before_ranking_existed_are_fetched_again() {
        let fake = Arc::new(FakeIgdb::ok());
        let state = state_with_igdb(Some(fake.clone()));
        let now = crate::cache::now();
        state
            .cache
            .put(
                "igdb:search:outer wilds",
                r#"{"results":[]}"#,
                now + 1_000,
                now,
                100,
            )
            .unwrap();

        let found = game_search(state, "outer wilds").await.unwrap();
        assert_eq!(found.results, vec![game_candidate()]);
        assert_eq!(*fake.searches.lock().unwrap(), 1);
    }
}
