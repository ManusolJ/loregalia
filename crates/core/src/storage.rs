use crate::{Error, Result};
use rusqlite::Connection;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub const APP_FOLDER: &str = "com.manu.loregalia";
pub const DATABASE_FILE: &str = "loregalia.db";

const MIGRATIONS: &[&str] = &[
    "CREATE TABLE lists (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        created_at TEXT NOT NULL
    ) STRICT;",
    "CREATE TABLE items (
        id TEXT PRIMARY KEY,
        media_kind TEXT NOT NULL,
        title TEXT NOT NULL,
        release_year INTEGER,
        release_date TEXT,
        finished_at TEXT,
        source TEXT,
        external_id TEXT,
        created_at TEXT NOT NULL,
        UNIQUE (source, external_id)
    ) STRICT;",
    "CREATE TABLE list_items (
        list_id TEXT NOT NULL REFERENCES lists(id) ON DELETE CASCADE,
        item_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
        position INTEGER NOT NULL,
        PRIMARY KEY (list_id, item_id)
    ) STRICT;",
];

fn latest_version() -> u32 {
    MIGRATIONS.len() as u32
}

pub fn data_directory(env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let base = non_empty(env("APPDATA"))
        .or_else(|| non_empty(env("XDG_DATA_HOME")))
        .or_else(|| non_empty(env("HOME")).map(|home| home.join(".local").join("share")));

    base.map(|base| base.join(APP_FOLDER))
}

pub fn default_data_directory() -> Result<PathBuf> {
    data_directory(|name| std::env::var_os(name)).ok_or(Error::DataDirectoryUnavailable)
}

pub fn database_in(directory: &Path) -> PathBuf {
    directory.join(DATABASE_FILE)
}

fn non_empty(value: Option<OsString>) -> Option<PathBuf> {
    value.filter(|value| !value.is_empty()).map(PathBuf::from)
}

pub struct Database {
    connection: Connection,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();

        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }

        Self::from_connection(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    pub(crate) fn connection(&self) -> &Connection {
        &self.connection
    }

    fn from_connection(mut connection: Connection) -> Result<Self> {
        connection.pragma_update(None, "foreign_keys", true)?;
        migrate(&mut connection)?;

        Ok(Self { connection })
    }
}

fn migrate(connection: &mut Connection) -> Result<()> {
    let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let latest = latest_version();

    if current > i64::from(latest) {
        return Err(Error::SchemaTooNew {
            found: current,
            supported: latest,
        });
    }

    for (index, migration) in MIGRATIONS.iter().enumerate() {
        let version = index as i64 + 1;

        if version <= current {
            continue;
        }

        let transaction = connection.transaction()?;
        transaction.execute_batch(migration)?;
        transaction.pragma_update(None, "user_version", version)?;
        transaction.commit()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        APP_FOLDER, DATABASE_FILE, Database, MIGRATIONS, data_directory, database_in,
        latest_version,
    };
    use crate::Error;
    use rusqlite::Connection;
    use std::ffi::OsString;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn environment(values: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let values: Vec<(String, OsString)> = values
            .iter()
            .map(|(name, value)| ((*name).to_owned(), OsString::from(value)))
            .collect();

        move |name| {
            values
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        }
    }

    fn schema_version(connection: &Connection) -> i64 {
        connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn windows_data_folder_uses_appdata() {
        let directory = data_directory(environment(&[
            ("APPDATA", r"C:\Users\me\AppData\Roaming"),
            ("HOME", "/home/me"),
        ]))
        .unwrap();

        assert_eq!(
            directory,
            PathBuf::from(r"C:\Users\me\AppData\Roaming").join(APP_FOLDER)
        );
    }

    #[test]
    fn linux_data_folder_prefers_xdg_then_home() {
        let xdg = data_directory(environment(&[
            ("XDG_DATA_HOME", "/home/me/.local/share"),
            ("HOME", "/home/me"),
        ]))
        .unwrap();
        assert_eq!(xdg, PathBuf::from("/home/me/.local/share").join(APP_FOLDER));

        let home = data_directory(environment(&[("HOME", "/home/me")])).unwrap();
        assert_eq!(
            home,
            PathBuf::from("/home/me/.local/share").join(APP_FOLDER)
        );
    }

    #[test]
    fn empty_environment_values_are_ignored() {
        let directory = data_directory(environment(&[
            ("APPDATA", ""),
            ("XDG_DATA_HOME", "/home/me/.local/share"),
        ]))
        .unwrap();

        assert_eq!(
            directory,
            PathBuf::from("/home/me/.local/share").join(APP_FOLDER)
        );
    }

    #[test]
    fn missing_environment_has_no_data_folder() {
        assert!(data_directory(environment(&[])).is_none());
    }

    #[test]
    fn database_in_appends_the_file_name() {
        let directory = data_directory(environment(&[("XDG_DATA_HOME", "/data")])).unwrap();

        assert_eq!(
            database_in(&directory),
            PathBuf::from("/data").join(APP_FOLDER).join(DATABASE_FILE)
        );
    }

    #[test]
    fn latest_version_equals_the_number_of_migrations() {
        let database = Database::open_in_memory().unwrap();

        assert_eq!(
            schema_version(database.connection()),
            MIGRATIONS.len() as i64
        );
        assert_eq!(latest_version() as usize, MIGRATIONS.len());
    }

    #[test]
    fn migrates_a_database_written_before_version_one() {
        let temporary = TempDir::new().unwrap();
        let path = temporary.path().join("legacy.db");

        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE legacy (id TEXT PRIMARY KEY) STRICT;
                     INSERT INTO legacy (id) VALUES ('keep-me');
                     PRAGMA user_version = 0;",
                )
                .unwrap();
        }

        let database = Database::open(&path).unwrap();

        assert_eq!(
            schema_version(database.connection()),
            i64::from(latest_version())
        );
        let legacy: String = database
            .connection()
            .query_row("SELECT id FROM legacy", [], |row| row.get(0))
            .unwrap();
        assert_eq!(legacy, "keep-me");
        let lists: i64 = database
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'lists'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(lists, 1);
    }

    #[test]
    fn fresh_database_is_at_latest_version_with_foreign_keys_on() {
        let database = Database::open_in_memory().unwrap();

        assert_eq!(
            schema_version(database.connection()),
            i64::from(latest_version())
        );

        let foreign_keys: i64 = database
            .connection()
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();
        assert_eq!(foreign_keys, 1);

        let lists_table: String = database
            .connection()
            .query_row(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'lists'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(lists_table, "lists");
    }

    #[test]
    fn reopening_keeps_rows_and_does_not_rerun_migrations() {
        let temporary = TempDir::new().unwrap();
        let path = temporary.path().join("nested").join("loregalia.db");

        {
            let database = Database::open(&path).unwrap();
            database
                .connection()
                .execute(
                    "INSERT INTO lists (id, name, created_at) VALUES (?1, ?2, ?3)",
                    ["list-1", "Watchlist", "2026-11-05T00:00:00Z"],
                )
                .unwrap();
            assert_eq!(
                schema_version(database.connection()),
                i64::from(latest_version())
            );
        }

        let database = Database::open(&path).unwrap();
        assert_eq!(
            schema_version(database.connection()),
            i64::from(latest_version())
        );

        let (name, created_at): (String, String) = database
            .connection()
            .query_row(
                "SELECT name, created_at FROM lists WHERE id = ?1",
                ["list-1"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(name, "Watchlist");
        assert_eq!(created_at, "2026-11-05T00:00:00Z");
    }

    #[test]
    fn open_creates_missing_parent_folders() {
        let temporary = TempDir::new().unwrap();
        let path = temporary
            .path()
            .join("com.manu.loregalia")
            .join("nested")
            .join("loregalia.db");

        let database = Database::open(&path).unwrap();

        assert!(path.is_file());
        assert_eq!(
            schema_version(database.connection()),
            i64::from(latest_version())
        );
    }

    #[test]
    fn newer_database_is_rejected_without_changes() {
        let temporary = TempDir::new().unwrap();
        let path = temporary.path().join("newer.db");

        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch("PRAGMA user_version = 4;")
                .unwrap();
        }

        assert!(matches!(
            Database::open(&path),
            Err(Error::SchemaTooNew {
                found: 4,
                supported: 3
            })
        ));

        let connection = Connection::open(&path).unwrap();
        assert_eq!(schema_version(&connection), 4);
        let list_tables: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'lists'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(list_tables, 0);
    }

    #[test]
    fn unusable_path_returns_error_without_panicking() {
        let temporary = TempDir::new().unwrap();
        let blocker = temporary.path().join("blocker");
        std::fs::write(&blocker, "not a directory").unwrap();

        assert!(Database::open(blocker.join("nested").join("loregalia.db")).is_err());
        assert!(Database::open(temporary.path()).is_err());
    }
}
