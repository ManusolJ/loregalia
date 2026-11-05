use loregalia_core::{Database, storage};
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageState {
    Ready(PathBuf),
    Failed(String),
}

pub fn prepare() -> StorageState {
    match storage::default_database_path() {
        Ok(path) => open_at(path),
        Err(error) => StorageState::Failed(format!("Could not find the app data folder: {error}")),
    }
}

fn open_at(path: PathBuf) -> StorageState {
    match Database::open(&path) {
        Ok(_) => StorageState::Ready(path),
        Err(error) => StorageState::Failed(format!("Could not open the database: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::{StorageState, open_at};
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
}
