use crate::{Error, Result};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const FILE_NAME: &str = "api-keys.json";
#[cfg(windows)]
const SERVICE: &str = "loregalia";
const TMDB_TOKEN: &str = "tmdb_token";
const IGDB_CLIENT_ID: &str = "igdb_client_id";
const IGDB_CLIENT_SECRET: &str = "igdb_client_secret";

pub trait SecretStore: Send + Sync {
    fn read(&self, name: &str) -> Result<Option<String>>;
    fn write(&self, name: &str, secret: &str) -> Result<()>;
    fn erase(&self, name: &str) -> Result<()>;
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ApiKeys {
    pub tmdb_token: Option<String>,
    pub igdb_client_id: Option<String>,
    pub igdb_client_secret: Option<String>,
}

impl ApiKeys {
    pub fn load(store: &dyn SecretStore) -> Result<Self> {
        Ok(Self {
            tmdb_token: store.read(TMDB_TOKEN)?,
            igdb_client_id: store.read(IGDB_CLIENT_ID)?,
            igdb_client_secret: store.read(IGDB_CLIENT_SECRET)?,
        })
    }

    pub fn save(&self, store: &dyn SecretStore) -> Result<()> {
        for (name, value) in [
            (TMDB_TOKEN, &self.tmdb_token),
            (IGDB_CLIENT_ID, &self.igdb_client_id),
            (IGDB_CLIENT_SECRET, &self.igdb_client_secret),
        ] {
            match value
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                Some(secret) => store.write(name, secret)?,
                None => store.erase(name)?,
            }
        }
        Ok(())
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

#[derive(Default)]
pub struct MemoryStore {
    secrets: Mutex<HashMap<String, String>>,
}

impl MemoryStore {
    fn secrets(&self) -> Result<std::sync::MutexGuard<'_, HashMap<String, String>>> {
        self.secrets
            .lock()
            .map_err(|_| Error::SecretStore("the in-memory key store is unusable".to_owned()))
    }
}

impl SecretStore for MemoryStore {
    fn read(&self, name: &str) -> Result<Option<String>> {
        Ok(self.secrets()?.get(name).cloned())
    }

    fn write(&self, name: &str, secret: &str) -> Result<()> {
        self.secrets()?.insert(name.to_owned(), secret.to_owned());
        Ok(())
    }

    fn erase(&self, name: &str) -> Result<()> {
        self.secrets()?.remove(name);
        Ok(())
    }
}

pub struct FileStore {
    path: PathBuf,
}

impl FileStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn entries(&self) -> Result<BTreeMap<String, Option<String>>> {
        match std::fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(Error::from),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(error) => Err(Error::Io(error)),
        }
    }

    fn store(&self, entries: &BTreeMap<String, Option<String>>) -> Result<()> {
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(entries)?;
        std::fs::write(&self.path, json).map_err(Error::from)
    }
}

impl SecretStore for FileStore {
    fn read(&self, name: &str) -> Result<Option<String>> {
        Ok(self.entries()?.remove(name).flatten())
    }

    fn write(&self, name: &str, secret: &str) -> Result<()> {
        let mut entries = self.entries()?;
        entries.insert(name.to_owned(), Some(secret.to_owned()));
        self.store(&entries)
    }

    fn erase(&self, name: &str) -> Result<()> {
        let mut entries = self.entries()?;
        if entries.remove(name).is_some() {
            self.store(&entries)?;
        }
        Ok(())
    }
}

pub struct UnavailableStore {
    reason: String,
}

impl UnavailableStore {
    pub fn new(reason: String) -> Self {
        Self { reason }
    }

    fn failure(&self) -> Error {
        Error::SecretStore(self.reason.clone())
    }
}

impl SecretStore for UnavailableStore {
    fn read(&self, _name: &str) -> Result<Option<String>> {
        Err(self.failure())
    }

    fn write(&self, _name: &str, _secret: &str) -> Result<()> {
        Err(self.failure())
    }

    fn erase(&self, _name: &str) -> Result<()> {
        Err(self.failure())
    }
}

#[cfg(windows)]
pub struct CredentialStore {
    store: Arc<windows_native_keyring_store::Store>,
    service: String,
}

#[cfg(windows)]
impl CredentialStore {
    pub fn open() -> Result<Self> {
        Self::open_for(SERVICE)
    }

    fn open_for(service: &str) -> Result<Self> {
        Ok(Self {
            store: windows_native_keyring_store::Store::new().map_err(credential_error)?,
            service: service.to_owned(),
        })
    }

    fn entry(&self, name: &str) -> Result<keyring_core::Entry> {
        use keyring_core::api::CredentialStoreApi;
        let local = HashMap::from([("persistence", "Local")]);
        self.store
            .build(&self.service, name, Some(&local))
            .map_err(credential_error)
    }
}

#[cfg(windows)]
fn credential_error(error: keyring_core::Error) -> Error {
    Error::SecretStore(error.to_string())
}

#[cfg(windows)]
impl SecretStore for CredentialStore {
    fn read(&self, name: &str) -> Result<Option<String>> {
        match self.entry(name)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(error) => Err(credential_error(error)),
        }
    }

    fn write(&self, name: &str, secret: &str) -> Result<()> {
        self.entry(name)?
            .set_password(secret)
            .map_err(credential_error)
    }

    fn erase(&self, name: &str) -> Result<()> {
        match self.entry(name)?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(error) => Err(credential_error(error)),
        }
    }
}

pub fn adopt_file(path: &Path, store: &dyn SecretStore) -> Result<bool> {
    if !path.is_file() {
        return Ok(false);
    }
    ApiKeys::load(&FileStore::new(path.to_path_buf()))?.save(store)?;
    std::fs::remove_file(path)?;
    Ok(true)
}

pub struct OpenedStore {
    pub store: Arc<dyn SecretStore>,
    pub note: Option<String>,
}

#[cfg(windows)]
pub fn open_store(data_directory: &Path) -> OpenedStore {
    let store = match CredentialStore::open() {
        Ok(store) => store,
        Err(error) => {
            let reason = format!("Windows Credential Manager is unavailable: {error}");
            return OpenedStore {
                note: Some(reason.clone()),
                store: Arc::new(UnavailableStore::new(reason)),
            };
        }
    };
    let note = match adopt_file(&data_directory.join(FILE_NAME), &store) {
        Ok(true) => Some("Moved the saved API keys into Windows Credential Manager".to_owned()),
        Ok(false) => None,
        Err(error) => Some(format!(
            "Could not move the saved API keys into Windows Credential Manager: {error}"
        )),
    };
    OpenedStore {
        store: Arc::new(store),
        note,
    }
}

#[cfg(not(windows))]
pub fn open_store(data_directory: &Path) -> OpenedStore {
    OpenedStore {
        store: Arc::new(FileStore::new(data_directory.join(FILE_NAME))),
        note: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ApiKeys, FILE_NAME, FileStore, MemoryStore, SecretStore, UnavailableStore, adopt_file,
    };
    use tempfile::TempDir;

    fn full_keys() -> ApiKeys {
        ApiKeys {
            tmdb_token: Some("abc123".to_owned()),
            igdb_client_id: Some("client-id".to_owned()),
            igdb_client_secret: Some("client-secret".to_owned()),
        }
    }

    #[test]
    fn an_empty_store_loads_empty_keys() {
        assert_eq!(
            ApiKeys::load(&MemoryStore::default()).unwrap(),
            ApiKeys::default()
        );
    }

    #[test]
    fn keys_round_trip_through_a_store() {
        let store = MemoryStore::default();

        full_keys().save(&store).unwrap();

        assert_eq!(ApiKeys::load(&store).unwrap(), full_keys());
    }

    #[test]
    fn saving_blank_keys_erases_them() {
        let store = MemoryStore::default();
        full_keys().save(&store).unwrap();

        ApiKeys {
            tmdb_token: Some("   ".to_owned()),
            ..Default::default()
        }
        .save(&store)
        .unwrap();

        assert_eq!(store.read("tmdb_token").unwrap(), None);
        assert_eq!(ApiKeys::load(&store).unwrap(), ApiKeys::default());
    }

    #[test]
    fn keys_round_trip_through_the_file_store() {
        let temporary = TempDir::new().unwrap();
        let store = FileStore::new(temporary.path().join("nested").join(FILE_NAME));

        full_keys().save(&store).unwrap();

        assert_eq!(ApiKeys::load(&store).unwrap(), full_keys());
    }

    #[test]
    fn the_file_store_reads_the_old_keys_file() {
        let temporary = TempDir::new().unwrap();
        let path = temporary.path().join(FILE_NAME);
        std::fs::write(
            &path,
            r#"{"tmdb_token":"abc123","igdb_client_id":null,"igdb_client_secret":null}"#,
        )
        .unwrap();

        let keys = ApiKeys::load(&FileStore::new(path)).unwrap();

        assert_eq!(keys.tmdb_token.as_deref(), Some("abc123"));
        assert_eq!(keys.igdb_client_id, None);
    }

    #[test]
    fn an_invalid_keys_file_is_an_error() {
        let temporary = TempDir::new().unwrap();
        let path = temporary.path().join(FILE_NAME);
        std::fs::write(&path, "{").unwrap();

        assert!(ApiKeys::load(&FileStore::new(path)).is_err());
    }

    #[test]
    fn saving_to_an_unusable_path_is_an_error() {
        let temporary = TempDir::new().unwrap();
        let blocker = temporary.path().join("blocker");
        std::fs::write(&blocker, "not a directory").unwrap();

        assert!(
            full_keys()
                .save(&FileStore::new(blocker.join(FILE_NAME)))
                .is_err()
        );
    }

    #[test]
    fn an_unavailable_store_fails_instead_of_looking_empty() {
        let store = UnavailableStore::new("no credential manager".to_owned());

        assert!(ApiKeys::load(&store).is_err());
        assert!(full_keys().save(&store).is_err());
    }

    #[test]
    fn adopting_moves_the_keys_file_into_the_store() {
        let temporary = TempDir::new().unwrap();
        let path = temporary.path().join(FILE_NAME);
        full_keys().save(&FileStore::new(path.clone())).unwrap();
        let store = MemoryStore::default();

        assert!(adopt_file(&path, &store).unwrap());

        assert_eq!(ApiKeys::load(&store).unwrap(), full_keys());
        assert!(!path.exists());
    }

    #[test]
    fn adopting_without_a_keys_file_changes_nothing() {
        let temporary = TempDir::new().unwrap();
        let store = MemoryStore::default();

        assert!(!adopt_file(&temporary.path().join(FILE_NAME), &store).unwrap());
        assert_eq!(ApiKeys::load(&store).unwrap(), ApiKeys::default());
    }

    #[test]
    fn a_failed_adoption_keeps_the_keys_file() {
        let temporary = TempDir::new().unwrap();
        let path = temporary.path().join(FILE_NAME);
        full_keys().save(&FileStore::new(path.clone())).unwrap();

        let store = UnavailableStore::new("no credential manager".to_owned());

        assert!(adopt_file(&path, &store).is_err());
        assert!(path.is_file());
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

    #[cfg(windows)]
    #[test]
    fn credential_manager_round_trips_and_erases_a_secret() {
        let service = format!("loregalia-test-{}", uuid::Uuid::now_v7());
        let store = super::CredentialStore::open_for(&service).unwrap();

        assert_eq!(store.read("tmdb_token").unwrap(), None);
        store.write("tmdb_token", "test-value").unwrap();
        assert_eq!(
            store.read("tmdb_token").unwrap().as_deref(),
            Some("test-value")
        );
        store.erase("tmdb_token").unwrap();
        assert_eq!(store.read("tmdb_token").unwrap(), None);
        store.erase("tmdb_token").unwrap();
    }
}
