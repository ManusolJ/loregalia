use loregalia_core::{
    Database, Result as CoreResult, log,
    settings::{self, Theme},
    storage,
};
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
    pub theme: Theme,
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

    let (storage, theme) = match directory {
        Ok(directory) => open_at_with_theme(storage::database_in(&directory)),
        Err(error) => (
            StorageState::Failed(format!("Could not find the app data folder: {error}")),
            Theme::Dark,
        ),
    };

    match &storage {
        StorageState::Ready(path) => {
            record(&log_path, &format!("database ready at {}", path.display()));
        }
        StorageState::Failed(message) => record(&log_path, message),
    }

    Startup {
        storage,
        log_path,
        theme,
    }
}

#[cfg(test)]
fn open_at(path: PathBuf) -> StorageState {
    open_at_with_theme(path).0
}

fn open_at_with_theme(path: PathBuf) -> (StorageState, Theme) {
    match Database::open(&path) {
        Ok(database) => {
            let theme = settings::theme(&database).unwrap_or_default();
            (StorageState::Ready(path), theme)
        }
        Err(error) => (
            StorageState::Failed(format!("Could not open the database: {error}")),
            Theme::Dark,
        ),
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
    use loregalia_core::{
        Database, Error,
        settings::{self, Theme},
        storage,
    };
    use tempfile::TempDir;

    #[test]
    fn missing_folders_are_created_before_migrating() {
        let temporary = TempDir::new().unwrap();
        let path = temporary
            .path()
            .join("com.manu.loregalia")
            .join("loregalia.db");

        assert_eq!(open_at(path.clone()), StorageState::Ready(path.clone()));
        assert!(path.is_file());
    }

    #[test]
    fn an_unusable_database_path_becomes_a_visible_failure() {
        let temporary = TempDir::new().unwrap();
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
    fn a_corrupt_database_is_reported_without_being_changed() {
        let temporary = TempDir::new().unwrap();
        let path = temporary.path().join("loregalia.db");
        std::fs::write(&path, b"this is not a database").unwrap();
        let before = std::fs::read(&path).unwrap();

        let state = open_at(path.clone());

        let message = match state {
            StorageState::Failed(message) => message,
            StorageState::Ready(path) => format!("unexpected ready state at {}", path.display()),
        };
        assert!(message.starts_with("Could not open the database:"));
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn startup_logs_the_version_and_database_path() {
        let temporary = TempDir::new().unwrap();

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
        assert_eq!(startup.theme, Theme::Dark);
    }

    #[test]
    fn startup_loads_the_saved_theme() {
        let temporary = TempDir::new().unwrap();
        let database = Database::open(storage::database_in(temporary.path())).unwrap();
        settings::set_theme(&database, Theme::Light).unwrap();
        drop(database);

        let startup = prepare_with(Ok(temporary.path().to_path_buf()));

        assert!(matches!(&startup.storage, StorageState::Ready(_)));
        assert_eq!(startup.theme, Theme::Light);
    }

    #[test]
    fn startup_falls_back_to_dark_for_an_unknown_theme() {
        let temporary = TempDir::new().unwrap();
        let database = Database::open(storage::database_in(temporary.path())).unwrap();
        settings::set(&database, "theme", "neon").unwrap();
        drop(database);

        let startup = prepare_with(Ok(temporary.path().to_path_buf()));

        assert_eq!(startup.theme, Theme::Dark);
    }

    #[test]
    fn startup_failures_are_written_to_the_log() {
        let temporary = TempDir::new().unwrap();
        std::fs::create_dir_all(temporary.path().join("loregalia.db")).unwrap();

        let startup = prepare_with(Ok(temporary.path().to_path_buf()));

        assert!(matches!(&startup.storage, StorageState::Failed(_)));
        let log_path = startup.log_path.clone().unwrap();
        let contents = std::fs::read_to_string(log_path).unwrap();
        assert!(contents.contains("Could not open the database"));
    }

    #[test]
    fn a_window_open_failure_is_written_to_the_log() {
        let temporary = TempDir::new().unwrap();
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
