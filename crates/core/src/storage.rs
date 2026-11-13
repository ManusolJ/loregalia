use crate::{Error, Result};
use rusqlite::{Connection, ErrorCode, Params, Row, types::Type};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub const APP_FOLDER: &str = "loregalia";
pub const LEGACY_APP_FOLDER: &str = "com.manu.loregalia";
pub const DATABASE_FILE: &str = "loregalia.db";
pub const PORTABLE_MARKER: &str = "portable";
pub const PORTABLE_DATA_FOLDER: &str = "data";

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
    "CREATE TABLE settings (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    ) STRICT;",
    "ALTER TABLE items ADD COLUMN source_data TEXT;
     ALTER TABLE items ADD COLUMN source_refreshed_at TEXT;",
];

fn latest_version() -> u32 {
    MIGRATIONS.len() as u32
}

fn data_base(env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    non_empty(env("APPDATA"))
        .or_else(|| non_empty(env("XDG_DATA_HOME")))
        .or_else(|| non_empty(env("HOME")).map(|home| home.join(".local").join("share")))
}

pub fn data_directory(env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    data_base(env).map(|base| base.join(APP_FOLDER))
}

pub fn legacy_data_directory(env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    data_base(env).map(|base| base.join(LEGACY_APP_FOLDER))
}

pub fn portable_data_directory(executable_directory: &Path) -> Option<PathBuf> {
    executable_directory
        .join(PORTABLE_MARKER)
        .is_file()
        .then(|| executable_directory.join(PORTABLE_DATA_FOLDER))
}

#[derive(Debug, Eq, PartialEq)]
pub struct DataLocation {
    pub directory: PathBuf,
    pub note: Option<String>,
}

pub fn adopt_legacy_directory(current: &Path, legacy: &Path) -> DataLocation {
    if current.exists() || !legacy.is_dir() {
        let note = (current.exists() && legacy.is_dir()).then(|| {
            format!(
                "an older data folder at {} was left as it is; Loregalia uses {}",
                legacy.display(),
                current.display()
            )
        });

        return DataLocation {
            directory: current.to_path_buf(),
            note,
        };
    }

    match std::fs::rename(legacy, current) {
        Ok(()) => DataLocation {
            directory: current.to_path_buf(),
            note: Some(format!(
                "moved the data folder from {} to {}",
                legacy.display(),
                current.display()
            )),
        },
        Err(error) => DataLocation {
            directory: legacy.to_path_buf(),
            note: Some(format!(
                "could not move the data folder from {} to {} ({error}); using the old folder",
                legacy.display(),
                current.display()
            )),
        },
    }
}

pub fn locate_data_directory() -> Result<DataLocation> {
    let portable = std::env::current_exe()
        .ok()
        .and_then(|executable| executable.parent().map(Path::to_path_buf))
        .and_then(|directory| portable_data_directory(&directory));
    if let Some(directory) = portable {
        return Ok(DataLocation {
            note: Some(format!("portable mode, data in {}", directory.display())),
            directory,
        });
    }

    let environment = |name: &str| std::env::var_os(name);
    let current = data_directory(environment).ok_or(Error::DataDirectoryUnavailable)?;

    Ok(match legacy_data_directory(environment) {
        Some(legacy) => adopt_legacy_directory(&current, &legacy),
        None => DataLocation {
            directory: current,
            note: None,
        },
    })
}

pub fn database_in(directory: &Path) -> PathBuf {
    directory.join(DATABASE_FILE)
}

pub fn open_failure_message(error: &Error, path: &Path) -> String {
    let unchanged = format!("The file was not changed: {}", path.display());
    let reason = match error {
        Error::SchemaTooNew { .. } => format!(
            "it was saved by a newer version of Loregalia. Install the latest version to open \
             it. {unchanged}"
        ),
        Error::Sqlite(sqlite) => match sqlite.sqlite_error_code() {
            Some(ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt) => format!(
                "the file is damaged or is not a Loregalia library. Restore it from a backup, or \
                 move it away to start a new library. {unchanged}"
            ),
            Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => format!(
                "another program is using it. Close that program and start Loregalia again. \
                 {unchanged}"
            ),
            Some(ErrorCode::ReadOnly | ErrorCode::CannotOpen | ErrorCode::PermissionDenied) => {
                format!("Loregalia is not allowed to write to it. {unchanged}")
            }
            Some(ErrorCode::DiskFull) => {
                format!("the disk is full. Free some space and start Loregalia again. {unchanged}")
            }
            _ => error.to_string(),
        },
        other => other.to_string(),
    };
    format!("Could not open the database: {reason}")
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

    pub(crate) fn select<T>(
        &self,
        sql: &str,
        params: impl Params,
        map: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
    ) -> Result<Vec<T>> {
        let mut statement = self.connection.prepare(sql)?;
        let rows = statement.query_map(params, map)?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Error::from)
    }

    fn from_connection(mut connection: Connection) -> Result<Self> {
        connection.pragma_update(None, "foreign_keys", true)?;
        migrate(&mut connection)?;

        Ok(Self { connection })
    }
}

pub(crate) fn keyed_column<T>(
    row: &Row<'_>,
    index: usize,
    from_key: impl Fn(&str) -> Option<T>,
) -> rusqlite::Result<T> {
    let value: String = row.get(index)?;
    from_key(&value).ok_or_else(|| rusqlite::Error::InvalidColumnType(index, value, Type::Text))
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
        APP_FOLDER, DATABASE_FILE, Database, LEGACY_APP_FOLDER, MIGRATIONS, PORTABLE_MARKER,
        adopt_legacy_directory, data_directory, database_in, keyed_column, latest_version,
        legacy_data_directory, open_failure_message, portable_data_directory,
    };

    fn failure_opening(path: &std::path::Path) -> Option<String> {
        Database::open(path)
            .err()
            .map(|error| open_failure_message(&error, path))
    }

    #[test]
    fn a_newer_database_asks_for_the_latest_version_and_names_the_file() {
        let temporary = TempDir::new().unwrap();
        let path = temporary.path().join(DATABASE_FILE);
        Connection::open(&path)
            .unwrap()
            .execute_batch("PRAGMA user_version = 999;")
            .unwrap();

        let message = failure_opening(&path).unwrap();

        assert!(message.starts_with("Could not open the database:"));
        assert!(message.contains("newer version of Loregalia"));
        assert!(message.contains(&path.display().to_string()));
    }

    #[test]
    fn a_damaged_database_says_so_instead_of_a_sqlite_error() {
        let temporary = TempDir::new().unwrap();
        let path = temporary.path().join(DATABASE_FILE);
        std::fs::write(&path, b"this is not a database at all, just some text").unwrap();

        let message = failure_opening(&path).unwrap();

        assert!(message.starts_with("Could not open the database:"));
        assert!(message.contains("damaged or is not a Loregalia library"));
        assert!(message.contains("was not changed"));
    }

    #[test]
    fn other_open_failures_keep_their_description() {
        let error = Error::Io(std::io::Error::other("disk unplugged"));

        assert_eq!(
            open_failure_message(&error, std::path::Path::new("loregalia.db")),
            "Could not open the database: storage i/o error: disk unplugged"
        );
    }

    #[test]
    fn the_data_folder_is_named_loregalia() {
        let directory =
            data_directory(environment(&[("APPDATA", r"C:\Users\me\AppData\Roaming")])).unwrap();
        let legacy =
            legacy_data_directory(environment(&[("APPDATA", r"C:\Users\me\AppData\Roaming")]))
                .unwrap();

        assert_eq!(directory.file_name().unwrap(), "loregalia");
        assert_eq!(legacy.file_name().unwrap(), "com.manu.loregalia");
        assert_eq!(directory.parent(), legacy.parent());
    }

    #[test]
    fn the_old_folder_moves_to_the_new_name_with_its_data() {
        let temporary = TempDir::new().unwrap();
        let legacy = temporary.path().join(LEGACY_APP_FOLDER);
        let current = temporary.path().join(APP_FOLDER);
        std::fs::create_dir_all(&legacy).unwrap();
        {
            let database = Database::open(database_in(&legacy)).unwrap();
            crate::lists::create(&database, "Watchlist").unwrap();
        }

        let location = adopt_legacy_directory(&current, &legacy);

        assert_eq!(location.directory, current);
        assert!(location.note.unwrap().starts_with("moved the data folder"));
        assert!(!legacy.exists());
        let database = Database::open(database_in(&current)).unwrap();
        let names: Vec<String> = crate::lists::all(&database)
            .unwrap()
            .into_iter()
            .map(|list| list.name)
            .collect();
        assert_eq!(names, ["Watchlist"]);
    }

    #[test]
    fn a_fresh_install_uses_the_new_folder_silently() {
        let temporary = TempDir::new().unwrap();

        let location = adopt_legacy_directory(
            &temporary.path().join(APP_FOLDER),
            &temporary.path().join(LEGACY_APP_FOLDER),
        );

        assert_eq!(location.directory, temporary.path().join(APP_FOLDER));
        assert_eq!(location.note, None);
    }

    #[test]
    fn an_existing_new_folder_wins_and_the_old_one_is_left_alone() {
        let temporary = TempDir::new().unwrap();
        let legacy = temporary.path().join(LEGACY_APP_FOLDER);
        let current = temporary.path().join(APP_FOLDER);
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::create_dir_all(&current).unwrap();
        std::fs::write(legacy.join("keep.txt"), "old").unwrap();

        let location = adopt_legacy_directory(&current, &legacy);

        assert_eq!(location.directory, current);
        assert!(location.note.unwrap().contains("left as it is"));
        assert!(legacy.join("keep.txt").is_file());
    }

    #[test]
    fn a_failed_move_keeps_using_the_old_folder() {
        let temporary = TempDir::new().unwrap();
        let legacy = temporary.path().join(LEGACY_APP_FOLDER);
        let current = temporary.path().join("missing-parent").join(APP_FOLDER);
        std::fs::create_dir_all(&legacy).unwrap();

        let location = adopt_legacy_directory(&current, &legacy);

        assert_eq!(location.directory, legacy);
        assert!(location.note.unwrap().starts_with("could not move"));
        assert!(legacy.is_dir());
    }

    #[test]
    fn a_portable_marker_keeps_data_beside_the_executable() {
        let temporary = TempDir::new().unwrap();
        assert_eq!(portable_data_directory(temporary.path()), None);

        std::fs::write(temporary.path().join(PORTABLE_MARKER), "").unwrap();

        assert_eq!(
            portable_data_directory(temporary.path()),
            Some(temporary.path().join("data"))
        );
    }
    use crate::Error;
    use crate::media::MediaKind;
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
        assert_eq!(schema_version(database.connection()), 10);
    }

    #[test]
    fn a_database_from_before_settings_gets_the_table() {
        let temporary = TempDir::new().unwrap();
        let path = temporary.path().join("before-settings.db");

        {
            let connection = Connection::open(&path).unwrap();
            for migration in &MIGRATIONS[..8] {
                connection.execute_batch(migration).unwrap();
            }
            connection.pragma_update(None, "user_version", 8).unwrap();
            connection
                .execute_batch(
                    "INSERT INTO items (id, media_kind, title, created_at, status_id)
                     VALUES ('movie-1', 'movie', 'Dune', '2026-11-05T00:00:00Z', 'movie-planned');",
                )
                .unwrap();
        }

        let database = Database::open(&path).unwrap();

        crate::settings::set(&database, "theme", "light").unwrap();
        assert_eq!(
            crate::settings::get(&database, "theme").unwrap(),
            Some("light".to_owned())
        );
        let title: String = database
            .connection()
            .query_row("SELECT title FROM items WHERE id = 'movie-1'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(title, "Dune");
        assert_eq!(schema_version(database.connection()), 10);
    }

    #[test]
    fn a_database_from_before_source_data_gets_the_columns() {
        let temporary = TempDir::new().unwrap();
        let path = temporary.path().join("before-source-data.db");

        {
            let connection = Connection::open(&path).unwrap();
            for migration in &MIGRATIONS[..9] {
                connection.execute_batch(migration).unwrap();
            }
            connection.pragma_update(None, "user_version", 9).unwrap();
            connection
                .execute_batch(
                    "INSERT INTO items (id, media_kind, title, source, external_id, created_at, status_id)
                     VALUES ('movie-1', 'movie', 'Dune', 'tmdb', '438631',
                             '2026-11-05T00:00:00Z', 'movie-planned');",
                )
                .unwrap();
        }

        let database = Database::open(&path).unwrap();

        let (source_data, refreshed_at): (Option<String>, Option<String>) = database
            .connection()
            .query_row(
                "SELECT source_data, source_refreshed_at FROM items WHERE id = 'movie-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(source_data, None);
        assert_eq!(refreshed_at, None);
        assert_eq!(schema_version(database.connection()), 10);
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
            .join("loregalia")
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
                .execute_batch("PRAGMA user_version = 11;")
                .unwrap();
        }

        assert!(matches!(
            Database::open(&path),
            Err(Error::SchemaTooNew {
                found: 11,
                supported: 10
            })
        ));

        let connection = Connection::open(&path).unwrap();
        assert_eq!(schema_version(&connection), 11);
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
    fn select_collects_mapped_rows() {
        let database = Database::open_in_memory().unwrap();

        let ids = database
            .select(
                "SELECT id FROM statuses WHERE media_kind = ?1 ORDER BY position",
                ["book"],
                |row| row.get::<_, String>(0),
            )
            .unwrap();

        assert_eq!(ids.len(), 5);
        assert_eq!(ids[0], "book-planned");
    }

    #[test]
    fn a_keyed_column_rejects_unknown_keys() {
        let database = Database::open_in_memory().unwrap();
        database
            .connection()
            .execute(
                "INSERT INTO statuses (id, media_kind, category, label, position)
                 VALUES ('odd', 'comic', 'planned', 'To read', 0)",
                [],
            )
            .unwrap();

        let known = database
            .select(
                "SELECT media_kind FROM statuses WHERE id = 'book-done'",
                [],
                |row| keyed_column(row, 0, MediaKind::from_key),
            )
            .unwrap();
        let unknown = database.select(
            "SELECT media_kind FROM statuses WHERE id = 'odd'",
            [],
            |row| keyed_column(row, 0, MediaKind::from_key),
        );

        assert_eq!(known, [MediaKind::Book]);
        assert!(unknown.is_err());
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
