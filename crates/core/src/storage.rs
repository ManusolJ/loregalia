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
    "CREATE TABLE statuses (
        id TEXT PRIMARY KEY,
        media_kind TEXT NOT NULL,
        category TEXT NOT NULL,
        label TEXT NOT NULL,
        position INTEGER NOT NULL,
        UNIQUE (media_kind, category)
    ) STRICT;
    INSERT INTO statuses (id, media_kind, category, label, position) VALUES
        ('movie-planned', 'movie', 'planned', 'To watch', 0),
        ('movie-active', 'movie', 'active', 'In progress', 1),
        ('movie-paused', 'movie', 'paused', 'On hold', 2),
        ('movie-done', 'movie', 'done', 'Watched', 3),
        ('movie-dropped', 'movie', 'dropped', 'Dropped', 4),
        ('show-planned', 'show', 'planned', 'To watch', 0),
        ('show-active', 'show', 'active', 'In progress', 1),
        ('show-paused', 'show', 'paused', 'On hold', 2),
        ('show-done', 'show', 'done', 'Watched', 3),
        ('show-dropped', 'show', 'dropped', 'Dropped', 4),
        ('book-planned', 'book', 'planned', 'To read', 0),
        ('book-active', 'book', 'active', 'In progress', 1),
        ('book-paused', 'book', 'paused', 'On hold', 2),
        ('book-done', 'book', 'done', 'Read', 3),
        ('book-dropped', 'book', 'dropped', 'Dropped', 4),
        ('game-planned', 'game', 'planned', 'To play', 0),
        ('game-active', 'game', 'active', 'In progress', 1),
        ('game-paused', 'game', 'paused', 'On hold', 2),
        ('game-done', 'game', 'done', 'Played', 3),
        ('game-dropped', 'game', 'dropped', 'Dropped', 4);
    ALTER TABLE items ADD COLUMN status_id TEXT REFERENCES statuses(id);
    UPDATE items SET status_id = (
        SELECT statuses.id FROM statuses
        WHERE statuses.media_kind = items.media_kind
          AND statuses.category = 'planned'
    );",
    "PRAGMA defer_foreign_keys = ON;
    CREATE TABLE item_statuses_backup (
        id TEXT PRIMARY KEY,
        status_id TEXT
    ) STRICT;
    INSERT INTO item_statuses_backup (id, status_id) SELECT id, status_id FROM items;
    UPDATE items SET status_id = NULL;
    CREATE TABLE statuses_new (
        id TEXT PRIMARY KEY,
        media_kind TEXT NOT NULL,
        category TEXT NOT NULL,
        label TEXT NOT NULL,
        position INTEGER NOT NULL
    ) STRICT;
    INSERT INTO statuses_new (id, media_kind, category, label, position)
        SELECT id, media_kind, category, label, position FROM statuses;
    DROP TABLE statuses;
    ALTER TABLE statuses_new RENAME TO statuses;
    UPDATE items SET status_id = (
        SELECT item_statuses_backup.status_id
        FROM item_statuses_backup
        WHERE item_statuses_backup.id = items.id
    );
    DROP TABLE item_statuses_backup;",
    "CREATE TABLE tags (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL COLLATE NOCASE UNIQUE,
        created_at TEXT NOT NULL
    ) STRICT;
    CREATE TABLE item_tags (
        item_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
        tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
        PRIMARY KEY (item_id, tag_id)
    ) STRICT;
    CREATE TABLE list_tags (
        list_id TEXT NOT NULL REFERENCES lists(id) ON DELETE CASCADE,
        tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
        PRIMARY KEY (list_id, tag_id)
    ) STRICT;",
    "CREATE TABLE seasons (
        id TEXT PRIMARY KEY,
        show_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
        season_number INTEGER NOT NULL,
        status_id TEXT NOT NULL REFERENCES statuses(id),
        finished_at TEXT,
        UNIQUE (show_id, season_number)
    ) STRICT;",
    "ALTER TABLE items ADD COLUMN cover_url TEXT;",
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

    #[test]
    fn a_kind_can_gain_a_second_status_in_the_same_category() {
        let database = Database::open_in_memory().unwrap();

        database
            .connection()
            .execute(
                "INSERT INTO statuses (id, media_kind, category, label, position)
                 VALUES ('movie-someday', 'movie', 'planned', 'Someday', 5)",
                [],
            )
            .unwrap();

        let planned: i64 = database
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM statuses
                 WHERE media_kind = 'movie' AND category = 'planned'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(planned, 2);
    }

    #[test]
    fn items_still_reject_a_status_that_does_not_exist() {
        let database = Database::open_in_memory().unwrap();

        let error = database.connection().execute(
            "INSERT INTO items (id, media_kind, title, created_at, status_id)
             VALUES ('item-1', 'movie', 'Dune', '2026-11-05T00:00:00Z', 'missing')",
            [],
        );

        assert!(error.is_err());
    }

    #[test]
    fn a_database_from_before_statuses_gets_a_planned_status_for_each_item() {
        let temporary = TempDir::new().unwrap();
        let path = temporary.path().join("before-statuses.db");

        {
            let connection = Connection::open(&path).unwrap();
            for migration in &MIGRATIONS[..3] {
                connection.execute_batch(migration).unwrap();
            }
            connection.pragma_update(None, "user_version", 3).unwrap();
            connection
                .execute_batch(
                    "INSERT INTO items (id, media_kind, title, created_at) VALUES
                        ('movie-1', 'movie', 'Dune', '2026-11-05T00:00:00Z'),
                        ('book-1', 'book', 'Piranesi', '2026-11-05T00:00:00Z');",
                )
                .unwrap();
        }

        let database = Database::open(&path).unwrap();

        let mut statement = database
            .connection()
            .prepare("SELECT id, status_id FROM items ORDER BY id")
            .unwrap();
        let statuses: Vec<(String, String)> = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();

        assert_eq!(
            statuses,
            [
                ("book-1".to_owned(), "book-planned".to_owned()),
                ("movie-1".to_owned(), "movie-planned".to_owned()),
            ]
        );
        assert_eq!(schema_version(database.connection()), 8);
    }

    #[test]
    fn a_database_from_before_covers_gets_a_cover_column() {
        let temporary = TempDir::new().unwrap();
        let path = temporary.path().join("before-covers.db");

        {
            let connection = Connection::open(&path).unwrap();
            for migration in &MIGRATIONS[..7] {
                connection.execute_batch(migration).unwrap();
            }
            connection.pragma_update(None, "user_version", 7).unwrap();
            connection
                .execute_batch(
                    "INSERT INTO items (id, media_kind, title, created_at, status_id)
                     VALUES ('show-1', 'show', 'Dark', '2026-11-05T00:00:00Z', 'show-planned');",
                )
                .unwrap();
        }

        let database = Database::open(&path).unwrap();

        let cover: Option<String> = database
            .connection()
            .query_row(
                "SELECT cover_url FROM items WHERE id = 'show-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(cover, None);
        assert_eq!(
            schema_version(database.connection()),
            i64::from(latest_version())
        );
    }

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
                .execute_batch("PRAGMA user_version = 9;")
                .unwrap();
        }

        assert!(matches!(
            Database::open(&path),
            Err(Error::SchemaTooNew {
                found: 9,
                supported: 8
            })
        ));

        let connection = Connection::open(&path).unwrap();
        assert_eq!(schema_version(&connection), 9);
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
