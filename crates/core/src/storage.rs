use crate::{Error, Result};
use rusqlite::Connection;
use std::path::Path;

const LATEST_VERSION: u32 = 1;

const MIGRATIONS: &[&str] = &["CREATE TABLE lists (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        created_at TEXT NOT NULL
    ) STRICT;"];

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

    pub fn connection(&self) -> &Connection {
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

    if current > i64::from(LATEST_VERSION) {
        return Err(Error::SchemaTooNew {
            found: current,
            supported: LATEST_VERSION,
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
    use super::{Database, LATEST_VERSION};
    use crate::Error;
    use rusqlite::Connection;
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
                "loregalia-core-{}-{nanos}-{}",
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

    fn schema_version(connection: &Connection) -> i64 {
        connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn fresh_database_is_at_latest_version_with_foreign_keys_on() {
        let database = Database::open_in_memory().unwrap();

        assert_eq!(
            schema_version(database.connection()),
            i64::from(LATEST_VERSION)
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
        let temporary = TempDir::new();
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
                i64::from(LATEST_VERSION)
            );
        }

        let database = Database::open(&path).unwrap();
        assert_eq!(
            schema_version(database.connection()),
            i64::from(LATEST_VERSION)
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
        let temporary = TempDir::new();
        let path = temporary
            .path()
            .join("com.manu.loregalia")
            .join("nested")
            .join("loregalia.db");

        let database = Database::open(&path).unwrap();

        assert!(path.is_file());
        assert_eq!(
            schema_version(database.connection()),
            i64::from(LATEST_VERSION)
        );
    }

    #[test]
    fn newer_database_is_rejected_without_changes() {
        let temporary = TempDir::new();
        let path = temporary.path().join("newer.db");

        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch("PRAGMA user_version = 2;")
                .unwrap();
        }

        assert!(matches!(
            Database::open(&path),
            Err(Error::SchemaTooNew {
                found: 2,
                supported: 1
            })
        ));

        let connection = Connection::open(&path).unwrap();
        assert_eq!(schema_version(&connection), 2);
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
        let temporary = TempDir::new();
        let blocker = temporary.path().join("blocker");
        std::fs::write(&blocker, "not a directory").unwrap();

        assert!(Database::open(blocker.join("nested").join("loregalia.db")).is_err());
        assert!(Database::open(temporary.path()).is_err());
    }
}
