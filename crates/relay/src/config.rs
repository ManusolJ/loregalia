use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;

pub const DEFAULT_BIND: &str = "127.0.0.1:8080";
pub const DEFAULT_DATABASE: &str = "relay-cache.db";

#[derive(Clone, Eq, PartialEq)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Secret(***)")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub bind_address: SocketAddr,
    pub database_path: PathBuf,
    pub trust_proxy: bool,
    pub tmdb_token: Option<Secret>,
    pub igdb_client_id: Option<Secret>,
    pub igdb_client_secret: Option<Secret>,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    pub fn from_lookup(
        mut lookup: impl FnMut(&str) -> Option<String>,
    ) -> Result<Self, ConfigError> {
        let bind = lookup("LOREGALIA_RELAY_BIND")
            .filter(|value| non_empty(value))
            .unwrap_or_else(|| DEFAULT_BIND.to_owned());
        let bind_address = bind
            .parse()
            .map_err(|_| ConfigError::InvalidBindAddress(bind))?;
        let database_path = PathBuf::from(
            lookup("LOREGALIA_RELAY_DATABASE")
                .filter(|value| non_empty(value))
                .unwrap_or_else(|| DEFAULT_DATABASE.to_owned()),
        );
        let trust_proxy = lookup("LOREGALIA_RELAY_TRUST_PROXY")
            .filter(|value| non_empty(value))
            .is_some_and(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes"
                )
            });

        Ok(Self {
            bind_address,
            database_path,
            trust_proxy,
            tmdb_token: lookup("TMDB_TOKEN")
                .filter(|value| non_empty(value))
                .map(Secret::new),
            igdb_client_id: lookup("IGDB_CLIENT_ID")
                .filter(|value| non_empty(value))
                .map(Secret::new),
            igdb_client_secret: lookup("IGDB_CLIENT_SECRET")
                .filter(|value| non_empty(value))
                .map(Secret::new),
        })
    }

    pub fn tmdb_configured(&self) -> bool {
        self.tmdb_token.is_some()
    }

    pub fn igdb_configured(&self) -> bool {
        self.igdb_client_id.is_some() && self.igdb_client_secret.is_some()
    }
}

fn non_empty(value: &str) -> bool {
    !value.trim().is_empty()
}

#[derive(Debug, Eq, PartialEq)]
pub enum ConfigError {
    InvalidBindAddress(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBindAddress(value) => {
                write!(formatter, "{value} is not a valid relay bind address")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::{Config, ConfigError, DEFAULT_BIND, DEFAULT_DATABASE};
    use std::collections::HashMap;

    fn config(values: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let values: HashMap<String, String> = values
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();

        Config::from_lookup(|name| values.get(name).cloned())
    }

    #[test]
    fn defaults_to_a_local_address_and_database() {
        let config = config(&[]).unwrap();

        assert_eq!(config.bind_address.to_string(), DEFAULT_BIND);
        assert_eq!(
            config.database_path.to_string_lossy(),
            DEFAULT_DATABASE.to_owned()
        );
        assert!(!config.tmdb_configured());
        assert!(!config.igdb_configured());
        assert!(!config.trust_proxy);
    }

    #[test]
    fn reads_every_value() {
        let config = config(&[
            ("LOREGALIA_RELAY_BIND", "0.0.0.0:9000"),
            ("LOREGALIA_RELAY_DATABASE", "/tmp/relay.db"),
            ("LOREGALIA_RELAY_TRUST_PROXY", "true"),
            ("TMDB_TOKEN", "tmdb-secret"),
            ("IGDB_CLIENT_ID", "igdb-id"),
            ("IGDB_CLIENT_SECRET", "igdb-secret"),
        ])
        .unwrap();

        assert_eq!(config.bind_address.to_string(), "0.0.0.0:9000");
        assert_eq!(config.database_path.to_string_lossy(), "/tmp/relay.db");
        assert!(config.trust_proxy);
        assert!(config.tmdb_configured());
        assert!(config.igdb_configured());
        assert_eq!(config.tmdb_token.unwrap().expose(), "tmdb-secret");
    }

    #[test]
    fn empty_values_are_treated_as_missing() {
        let config = config(&[
            ("LOREGALIA_RELAY_BIND", "   "),
            ("TMDB_TOKEN", ""),
            ("IGDB_CLIENT_ID", ""),
            ("IGDB_CLIENT_SECRET", ""),
        ])
        .unwrap();

        assert_eq!(config.bind_address.to_string(), DEFAULT_BIND);
        assert!(!config.tmdb_configured());
        assert!(!config.igdb_configured());
    }

    #[test]
    fn an_invalid_bind_address_is_an_error() {
        assert_eq!(
            config(&[("LOREGALIA_RELAY_BIND", "not-an-address")]),
            Err(ConfigError::InvalidBindAddress("not-an-address".to_owned()))
        );
    }

    #[test]
    fn debug_output_redacts_secrets() {
        let config = config(&[
            ("TMDB_TOKEN", "tmdb-secret"),
            ("IGDB_CLIENT_ID", "igdb-id"),
            ("IGDB_CLIENT_SECRET", "igdb-secret"),
        ])
        .unwrap();

        let debug = format!("{config:?}");

        assert!(debug.contains("Secret(***)"));
        assert!(!debug.contains("tmdb-secret"));
        assert!(!debug.contains("igdb-secret"));
    }

    #[test]
    fn igdb_needs_both_credentials() {
        let config = config(&[("IGDB_CLIENT_ID", "igdb-id")]).unwrap();

        assert!(!config.igdb_configured());
    }
}
