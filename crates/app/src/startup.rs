use loregalia_core::{Database, Result as CoreResult, log, storage};
use std::any::Any;
use std::fmt::Display;
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

pub fn record(log_path: &Option<PathBuf>, message: &str) {
    if let Some(path) = log_path {
        let _ = log::append(path, message);
    }
}

pub fn window_open_failure(error: &impl Display) -> String {
    format!("Failed to open the Loregalia window: {error}")
}

pub fn panic_message(payload: &(dyn Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("panic with a non-text message")
}

pub fn panic_line(message: &str, location: Option<&str>) -> String {
    format!(
        "panic at {}: {message}",
        location.unwrap_or("an unknown location")
    )
}

pub fn install_panic_hook(log_path: &Option<PathBuf>) {
    let Some(path) = log_path.clone() else {
        return;
    };

    std::panic::set_hook(Box::new(move |info| {
        let message = panic_message(info.payload());
        let location = info.location().map(|location| location.to_string());
        record(
            &Some(path.clone()),
            &panic_line(message, location.as_deref()),
        );
    }));
}

#[cfg(test)]
mod tests {
    use super::{
        StorageState, install_panic_hook, open_at, panic_line, panic_message, prepare_with, record,
        window_open_failure,
    };
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

    #[test]
    fn a_window_open_failure_is_written_to_the_log() {
        let temporary = TempDir::new();
        let path = temporary.path().join("loregalia.log");
        let error = std::io::Error::other("no display");

        record(&Some(path.clone()), &window_open_failure(&error));

        let contents = std::fs::read_to_string(path).unwrap();
        assert!(contents.ends_with("Failed to open the Loregalia window: no display\n"));
    }

    #[test]
    fn a_panic_is_one_line_with_its_location() {
        let line = panic_line("boom", Some("crates/app/src/main.rs:42:9"));

        assert_eq!(line, "panic at crates/app/src/main.rs:42:9: boom");
        assert!(!line.contains('\n'));
        assert_eq!(
            panic_line("boom", None),
            "panic at an unknown location: boom"
        );
        assert_eq!(panic_message(&"boom"), "boom");
        assert_eq!(panic_message(&String::from("boom")), "boom");
        assert_eq!(panic_message(&42_u32), "panic with a non-text message");
    }

    #[test]
    fn recording_without_a_log_path_does_nothing() {
        record(&None, "ignored");
        install_panic_hook(&None);
    }
}
