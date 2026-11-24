use loregalia_core::Error;
use loregalia_core::providers::Candidate;
use loregalia_core::providers::igdb::{self, TokenCache, unix_now};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::config::Secret;

pub type UpstreamResult<T> = Pin<Box<dyn Future<Output = Result<T, Error>> + Send>>;

pub trait IgdbUpstream: Send + Sync {
    fn search(&self, query: String) -> UpstreamResult<Vec<Candidate>>;
    fn details(&self, id: String) -> UpstreamResult<Candidate>;
    fn time_to_beat(&self, id: String) -> UpstreamResult<Option<u32>>;
}

pub struct IgdbClient {
    client_id: Secret,
    client_secret: Secret,
    tokens: Arc<TokenCache>,
}

impl IgdbClient {
    pub fn new(client_id: Secret, client_secret: Secret) -> Self {
        Self {
            client_id,
            client_secret,
            tokens: Arc::new(TokenCache::new()),
        }
    }

    fn call<T: Send + 'static>(
        &self,
        request: impl Fn(&str, &str) -> Result<T, Error> + Send + 'static,
    ) -> UpstreamResult<T> {
        let client_id = self.client_id.clone();
        let client_secret = self.client_secret.clone();
        let tokens = self.tokens.clone();

        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let id = client_id.expose();
                let secret = client_secret.expose();
                tokens.with_token(
                    id,
                    secret,
                    unix_now(),
                    &|| igdb::token(id, secret),
                    &|access| request(id, access),
                )
            })
            .await
            .map_err(|_| Error::ConnectionFailed)?
        })
    }
}

impl IgdbUpstream for IgdbClient {
    fn search(&self, query: String) -> UpstreamResult<Vec<Candidate>> {
        self.call(move |client_id, access| igdb::search(client_id, access, &query))
    }

    fn details(&self, id: String) -> UpstreamResult<Candidate> {
        self.call(move |client_id, access| igdb::game(client_id, access, &id))
    }

    fn time_to_beat(&self, id: String) -> UpstreamResult<Option<u32>> {
        self.call(move |client_id, access| igdb::time_to_beat(client_id, access, &id))
    }
}
