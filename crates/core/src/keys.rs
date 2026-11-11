use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const FILE_NAME: &str = "api-keys.json";

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ApiKeys {
    pub tmdb_token: Option<String>,
    pub igdb_client_id: Option<String>,
    pub igdb_client_secret: Option<String>,
}

impl ApiKeys {
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(Error::from),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(Error::Io(error)),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json).map_err(Error::from)
    }

    pub fn tmdb_token(&self) -> Option<&str> {
        self.tmdb_token
            .as_deref()
            .filter(|token| !token.trim().is_empty())
    }

    pub fn igdb_credentials(&self) -> Option<(&str, &str)> {
        match (&self.igdb_client_id, &self.igdb_client_secret) {
            (Some(client_id), Some(client_secret))
                if !client_id.trim().is_empty() && !client_secret.trim().is_empty() =>
            {
                Some((client_id, client_secret))
            }
            _ => None,
        }
    }
}

pub fn file_for_database(database_path: &Path) -> Option<PathBuf> {
    let directory = database_path.parent()?;
    if directory.as_os_str().is_empty() {
        return None;
    }
    Some(directory.join(FILE_NAME))
}

#[cfg(test)]
mod tests {
    use super::{ApiKeys, FILE_NAME, file_for_database};
    use std::path::Path;
    use tempfile::TempDir;

    fn keys_path(temporary: &TempDir) -> std::path::PathBuf {
        temporary.path().join(FILE_NAME)
    }

    #[test]
    fn a_missing_file_loads_empty_keys() {
        let temporary = TempDir::new().unwrap();

        assert_eq!(
            ApiKeys::load(&keys_path(&temporary)).unwrap(),
            ApiKeys::default()
        );
    }

    #[test]
    fn keys_round_trip_through_the_file() {
        let temporary = TempDir::new().unwrap();
        let keys = ApiKeys {
            tmdb_token: Some("abc123".to_owned()),
            igdb_client_id: Some("client-id".to_owned()),
            igdb_client_secret: Some("client-secret".to_owned()),
        };

        keys.save(&keys_path(&temporary)).unwrap();

        assert_eq!(ApiKeys::load(&keys_path(&temporary)).unwrap(), keys);
    }

    #[test]
    fn saving_creates_missing_parent_folders() {
        let temporary = TempDir::new().unwrap();
        let path = temporary.path().join("nested").join(FILE_NAME);

        ApiKeys::default().save(&path).unwrap();

        assert!(path.is_file());
    }

    #[test]
    fn invalid_json_is_an_error() {
        let temporary = TempDir::new().unwrap();
        let path = keys_path(&temporary);
        std::fs::write(&path, "{").unwrap();

        assert!(ApiKeys::load(&path).is_err());
    }

    #[test]
    fn saving_to_an_unusable_path_is_an_error() {
        let temporary = TempDir::new().unwrap();
        let blocker = temporary.path().join("blocker");
        std::fs::write(&blocker, "not a directory").unwrap();

        assert!(ApiKeys::default().save(&blocker.join(FILE_NAME)).is_err());
    }

    #[test]
    fn a_blank_token_counts_as_no_token() {
        assert_eq!(
            ApiKeys {
                tmdb_token: Some("   ".to_owned()),
                ..Default::default()
            }
            .tmdb_token(),
            None
        );
        assert_eq!(ApiKeys::default().tmdb_token(), None);
        assert_eq!(
            ApiKeys {
                tmdb_token: Some("abc".to_owned()),
                ..Default::default()
            }
            .tmdb_token(),
            Some("abc")
        );
    }

    #[test]
    fn igdb_credentials_need_both_parts() {
        let both = ApiKeys {
            igdb_client_id: Some("id".to_owned()),
            igdb_client_secret: Some("secret".to_owned()),
            ..Default::default()
        };
        assert_eq!(both.igdb_credentials(), Some(("id", "secret")));

        let only_id = ApiKeys {
            igdb_client_id: Some("id".to_owned()),
            ..Default::default()
        };
        assert_eq!(only_id.igdb_credentials(), None);

        let blank_secret = ApiKeys {
            igdb_client_id: Some("id".to_owned()),
            igdb_client_secret: Some("  ".to_owned()),
            ..Default::default()
        };
        assert_eq!(blank_secret.igdb_credentials(), None);

        assert_eq!(ApiKeys::default().igdb_credentials(), None);
    }

    #[test]
    fn the_keys_file_is_a_sibling_of_the_database() {
        let temporary = TempDir::new().unwrap();
        let database = temporary.path().join("loregalia.db");

        assert_eq!(
            file_for_database(&database),
            Some(temporary.path().join(FILE_NAME))
        );
    }

    #[test]
    fn a_database_path_without_a_parent_has_no_keys_file() {
        assert_eq!(file_for_database(Path::new("loregalia.db")), None);
    }
}
