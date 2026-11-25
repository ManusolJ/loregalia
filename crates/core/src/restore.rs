use crate::export::{self, Format, Library};
use crate::storage::Database;
use crate::{Error, Result, dates};
use rusqlite::{
    Connection, ErrorCode, OpenFlags, OptionalExtension, Transaction, TransactionBehavior,
};
use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

const STAGED_FILE: &str = "loregalia-restore.partial";
const SIDE_FILES: [&str; 3] = ["-journal", "-wal", "-shm"];
const REPLACE_CONTENTS: &str = "
    DELETE FROM main.list_tags;
    DELETE FROM main.item_tags;
    DELETE FROM main.list_items;
    DELETE FROM main.seasons;
    DELETE FROM main.tags;
    DELETE FROM main.lists;
    DELETE FROM main.items;
    DELETE FROM main.statuses;
    INSERT INTO main.statuses SELECT * FROM backup.statuses;
    INSERT INTO main.items SELECT * FROM backup.items;
    INSERT INTO main.lists SELECT * FROM backup.lists;
    INSERT INTO main.tags SELECT * FROM backup.tags;
    INSERT INTO main.seasons SELECT * FROM backup.seasons;
    INSERT INTO main.list_items SELECT * FROM backup.list_items;
    INSERT INTO main.item_tags SELECT * FROM backup.item_tags;
    INSERT INTO main.list_tags SELECT * FROM backup.list_tags;";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Contents {
    pub items: usize,
    pub lists: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Restored {
    pub contents: Contents,
    pub previous: PathBuf,
}

struct Staged {
    path: PathBuf,
    contents: Contents,
}

impl Drop for Staged {
    fn drop(&mut self) {
        remove(&self.path);
    }
}

pub fn inspect(library_path: &Path, chosen: &Path) -> Result<Contents> {
    Ok(stage(library_path, chosen)?.contents.clone())
}

pub fn restore(library_path: &Path, chosen: &Path) -> Result<Restored> {
    let staged = stage(library_path, chosen)?;
    let previous = match replace_contents(library_path, &staged.path) {
        Err(error) if error.is_damage() => swap(library_path, &staged.path)?,
        outcome => outcome?,
    };

    Ok(Restored {
        contents: staged.contents.clone(),
        previous,
    })
}

fn stage(library_path: &Path, chosen: &Path) -> Result<Staged> {
    if export::is_same_file(library_path, chosen) {
        return Err(Error::WouldReplaceLibrary);
    }
    let backup = open_backup(chosen).map_err(|error| match error {
        Error::Sqlite(sqlite) if sqlite.sqlite_error_code() == Some(ErrorCode::NotADatabase) => {
            Error::NotALibrary
        }
        other => other,
    })?;
    let mut staged = Staged {
        path: library_path.with_file_name(STAGED_FILE),
        contents: Contents { items: 0, lists: 0 },
    };
    export::verified_copy(&backup, &staged.path)?;

    let library = Library::read(&Database::open(&staged.path)?)?;
    staged.contents = Contents {
        items: library.item_count(),
        lists: library.list_count(),
    };

    Ok(staged)
}

fn open_backup(chosen: &Path) -> Result<Connection> {
    if !chosen.is_file() {
        return Err(std::io::Error::from(ErrorKind::NotFound).into());
    }
    let backup = Connection::open_with_flags(chosen, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let has_lists = backup
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'lists'",
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    let version: i64 = backup.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if !has_lists || version < 1 {
        return Err(Error::NotALibrary);
    }

    Ok(backup)
}

fn replace_contents(library_path: &Path, staged: &Path) -> Result<PathBuf> {
    let database = Database::open(library_path)?;
    let previous = kept_path(library_path, "before-restore");
    let replaced = export::verified_copy(database.connection(), &previous)
        .and_then(|()| copy_tables(database.connection(), staged));
    if let Err(error) = replaced {
        if !error.is_damage() {
            remove(&previous);
        }
        return Err(error);
    }

    Ok(previous)
}

fn copy_tables(library: &Connection, staged: &Path) -> Result<()> {
    let name = staged
        .to_str()
        .ok_or_else(|| std::io::Error::from(ErrorKind::InvalidFilename))?;
    library.execute("ATTACH DATABASE ?1 AS backup", [name])?;
    let transaction = Transaction::new_unchecked(library, TransactionBehavior::Immediate)?;
    transaction.execute_batch(REPLACE_CONTENTS)?;
    transaction.commit()?;
    library.execute("DETACH DATABASE backup", [])?;

    Ok(())
}

fn swap(library_path: &Path, staged: &Path) -> Result<PathBuf> {
    let previous = kept_path(library_path, "damaged");
    std::fs::rename(library_path, &previous)?;
    let moved = SIDE_FILES
        .iter()
        .filter(|suffix| beside(library_path, suffix).exists())
        .try_for_each(|suffix| {
            std::fs::rename(beside(library_path, suffix), beside(&previous, suffix))
        })
        .and_then(|()| std::fs::rename(staged, library_path));
    if let Err(error) = moved {
        for suffix in SIDE_FILES {
            let _ = std::fs::rename(beside(&previous, suffix), beside(library_path, suffix));
        }
        let _ = std::fs::rename(&previous, library_path);
        return Err(error.into());
    }

    Ok(previous)
}

fn kept_path(library_path: &Path, reason: &str) -> PathBuf {
    let name = format!("loregalia-{reason}-{}", dates::today());
    export::with_extension(&library_path.with_file_name(name), Format::Backup)
}

fn beside(path: &Path, suffix: &str) -> PathBuf {
    let mut name: OsString = path.file_name().unwrap_or_default().to_owned();
    name.push(suffix);
    path.with_file_name(name)
}

fn remove(path: &Path) {
    let _ = std::fs::remove_file(path);
}

pub fn failure_message(error: &Error, chosen: &Path) -> String {
    let reason = match error {
        Error::WouldReplaceLibrary => {
            "that file is the library Loregalia is using. Choose a backup of it.".to_owned()
        }
        Error::NotALibrary => {
            "it is not a Loregalia backup. Choose a .db file saved with Back up library.".to_owned()
        }
        Error::SchemaTooNew { .. } => {
            "it was saved by a newer version of Loregalia. Install the latest version first."
                .to_owned()
        }
        Error::Io(io) => match io.kind() {
            ErrorKind::NotFound => "the file is not there any more.".to_owned(),
            ErrorKind::PermissionDenied => {
                "Loregalia is not allowed to use it, or another program has it open.".to_owned()
            }
            ErrorKind::StorageFull => "the disk is full.".to_owned(),
            _ => format!("{error}."),
        },
        Error::Sqlite(sqlite) => match sqlite.sqlite_error_code() {
            Some(ErrorCode::DiskFull) => "the disk is full.".to_owned(),
            Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => {
                "another program is using the library or the backup. Close it and try again."
                    .to_owned()
            }
            Some(ErrorCode::CannotOpen | ErrorCode::PermissionDenied | ErrorCode::ReadOnly) => {
                "Loregalia is not allowed to use it, or another program has it open.".to_owned()
            }
            _ => "the backup is damaged. Choose another one.".to_owned(),
        },
        Error::DamagedCopy => "the backup is damaged. Choose another one.".to_owned(),
        other => format!("{other}."),
    };

    format!(
        "Could not restore {}: {reason} Your library was not changed.",
        chosen.display()
    )
}

#[cfg(test)]
mod tests {
    use super::{
        Contents, REPLACE_CONTENTS, SIDE_FILES, STAGED_FILE, beside, failure_message, inspect,
        restore, swap,
    };
    use crate::export::{self, Format};
    use crate::media::MediaKind;
    use crate::{
        Database, Error, Result, dates, items, list_items, lists, seasons, settings, statuses, tags,
    };
    use rusqlite::Connection;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    const FIRST_MIGRATION: &str = "CREATE TABLE lists (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        created_at TEXT NOT NULL
    ) STRICT;
    INSERT INTO lists VALUES ('old', 'From long ago', '2026-01-01T00:00:00.000Z');
    PRAGMA user_version = 1;";

    struct Folder {
        temporary: TempDir,
        library: PathBuf,
        backup: PathBuf,
    }

    fn fill(path: &Path, show: &str) -> Result<()> {
        let database = Database::open(path)?;
        let show = items::create(&database, MediaKind::Show, show, Some(2022))?;
        let book = items::create(&database, MediaKind::Book, "Piranesi", None)?;
        items::set_status(&database, &book.id, "book-done")?;
        seasons::create(&database, &show.id, 1)?;
        let tag = tags::create(&database, "cosy")?;
        tags::attach_to_item(&database, &show.id, &tag.id)?;
        let list = lists::create(&database, "Autumn")?;
        tags::attach_to_list(&database, &list.id, &tag.id)?;
        list_items::add(&database, &list.id, &book.id)?;
        list_items::add(&database, &list.id, &show.id)?;
        Ok(())
    }

    fn folder() -> Option<Folder> {
        let temporary = TempDir::new().ok()?;
        let library = temporary.path().join("loregalia.db");
        let backup = temporary.path().join("elsewhere").join("backup.db");
        std::fs::create_dir_all(backup.parent()?).ok()?;
        fill(&library, "Severance").ok()?;
        export::save(&library, Format::Backup, &backup).ok()?;
        let database = Database::open(&library).ok()?;
        for item in items::all(&database).ok()? {
            items::delete(&database, &item.id).ok()?;
        }
        for list in lists::all(&database).ok()? {
            lists::delete(&database, &list.id).ok()?;
        }
        items::create(&database, MediaKind::Game, "Added later", None).ok()?;
        tags::create(&database, "later").ok()?;

        Some(Folder {
            temporary,
            library,
            backup,
        })
    }

    fn titles(path: &Path) -> Result<Vec<String>> {
        Ok(items::all(&Database::open(path)?)?
            .into_iter()
            .map(|item| item.title)
            .collect())
    }

    fn everything(path: &Path) -> Result<String> {
        let database = Database::open(path)?;
        Ok(format!(
            "{:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?}",
            items::all(&database)?,
            lists::all(&database)?,
            list_items::memberships(&database)?,
            tags::all(&database)?,
            tags::item_taggings(&database)?,
            tags::list_taggings(&database)?,
            seasons::all_for_every_show(&database)?,
            statuses::all(&database)?,
        ))
    }

    fn files(folder: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(folder)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.path().is_file())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_backup_says_what_it_holds_and_changes_nothing() {
        let folder = folder().unwrap();
        let before = everything(&folder.library).unwrap();

        let contents = inspect(&folder.library, &folder.backup).unwrap();

        assert_eq!(contents, Contents { items: 2, lists: 1 });
        assert_eq!(everything(&folder.library).unwrap(), before);
        assert_eq!(files(folder.temporary.path()), ["loregalia.db"]);
    }

    #[test]
    fn restoring_brings_back_exactly_what_was_backed_up() {
        let folder = folder().unwrap();
        let backup = std::fs::read(&folder.backup).unwrap();

        let restored = restore(&folder.library, &folder.backup).unwrap();

        assert_eq!(restored.contents, Contents { items: 2, lists: 1 });
        assert_eq!(
            everything(&folder.library).unwrap(),
            everything(&folder.backup).unwrap()
        );
        assert_eq!(std::fs::read(&folder.backup).unwrap(), backup);
    }

    #[test]
    fn the_library_that_was_replaced_is_kept_and_can_be_restored() {
        let folder = folder().unwrap();
        let before = everything(&folder.library).unwrap();

        let restored = restore(&folder.library, &folder.backup).unwrap();

        assert_eq!(everything(&restored.previous).unwrap(), before);
        assert_eq!(restored.previous.parent(), folder.library.parent());
        let name = format!("loregalia-before-restore-{}.db", dates::today());
        assert_eq!(files(folder.temporary.path()), [&name, "loregalia.db"]);

        let undone = restore(&folder.library, &restored.previous).unwrap();

        assert_eq!(everything(&folder.library).unwrap(), before);
        assert_ne!(undone.previous, restored.previous);
        assert!(undone.previous.exists() && restored.previous.exists());
    }

    #[test]
    fn settings_stay_as_they_are() {
        let folder = folder().unwrap();
        let database = Database::open(&folder.library).unwrap();
        settings::set_theme(&database, settings::Theme::Light).unwrap();
        settings::set_update_checks(&database, false).unwrap();
        drop(database);

        restore(&folder.library, &folder.backup).unwrap();

        let database = Database::open(&folder.library).unwrap();
        assert_eq!(settings::theme(&database).unwrap(), settings::Theme::Light);
        assert!(!settings::update_checks(&database).unwrap());
    }

    #[test]
    fn every_table_is_restored_or_kept_on_purpose() {
        let database = Database::open_in_memory().unwrap();
        let tables = database
            .select(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap();

        assert!(tables.len() > 8);
        for table in tables.iter().filter(|table| *table != "settings") {
            assert!(
                REPLACE_CONTENTS.contains(&format!("DELETE FROM main.{table};"))
                    && REPLACE_CONTENTS.contains(&format!(
                        "INSERT INTO main.{table} SELECT * FROM backup.{table};"
                    )),
                "{table} is not restored"
            );
        }
    }

    #[test]
    fn a_backup_from_an_older_version_is_brought_up_to_date() {
        let folder = folder().unwrap();
        let old = folder.temporary.path().join("old.db");
        Connection::open(&old)
            .unwrap()
            .execute_batch(FIRST_MIGRATION)
            .unwrap();
        let before = std::fs::read(&old).unwrap();

        let restored = restore(&folder.library, &old).unwrap();

        assert_eq!(restored.contents, Contents { items: 0, lists: 1 });
        let database = Database::open(&folder.library).unwrap();
        assert_eq!(lists::all(&database).unwrap()[0].name, "From long ago");
        assert_eq!(statuses::all(&database).unwrap().len(), 20);
        assert_eq!(std::fs::read(&old).unwrap(), before);
    }

    fn refused(folder: &Folder, chosen: &Path, expected: &str) {
        let before = everything(&folder.library).ok();
        let listed = files(folder.temporary.path());

        for outcome in [
            inspect(&folder.library, chosen).err(),
            restore(&folder.library, chosen).err(),
        ] {
            let message = outcome.map(|error| failure_message(&error, chosen));
            assert!(
                message
                    .as_ref()
                    .is_some_and(|message| message.contains(expected)),
                "{message:?}"
            );
        }
        assert_eq!(everything(&folder.library).ok(), before);
        assert_eq!(files(folder.temporary.path()), listed);
    }

    #[test]
    fn files_that_are_not_backups_are_refused() {
        let folder = folder().unwrap();
        let text = folder.temporary.path().join("export.json");
        std::fs::write(&text, "{ \"format\": \"loregalia\" }".repeat(200)).unwrap();
        let empty = folder.temporary.path().join("empty.db");
        std::fs::write(&empty, "").unwrap();
        let other = folder.temporary.path().join("other.db");
        Connection::open(&other)
            .unwrap()
            .execute_batch("CREATE TABLE places (id TEXT); PRAGMA user_version = 999;")
            .unwrap();
        let unversioned = folder.temporary.path().join("unversioned.db");
        Connection::open(&unversioned)
            .unwrap()
            .execute_batch("CREATE TABLE lists (id TEXT);")
            .unwrap();

        for chosen in [&text, &empty, &other, &unversioned] {
            refused(&folder, chosen, "not a Loregalia backup");
        }
    }

    #[test]
    fn a_backup_from_a_newer_version_is_refused() {
        let folder = folder().unwrap();
        Connection::open(&folder.backup)
            .unwrap()
            .execute_batch("PRAGMA user_version = 999;")
            .unwrap();

        refused(&folder, &folder.backup, "newer version of Loregalia");
    }

    #[test]
    fn the_library_in_use_and_missing_files_are_refused() {
        let folder = folder().unwrap();
        let gone = folder.temporary.path().join("gone.db");

        refused(
            &folder,
            &folder.library,
            "is the library Loregalia is using",
        );
        refused(&folder, &gone, "not there any more");
        assert!(!gone.exists());
    }

    #[test]
    fn a_backup_cut_short_is_refused() {
        let folder = folder().unwrap();
        let bytes = std::fs::read(&folder.backup).unwrap();
        std::fs::write(&folder.backup, &bytes[..bytes.len() / 2]).unwrap();

        refused(&folder, &folder.backup, "the backup is damaged");
    }

    #[test]
    fn a_backup_with_values_nobody_can_read_is_refused() {
        let folder = folder().unwrap();
        Connection::open(&folder.backup)
            .unwrap()
            .execute_batch("UPDATE items SET media_kind = 'vinyl';")
            .unwrap();

        refused(&folder, &folder.backup, "the backup is damaged");
    }

    #[test]
    fn a_backup_with_broken_links_leaves_the_library_as_it_was() {
        let folder = folder().unwrap();
        Connection::open(&folder.backup)
            .unwrap()
            .execute_batch(
                "PRAGMA foreign_keys = OFF;
                 INSERT INTO list_items VALUES ('nowhere', 'nothing', 9);",
            )
            .unwrap();
        let before = everything(&folder.library).unwrap();

        let error = restore(&folder.library, &folder.backup).unwrap_err();

        assert!(failure_message(&error, &folder.backup).contains("the backup is damaged"));
        assert_eq!(everything(&folder.library).unwrap(), before);
        assert_eq!(files(folder.temporary.path()), ["loregalia.db"]);
    }

    #[test]
    fn a_library_that_is_busy_is_left_alone() {
        let folder = folder().unwrap();
        let before = everything(&folder.library).unwrap();
        let other = Connection::open(&folder.library).unwrap();
        other.execute_batch("BEGIN EXCLUSIVE;").unwrap();

        let error = restore(&folder.library, &folder.backup).unwrap_err();
        other.execute_batch("ROLLBACK;").unwrap();

        assert!(failure_message(&error, &folder.backup).contains("another program is using"));
        assert_eq!(everything(&folder.library).unwrap(), before);
        assert_eq!(files(folder.temporary.path()), ["loregalia.db"]);
    }

    #[test]
    fn a_damaged_library_is_set_aside_and_replaced() {
        let folder = folder().unwrap();
        std::fs::write(&folder.library, "ruined").unwrap();

        let restored = restore(&folder.library, &folder.backup).unwrap();

        assert_eq!(std::fs::read(&restored.previous).unwrap(), b"ruined");
        assert_eq!(titles(&folder.library).unwrap().len(), 2);
        assert!(!folder.library.with_file_name(STAGED_FILE).exists());
    }

    #[test]
    fn unfinished_changes_move_away_with_the_damaged_library() {
        let folder = folder().unwrap();
        for suffix in SIDE_FILES {
            std::fs::write(beside(&folder.library, suffix), suffix).unwrap();
        }

        let previous = swap(&folder.library, &folder.backup).unwrap();

        for suffix in SIDE_FILES {
            assert!(!beside(&folder.library, suffix).exists());
            assert_eq!(
                std::fs::read(beside(&previous, suffix)).unwrap(),
                suffix.as_bytes()
            );
        }
        assert_eq!(titles(&folder.library).unwrap().len(), 2);
    }

    #[test]
    fn a_damaged_library_that_cannot_be_replaced_stays_where_it_was() {
        let folder = folder().unwrap();
        std::fs::write(&folder.library, "ruined").unwrap();
        std::fs::write(beside(&folder.library, "-journal"), "kept").unwrap();

        let outcome = swap(&folder.library, &folder.temporary.path().join("missing"));

        assert!(outcome.is_err());
        assert_eq!(std::fs::read(&folder.library).unwrap(), b"ruined");
        assert_eq!(
            files(folder.temporary.path()),
            ["loregalia.db", "loregalia.db-journal"]
        );
    }

    #[test]
    fn damage_inside_a_library_that_still_opens_is_replaced_too() {
        let folder = folder().unwrap();
        fill(&folder.library, &"x".repeat(60_000)).unwrap();
        let mut bytes = std::fs::read(&folder.library).unwrap();
        let start = bytes.len() / 2;
        bytes[start..].fill(0xAB);
        std::fs::write(&folder.library, &bytes).unwrap();

        let restored = restore(&folder.library, &folder.backup).unwrap();

        assert_eq!(std::fs::read(&restored.previous).unwrap(), bytes);
        assert_eq!(titles(&folder.library).unwrap().len(), 2);
    }

    #[test]
    fn failures_say_what_to_do_and_that_nothing_changed() {
        let path = Path::new("x.db");
        let io = |kind| Error::Io(std::io::Error::from(kind));

        for (error, expected) in [
            (Error::WouldReplaceLibrary, "Choose a backup"),
            (Error::NotALibrary, "Back up library"),
            (
                Error::SchemaTooNew {
                    found: 999,
                    supported: 1,
                },
                "Install the latest version",
            ),
            (io(std::io::ErrorKind::StorageFull), "disk is full"),
            (io(std::io::ErrorKind::PermissionDenied), "not allowed"),
            (Error::DamagedCopy, "is damaged"),
        ] {
            let message = failure_message(&error, path);
            assert!(message.starts_with("Could not restore x.db:"), "{message}");
            assert!(message.contains(expected), "{message}");
            assert!(message.ends_with("Your library was not changed."));
        }
    }
}
