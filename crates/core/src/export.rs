use crate::storage::Database;
use crate::{Error, Result, items, list_items, lists, seasons, statuses, tags};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const JSON_FORMAT: &str = "loregalia";
const JSON_VERSION: u32 = 1;
const MAX_COPIES: usize = 1_000;
const BYTE_ORDER_MARK: &str = "\u{feff}";
const CSV_HEADER: [&str; 11] = [
    "Type",
    "Title",
    "Year",
    "Status",
    "Finished",
    "Added",
    "Tags",
    "Lists",
    "Seasons",
    "Source",
    "Source ID",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    Backup,
    Json,
    Csv,
}

impl Format {
    pub const ALL: [Self; 3] = [Self::Backup, Self::Json, Self::Csv];

    pub const fn key(self) -> &'static str {
        match self {
            Self::Backup => "backup",
            Self::Json => "json",
            Self::Csv => "csv",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Backup => "Back up library",
            Self::Json => "Export as JSON",
            Self::Csv => "Export as CSV",
        }
    }

    const fn extension(self) -> &'static str {
        match self {
            Self::Backup => "db",
            Self::Json => "json",
            Self::Csv => "csv",
        }
    }

    pub fn file_name(self, today: &str) -> String {
        match self {
            Self::Backup => format!("loregalia-backup-{today}.db"),
            Self::Json | Self::Csv => format!("loregalia-{today}.{}", self.extension()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Saved {
    pub path: PathBuf,
    pub items: usize,
}

#[derive(Serialize)]
struct Document<'a> {
    format: &'static str,
    version: u32,
    app_version: &'static str,
    exported_at: String,
    items: Vec<ItemEntry<'a>>,
    lists: Vec<ListEntry<'a>>,
    tags: Vec<&'a str>,
}

#[derive(Serialize)]
struct ItemEntry<'a> {
    id: &'a str,
    kind: &'static str,
    title: &'a str,
    year: Option<i32>,
    release_date: Option<&'a str>,
    status: &'a str,
    status_category: &'static str,
    finished_at: Option<&'a str>,
    added_at: &'a str,
    source: Option<&'a str>,
    source_id: Option<&'a str>,
    tags: Vec<&'a str>,
    seasons: Vec<SeasonEntry<'a>>,
}

#[derive(Serialize)]
struct SeasonEntry<'a> {
    number: i32,
    status: &'a str,
    status_category: &'static str,
    finished_at: Option<&'a str>,
}

#[derive(Serialize)]
struct ListEntry<'a> {
    id: &'a str,
    name: &'a str,
    tags: Vec<&'a str>,
    items: Vec<&'a str>,
}

struct Library {
    items: Vec<items::Item>,
    added: HashMap<String, (String, Option<String>)>,
    statuses: Vec<statuses::Status>,
    lists: Vec<lists::List>,
    memberships: Vec<list_items::Membership>,
    tags: Vec<tags::Tag>,
    item_tags: Vec<tags::Tagging>,
    list_tags: Vec<tags::Tagging>,
    seasons: Vec<seasons::Season>,
}

impl Library {
    fn read(database: &Database) -> Result<Self> {
        let snapshot = database.connection().unchecked_transaction()?;
        let added = database
            .select(
                "SELECT id, created_at, date(created_at, 'localtime') FROM items",
                [],
                |row| Ok((row.get(0)?, (row.get(1)?, row.get(2)?))),
            )?
            .into_iter()
            .collect();
        let library = Self {
            items: items::all(database)?,
            added,
            statuses: statuses::all(database)?,
            lists: lists::all(database)?,
            memberships: list_items::memberships(database)?,
            tags: tags::all(database)?,
            item_tags: tags::item_taggings(database)?,
            list_tags: tags::list_taggings(database)?,
            seasons: seasons::all_for_every_show(database)?,
        };
        snapshot.finish()?;

        Ok(library)
    }

    fn status(&self, id: &str) -> (&str, &'static str) {
        self.statuses
            .iter()
            .find(|status| status.id == id)
            .map_or(("", ""), |status| {
                (status.label.as_str(), status.category.key())
            })
    }

    fn tag_names<'a>(&'a self, links: &'a [tags::Tagging], owner: &str) -> Vec<&'a str> {
        let mut names: Vec<&str> = links
            .iter()
            .filter(|link| link.owner_id == owner)
            .filter_map(|link| self.tags.iter().find(|tag| tag.id == link.tag_id))
            .map(|tag| tag.name.as_str())
            .collect();
        names.sort_unstable();
        names
    }

    fn seasons_of<'a>(
        &'a self,
        item: &'a items::Item,
    ) -> impl Iterator<Item = &'a seasons::Season> {
        self.seasons
            .iter()
            .filter(move |season| season.show_id == item.id)
    }

    fn added(&self, item: &items::Item) -> (&str, &str) {
        self.added.get(&item.id).map_or(("", ""), |(at, on)| {
            (at.as_str(), on.as_deref().unwrap_or_default())
        })
    }
}

pub fn json(database: &Database) -> Result<String> {
    let library = Library::read(database)?;
    let document = Document {
        format: JSON_FORMAT,
        version: JSON_VERSION,
        app_version: env!("CARGO_PKG_VERSION"),
        exported_at: crate::log::timestamp(SystemTime::now()),
        items: library
            .items
            .iter()
            .map(|item| {
                let (status, status_category) = library.status(&item.status_id);
                ItemEntry {
                    id: &item.id,
                    kind: item.media_kind.key(),
                    title: &item.title,
                    year: item.release_year,
                    release_date: item.release_date.as_deref(),
                    status,
                    status_category,
                    finished_at: item.finished_at.as_deref(),
                    added_at: library.added(item).0,
                    source: item.source.as_deref(),
                    source_id: item.external_id.as_deref(),
                    tags: library.tag_names(&library.item_tags, &item.id),
                    seasons: library
                        .seasons_of(item)
                        .map(|season| {
                            let (status, status_category) = library.status(&season.status_id);
                            SeasonEntry {
                                number: season.season_number,
                                status,
                                status_category,
                                finished_at: season.finished_at.as_deref(),
                            }
                        })
                        .collect(),
                }
            })
            .collect(),
        lists: library
            .lists
            .iter()
            .map(|list| ListEntry {
                id: &list.id,
                name: &list.name,
                tags: library.tag_names(&library.list_tags, &list.id),
                items: library
                    .memberships
                    .iter()
                    .filter(|membership| membership.list_id == list.id)
                    .map(|membership| membership.item_id.as_str())
                    .collect(),
            })
            .collect(),
        tags: library.tags.iter().map(|tag| tag.name.as_str()).collect(),
    };

    Ok(serde_json::to_string_pretty(&document)?)
}

pub fn csv(database: &Database) -> Result<String> {
    let library = Library::read(database)?;
    let mut text = String::from(BYTE_ORDER_MARK);
    push_row(&mut text, CSV_HEADER);

    for item in &library.items {
        let lists: Vec<&str> = library
            .lists
            .iter()
            .filter(|list| {
                library.memberships.iter().any(|membership| {
                    membership.list_id == list.id && membership.item_id == item.id
                })
            })
            .map(|list| list.name.as_str())
            .collect();
        let seasons: Vec<String> = library
            .seasons_of(item)
            .map(|season| {
                let (status, _) = library.status(&season.status_id);
                format!("{} {status}", season.season_number)
            })
            .collect();
        let year = item.release_year.map(|year| year.to_string());

        push_row(
            &mut text,
            [
                item.media_kind.label(),
                &item.title,
                year.as_deref().unwrap_or_default(),
                library.status(&item.status_id).0,
                item.finished_on.as_deref().unwrap_or_default(),
                library.added(item).1,
                &library.tag_names(&library.item_tags, &item.id).join("; "),
                &lists.join("; "),
                &seasons.join("; "),
                item.source.as_deref().unwrap_or_default(),
                item.external_id.as_deref().unwrap_or_default(),
            ],
        );
    }

    Ok(text)
}

fn push_row<'a>(text: &mut String, cells: impl IntoIterator<Item = &'a str>) {
    let row: Vec<String> = cells.into_iter().map(cell).collect();
    text.push_str(&row.join(","));
    text.push_str("\r\n");
}

fn cell(value: &str) -> String {
    let safe = if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else {
        value.to_owned()
    };

    if safe.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", safe.replace('"', "\"\""))
    } else {
        safe
    }
}

pub fn save(library_path: &Path, format: Format, chosen: &Path) -> Result<Saved> {
    let target = with_extension(chosen, format);
    if is_same_file(library_path, &target) {
        return Err(Error::WouldReplaceLibrary);
    }
    let database = Database::open(library_path)?;
    let partial = partial_path(&target);
    let written = match format {
        Format::Backup => copy_library(&database, &partial),
        Format::Json => json(&database).and_then(|text| write(&partial, &text)),
        Format::Csv => csv(&database).and_then(|text| write(&partial, &text)),
    }
    .and_then(|()| std::fs::rename(&partial, &target).map_err(Error::from));
    if let Err(error) = written {
        let _ = std::fs::remove_file(&partial);
        return Err(error);
    }

    Ok(Saved {
        path: target,
        items: items::all(&database)?.len(),
    })
}

fn write(path: &Path, text: &str) -> Result<()> {
    let mut file = std::fs::File::create(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

fn copy_library(database: &Database, partial: &Path) -> Result<()> {
    match std::fs::remove_file(partial) {
        Err(error) if error.kind() != ErrorKind::NotFound => return Err(error.into()),
        _ => {}
    }
    let name = partial
        .to_str()
        .ok_or_else(|| std::io::Error::from(ErrorKind::InvalidFilename))?;
    database.connection().execute("VACUUM INTO ?1", [name])?;

    let copy = Connection::open_with_flags(partial, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let check: String = copy.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if check == "ok" {
        Ok(())
    } else {
        Err(Error::DamagedCopy)
    }
}

fn with_extension(chosen: &Path, format: Format) -> PathBuf {
    if chosen.extension().is_some() {
        return chosen.to_owned();
    }
    let named = chosen.with_extension(format.extension());
    let stem = chosen.file_name().unwrap_or_default().to_string_lossy();

    std::iter::once(named.clone())
        .chain(
            (2..=MAX_COPIES).map(|copy| {
                chosen.with_file_name(format!("{stem} ({copy}).{}", format.extension()))
            }),
        )
        .find(|candidate| !candidate.exists())
        .unwrap_or(named)
}

fn partial_path(target: &Path) -> PathBuf {
    let mut name: OsString = target.file_name().unwrap_or_default().to_owned();
    name.push(".partial");
    target.with_file_name(name)
}

fn is_same_file(library_path: &Path, target: &Path) -> bool {
    match (library_path.canonicalize(), target.canonicalize()) {
        (Ok(library), Ok(target)) => library == target,
        _ => library_path == target,
    }
}

pub fn failure_message(error: &Error, path: &Path) -> String {
    let reason = match error {
        Error::WouldReplaceLibrary => {
            "that file is the library Loregalia is using. Choose another name or folder.".to_owned()
        }
        Error::Io(io) => match io.kind() {
            ErrorKind::PermissionDenied => {
                "Loregalia is not allowed to write there. Choose another folder, or close the \
                 program that has the file open."
                    .to_owned()
            }
            ErrorKind::StorageFull => "the disk is full.".to_owned(),
            ErrorKind::NotFound => "the folder does not exist any more.".to_owned(),
            _ => format!("{error}."),
        },
        Error::DamagedCopy => "the copy could not be verified. Try again.".to_owned(),
        other => format!("{other}."),
    };

    format!("Could not save {}: {reason}", path.display())
}

#[cfg(test)]
mod tests {
    use super::{Format, cell, csv, failure_message, json, partial_path, save};
    use crate::media::MediaKind;
    use crate::providers::Source;
    use crate::{Database, Error, items, list_items, lists, seasons, tags};
    use serde_json::Value;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    fn seeded(path: &Path) -> Database {
        let database = Database::open(path).unwrap();
        let show = items::create_from_lookup(
            &database,
            MediaKind::Show,
            "Pokémon, \"the\" show",
            Some(1997),
            &items::Lookup {
                source: Source::TvMaze,
                external_id: "42".to_owned(),
                cover_url: Some("https://static.tvmaze.com/x.jpg".to_owned()),
            },
        )
        .unwrap();
        let book = items::create(&database, MediaKind::Book, "Piranesi", None).unwrap();
        items::set_status(&database, &book.id, "book-done").unwrap();
        let season = seasons::create(&database, &show.id, 1).unwrap();
        seasons::set_status(&database, &season.id, "show-done").unwrap();
        seasons::create(&database, &show.id, 2).unwrap();
        let tag = tags::create(&database, "cosy").unwrap();
        tags::attach_to_item(&database, &show.id, &tag.id).unwrap();
        let list = lists::create(&database, "Autumn").unwrap();
        tags::attach_to_list(&database, &list.id, &tag.id).unwrap();
        list_items::add(&database, &list.id, &book.id).unwrap();
        list_items::add(&database, &list.id, &show.id).unwrap();
        let data = items::SourceData {
            title: show.title.clone(),
            release_year: show.release_year,
            cover_url: show.cover_url.clone(),
            details: Some(crate::details::Details {
                synopsis: Some("A synopsis that belongs to the source.".to_owned()),
                ..Default::default()
            }),
        };
        items::apply_refresh(&database, &show.id, data, "2026-11-25T00:00:00.000Z").unwrap();
        database
    }

    fn library() -> (TempDir, PathBuf) {
        let temporary = TempDir::new().unwrap();
        let path = temporary.path().join("loregalia.db");
        seeded(&path);
        (temporary, path)
    }

    #[test]
    fn json_holds_everything_the_user_entered() {
        let (_folder, path) = library();
        let document: Value =
            serde_json::from_str(&json(&Database::open(&path).unwrap()).unwrap()).unwrap();

        assert_eq!(document["format"], "loregalia");
        assert_eq!(document["version"], 1);
        let show = &document["items"][0];
        assert_eq!(show["title"], "Pokémon, \"the\" show");
        assert_eq!(show["kind"], "show");
        assert_eq!(show["year"], 1997);
        assert_eq!(show["source"], "tvmaze");
        assert_eq!(show["source_id"], "42");
        assert_eq!(show["tags"][0], "cosy");
        assert_eq!(show["seasons"][0]["number"], 1);
        assert_eq!(show["seasons"][0]["status_category"], "done");
        assert!(show["seasons"][0]["finished_at"].is_string());
        assert_eq!(show["seasons"][1]["status"], "To watch");
        let book = &document["items"][1];
        assert_eq!(book["status"], "Read");
        assert!(book["finished_at"].is_string());
        assert!(book["added_at"].as_str().unwrap().ends_with('Z'));
        let list = &document["lists"][0];
        assert_eq!(list["name"], "Autumn");
        assert_eq!(list["tags"][0], "cosy");
        assert_eq!(list["items"][0], book["id"]);
        assert_eq!(list["items"][1], show["id"]);
        assert_eq!(document["tags"][0], "cosy");
    }

    #[test]
    fn exports_leave_out_what_belongs_to_the_sources() {
        let (_folder, path) = library();
        let database = Database::open(&path).unwrap();

        for text in [json(&database).unwrap(), csv(&database).unwrap()] {
            assert!(!text.contains("belongs to the source"));
            assert!(!text.contains("static.tvmaze.com"));
        }
    }

    #[test]
    fn csv_has_one_row_for_each_item() {
        let (_folder, path) = library();
        let text = csv(&Database::open(&path).unwrap()).unwrap();
        let lines: Vec<&str> = text.trim_end().split("\r\n").collect();

        assert!(text.starts_with('\u{feff}'));
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("Type,Title,Year,Status,Finished,Added"));
        assert!(lines[1].starts_with("Show,\"Pokémon, \"\"the\"\" show\",1997,To watch,,20"));
        assert!(lines[1].ends_with(",cosy,Autumn,1 Watched; 2 To watch,tvmaze,42"));
        assert!(lines[2].starts_with("Book,Piranesi,,Read,20"));
        assert!(lines[2].ends_with(",,Autumn,,,"));
    }

    #[test]
    fn csv_cells_are_quoted_and_never_run_as_formulas() {
        assert_eq!(cell("plain"), "plain");
        assert_eq!(cell("a,b"), "\"a,b\"");
        assert_eq!(cell("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(cell("two\nlines"), "\"two\nlines\"");
        assert_eq!(cell("=1+1"), "'=1+1");
        assert_eq!(cell("@SUM(A1,A2)"), "\"'@SUM(A1,A2)\"");
        assert_eq!(cell("+1"), "'+1");
        assert_eq!(cell("-30-"), "'-30-");
        assert_eq!(cell(""), "");
    }

    #[test]
    fn an_empty_library_exports_too() {
        let database = Database::open_in_memory().unwrap();

        assert_eq!(csv(&database).unwrap().lines().count(), 1);
        let document: Value = serde_json::from_str(&json(&database).unwrap()).unwrap();
        assert_eq!(document["items"], Value::Array(Vec::new()));
    }

    #[test]
    fn a_backup_opens_as_a_library_with_everything_in_it() {
        let (folder, path) = library();
        let target = folder.path().join("copy.db");

        let saved = save(&path, Format::Backup, &target).unwrap();

        assert_eq!(saved.items, 2);
        assert_eq!(saved.path, target);
        let original = Database::open(&path).unwrap();
        let copy = Database::open(&target).unwrap();
        assert_eq!(items::all(&copy).unwrap(), items::all(&original).unwrap());
        assert_eq!(
            list_items::memberships(&copy).unwrap(),
            list_items::memberships(&original).unwrap()
        );
        assert!(items::all(&copy).unwrap()[0].details().is_some());
    }

    #[test]
    fn every_format_saves_replaces_and_leaves_no_partial_file() {
        let (folder, path) = library();
        for format in Format::ALL {
            let target = folder.path().join(format.file_name("2026-11-25"));
            std::fs::write(&target, "older").unwrap();
            std::fs::write(partial_path(&target), "left over").unwrap();

            save(&path, format, &target).unwrap();

            assert_ne!(std::fs::read(&target).unwrap(), b"older");
            assert!(!partial_path(&target).exists());
        }
    }

    #[test]
    fn a_missing_extension_is_added() {
        let (folder, path) = library();
        for format in Format::ALL {
            let saved = save(&path, format, &folder.path().join("mine")).unwrap();

            assert!(saved.path.to_string_lossy().contains("mine."));
            assert!(saved.path.exists());
        }
    }

    #[test]
    fn adding_an_extension_never_replaces_a_file_nobody_agreed_to_replace() {
        let (folder, path) = library();
        for format in Format::ALL {
            let chosen = folder.path().join("mine");
            let existing = chosen.with_extension(format.extension());
            std::fs::write(&existing, "precious").unwrap();

            let first = save(&path, format, &chosen).unwrap();
            let second = save(&path, format, &chosen).unwrap();

            assert_eq!(std::fs::read(&existing).unwrap(), b"precious");
            assert!(first.path.to_string_lossy().contains("mine (2)."));
            assert!(second.path.to_string_lossy().contains("mine (3)."));
        }
    }

    #[test]
    fn the_library_in_use_is_never_replaced() {
        let (folder, path) = library();
        let before = std::fs::read(&path).unwrap();
        let roundabout = folder.path().join(".").join("loregalia.db");

        for format in Format::ALL {
            for target in [&path, &roundabout] {
                assert!(matches!(
                    save(&path, format, target),
                    Err(Error::WouldReplaceLibrary)
                ));
            }
        }
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn a_failed_save_leaves_the_target_as_it_was() {
        let (folder, path) = library();
        for format in Format::ALL {
            let target = folder.path().join(format!("{}.out", format.key()));
            std::fs::create_dir_all(&target).unwrap();
            std::fs::write(target.join("kept.txt"), "kept").unwrap();

            let error = save(&path, format, &target).unwrap_err();

            assert!(failure_message(&error, &target).starts_with("Could not save"));
            assert_eq!(std::fs::read(target.join("kept.txt")).unwrap(), b"kept");
            assert!(!partial_path(&target).exists());
        }
    }

    #[test]
    fn a_missing_folder_is_a_failure() {
        let (folder, path) = library();
        for format in Format::ALL {
            let target = folder.path().join("gone").join("file.out");

            assert!(save(&path, format, &target).is_err());
            assert!(!target.exists());
        }
    }

    #[test]
    fn failures_say_what_to_do() {
        let path = Path::new("x.json");
        let io = |kind| Error::Io(std::io::Error::from(kind));

        assert!(failure_message(&Error::WouldReplaceLibrary, path).contains("Choose another name"));
        assert!(
            failure_message(&io(std::io::ErrorKind::PermissionDenied), path)
                .contains("not allowed")
        );
        assert!(failure_message(&io(std::io::ErrorKind::StorageFull), path).contains("full"));
    }

    #[test]
    fn file_names_carry_the_date() {
        let names = Format::ALL.map(|format| format.file_name("2026-11-25"));

        assert_eq!(
            names,
            [
                "loregalia-backup-2026-11-25.db",
                "loregalia-2026-11-25.json",
                "loregalia-2026-11-25.csv"
            ]
        );
    }
}
