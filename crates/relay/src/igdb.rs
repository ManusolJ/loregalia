use loregalia_core::Error;
use loregalia_core::providers::{Candidate, igdb};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use crate::config::Secret;

const REFRESH_MARGIN_SECONDS: i64 = 60;

pub type UpstreamResult<T> = Pin<Box<dyn Future<Output = Result<T, Error>> + Send>>;

pub trait IgdbUpstream: Send + Sync {
    fn search(&self, query: String) -> UpstreamResult<Vec<Candidate>>;
    fn details(&self, id: String) -> UpstreamResult<Candidate>;
}

struct CachedToken {
    value: String,
    expires_at: i64,
}

pub struct IgdbClient {
    client_id: Secret,
    client_secret: Secret,
    token: Arc<Mutex<Option<CachedToken>>>,
}

impl IgdbClient {
    pub fn new(client_id: Secret, client_secret: Secret) -> Self {
        Self {
            client_id,
            client_secret,
            token: Arc::new(Mutex::new(None)),
        }
    }
}

fn access_token(
    client_id: &Secret,
    client_secret: &Secret,
    cache: &Mutex<Option<CachedToken>>,
) -> Result<String, Error> {
    let now = crate::cache::now();
    {
        let cached = cache.lock().map_err(|_| Error::ConnectionFailed)?;
        if let Some(token) = cached.as_ref()
            && !should_refresh(Some(token), now)
        {
            return Ok(token.value.clone());
        }
    }

    let token = igdb::token(client_id.expose(), client_secret.expose())?;
    let expires_at = now + i64::try_from(token.expires_in).unwrap_or(0);
    *cache.lock().map_err(|_| Error::ConnectionFailed)? = Some(CachedToken {
        value: token.value.clone(),
        expires_at,
    });

    Ok(token.value)
}

fn should_refresh(cached: Option<&CachedToken>, now: i64) -> bool {
    cached.is_none_or(|token| token.expires_at <= now + REFRESH_MARGIN_SECONDS)
}

impl IgdbUpstream for IgdbClient {
    fn search(&self, query: String) -> UpstreamResult<Vec<Candidate>> {
        let client_id = self.client_id.clone();
        let client_secret = self.client_secret.clone();
        let token = self.token.clone();

        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let access = access_token(&client_id, &client_secret, &token)?;
                igdb::search(client_id.expose(), &access, &query)
            })
            .await
            .map_err(|_| Error::ConnectionFailed)?
        })
    }

    fn details(&self, id: String) -> UpstreamResult<Candidate> {
        let client_id = self.client_id.clone();
        let client_secret = self.client_secret.clone();
        let token = self.token.clone();

        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let access = access_token(&client_id, &client_secret, &token)?;
                igdb::details(client_id.expose(), &access, &id)
            })
            .await
            .map_err(|_| Error::ConnectionFailed)?
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{CachedToken, should_refresh};

    #[test]
    fn a_fresh_token_is_not_refreshed() {
        let cached = CachedToken {
            value: "a".to_owned(),
            expires_at: 1_000,
        };

        assert!(!should_refresh(Some(&cached), 100));
    }

    #[test]
    fn a_token_near_expiry_is_refreshed() {
        let cached = CachedToken {
            value: "a".to_owned(),
            expires_at: 159,
        };

        assert!(should_refresh(Some(&cached), 100));
    }

    #[test]
    fn a_missing_token_is_refreshed() {
        assert!(should_refresh(None, 100));
    }
}
