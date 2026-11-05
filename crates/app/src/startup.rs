use loregalia_core::{Database, Result as CoreResult, log, storage};
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageState {
    Ready(PathBuf),
    Failed(String),
}

pub struct Startup {
    pub storage: StorageState,
    pub log_path: Option<PathBuf>,
}

pub fn prepare() -> Startup {
    prepare_with(storage::default_data_directory())
}

fn prepare_with(directory: CoreResult<PathBuf>) -> Startup {
    let log_path = directory
        .as_ref()
        .ok()
        .map(|directory| log::path_in(directory));

    record(
        &log_path,
        &format!("Loregalia {} starting", env!("CARGO_PKG_VERSION")),
    );

    let storage = match directory {
        Ok(directory) => open_at(storage::database_in(&directory)),
        Err(error) => StorageState::Failed(format!("Could not find the app data folder: {error}")),
    };

    match &storage {
        StorageState::Ready(path) => {
            record(&log_path, &format!("database ready at {}", path.display()));
        }
        StorageState::Failed(message) => record(&log_path, message),
    }

    Startup { storage, log_path }
}

fn open_at(path: PathBuf) -> StorageState {
    match Database::open(&path) {
        Ok(_) => StorageState::Ready(path),
        Err(error) => StorageState::Failed(format!("Could not open the database: {error}")),
    }
}

fn record(log_path: &Option<PathBuf>, message: &str) {
    if let Some(path) = log_path {
        let _ = log::append(path, message);
    }
}

#[cfg(test)]
mod tests {
    use super::{StorageState, open_at, prepare_with};
    use loregalia_core::Error;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default();
            let path = std::env::temp_dir().join(format!(
                "loregalia-app-{}-{nanos}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn missing_folders_are_created_before_migrating() {
        let temporary = TempDir::new();
        let path = temporary
            .path()
            .join("com.manu.loregalia")
            .join("loregalia.db");

        assert_eq!(open_at(path.clone()), StorageState::Ready(path.clone()));
        assert!(path.is_file());
    }

    #[test]
    fn an_unusable_database_path_becomes_a_visible_failure() {
        let temporary = TempDir::new();
        let blocker = temporary.path().join("blocker");
        std::fs::write(&blocker, "not a directory").unwrap();

        let state = open_at(blocker.join("nested").join("loregalia.db"));

        let message = match state {
            StorageState::Failed(message) => message,
            StorageState::Ready(path) => format!("unexpected ready state at {}", path.display()),
        };
        assert!(message.starts_with("Could not open the database:"));
    }

    #[test]
    fn startup_logs_the_version_and_database_path() {
        let temporary = TempDir::new();

        let startup = prepare_with(Ok(temporary.path().to_path_buf()));

        assert!(matches!(&startup.storage, StorageState::Ready(_)));
        let log_path = startup.log_path.clone().unwrap();
        let contents = std::fs::read_to_string(log_path).unwrap();
        assert!(contents.contains(env!("CARGO_PKG_VERSION")));
        assert!(contents.contains("starting"));
        assert!(contents.contains("database ready at"));
        assert!(contents.contains("loregalia.db"));
    }

    #[test]
    fn a_missing_data_folder_has_no_log() {
        let startup = prepare_with(Err(Error::DataDirectoryUnavailable));

        assert!(matches!(&startup.storage, StorageState::Failed(_)));
        assert!(startup.log_path.is_none());
    }

    #[test]
    fn startup_failures_are_written_to_the_log() {
        let temporary = TempDir::new();
        std::fs::create_dir_all(temporary.path().join("loregalia.db")).unwrap();

        let startup = prepare_with(Ok(temporary.path().to_path_buf()));

        assert!(matches!(&startup.storage, StorageState::Failed(_)));
        let log_path = startup.log_path.clone().unwrap();
        let contents = std::fs::read_to_string(log_path).unwrap();
        assert!(contents.contains("Could not open the database"));
    }
}
