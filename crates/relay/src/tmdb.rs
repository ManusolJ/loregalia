use loregalia_core::Error;
use loregalia_core::providers::{Candidate, tmdb};
use std::future::Future;
use std::pin::Pin;

use crate::config::Secret;

pub type UpstreamResult<T> = Pin<Box<dyn Future<Output = Result<T, Error>> + Send>>;

pub trait TmdbUpstream: Send + Sync {
    fn search(&self, query: String) -> UpstreamResult<Vec<Candidate>>;
    fn details(&self, id: String) -> UpstreamResult<Candidate>;
}

pub struct TmdbClient {
    token: Secret,
}

impl TmdbClient {
    pub fn new(token: Secret) -> Self {
        Self { token }
    }
}

impl TmdbUpstream for TmdbClient {
    fn search(&self, query: String) -> UpstreamResult<Vec<Candidate>> {
        let token = self.token.clone();

        Box::pin(async move {
            tokio::task::spawn_blocking(move || tmdb::search(token.expose(), &query))
                .await
                .map_err(|_| Error::ConnectionFailed)?
        })
    }

    fn details(&self, id: String) -> UpstreamResult<Candidate> {
        let token = self.token.clone();

        Box::pin(async move {
            tokio::task::spawn_blocking(move || tmdb::details(token.expose(), &id))
                .await
                .map_err(|_| Error::ConnectionFailed)?
        })
    }
}
