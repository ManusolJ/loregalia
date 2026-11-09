use crate::providers::Source;
use crate::{Database, Error, Result, media::MediaKind};
use rusqlite::{OptionalExtension, Row, params, types::Type};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceData {
    pub title: String,
    pub release_year: Option<i32>,
    pub cover_url: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Item {
    pub id: String,
    pub media_kind: MediaKind,
    pub title: String,
    pub release_year: Option<i32>,
    pub release_date: Option<String>,
    pub finished_at: Option<String>,
    pub status_id: String,
    pub source: Option<String>,
    pub external_id: Option<String>,
    pub cover_url: Option<String>,
    pub source_data: Option<SourceData>,
    pub source_refreshed_at: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lookup {
    pub source: Source,
    pub external_id: String,
    pub cover_url: Option<String>,
}

impl Item {
    pub(crate) fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            title: row.get(1)?,
            media_kind: media_kind(row, 2)?,
            release_year: row.get(3)?,
            release_date: row.get(4)?,
            finished_at: row.get(5)?,
            status_id: row.get(6)?,
            source: row.get(7)?,
            external_id: row.get(8)?,
            cover_url: row.get(9)?,
            source_data: source_data(row, 10)?,
            source_refreshed_at: row.get(11)?,
        })
    }
}

fn source_data(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<SourceData>> {
    let value: Option<String> = row.get(index)?;

    value
        .map(|json| serde_json::from_str(&json))
        .transpose()
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
        })
}

fn media_kind(row: &Row<'_>, index: usize) -> rusqlite::Result<MediaKind> {
    let value: String = row.get(index)?;
    MediaKind::from_key(&value)
        .ok_or_else(|| rusqlite::Error::InvalidColumnType(index, value, Type::Text))
}

pub fn create(
    database: &Database,
    media_kind: MediaKind,
    title: &str,
    release_year: Option<i32>,
) -> Result<Item> {
    let id = Uuid::now_v7().to_string();

    database
        .connection()
        .query_row(
            "INSERT INTO items (id, media_kind, title, release_year, status_id, created_at)
             VALUES (?1, ?2, ?3, ?4,
                     (SELECT id FROM statuses
                      WHERE media_kind = ?2 AND category = 'planned'),
                     strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             RETURNING id, title, media_kind, release_year, release_date, finished_at, status_id, source, external_id, cover_url, source_data, source_refreshed_at",
            params![id, media_kind.key(), title, release_year],
            Item::from_row,
        )
        .map_err(Error::from)
}

pub fn create_from_lookup(
    database: &Database,
    media_kind: MediaKind,
    title: &str,
    release_year: Option<i32>,
    lookup: &Lookup,
) -> Result<Item> {
    let id = Uuid::now_v7().to_string();
    let source_data = SourceData {
        title: title.to_owned(),
        release_year,
        cover_url: lookup.cover_url.clone(),
    };
    let source_data_json = serde_json::to_string(&source_data)?;
    let refreshed_at = crate::log::timestamp(SystemTime::now());

    database
        .connection()
        .query_row(
            "INSERT INTO items
                 (id, media_kind, title, release_year, status_id, source, external_id, cover_url, source_data, source_refreshed_at, created_at)
             VALUES (?1, ?2, ?3, ?4,
                     (SELECT id FROM statuses
                      WHERE media_kind = ?2 AND category = 'planned'),
                     ?5, ?6, ?7, ?8, ?9,
                     strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             RETURNING id, title, media_kind, release_year, release_date, finished_at, status_id, source, external_id, cover_url, source_data, source_refreshed_at",
            params![
                id,
                media_kind.key(),
                title,
                release_year,
                lookup.source.key(),
                lookup.external_id.as_str(),
                lookup.cover_url.as_deref(),
                source_data_json,
                refreshed_at
            ],
            Item::from_row,
        )
        .map_err(|error| match error {
            rusqlite::Error::SqliteFailure(failure, _)
                if failure.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                Error::DuplicateLookup
            }
            error => Error::Sqlite(error),
        })
}

pub fn duplicate<'a>(items: &'a [Item], media_kind: MediaKind, title: &str) -> Option<&'a Item> {
    let title = title.trim().to_lowercase();
    if title.is_empty() {
        return None;
    }

    items
        .iter()
        .find(|item| item.media_kind == media_kind && item.title.trim().to_lowercase() == title)
}

pub fn all(database: &Database) -> Result<Vec<Item>> {
    let mut statement = database.connection().prepare(
        "SELECT id, title, media_kind, release_year, release_date, finished_at, status_id, source, external_id, cover_url, source_data, source_refreshed_at
         FROM items ORDER BY created_at, rowid",
    )?;
    let rows = statement.query_map([], Item::from_row)?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Error::from)
}

pub fn get(database: &Database, id: &str) -> Result<Option<Item>> {
    database
        .connection()
        .query_row(
            "SELECT id, title, media_kind, release_year, release_date, finished_at, status_id, source, external_id, cover_url, source_data, source_refreshed_at
             FROM items WHERE id = ?1",
            [id],
            Item::from_row,
        )
        .optional()
        .map_err(Error::from)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Refresh {
    pub title: String,
    pub release_year: Option<i32>,
    pub cover_url: Option<String>,
    pub source_data: SourceData,
}

pub fn refreshed(item: &Item, latest: SourceData) -> Refresh {
    let previous = item.source_data.as_ref();

    Refresh {
        title: if previous.is_some_and(|old| old.title == item.title) {
            latest.title.clone()
        } else {
            item.title.clone()
        },
        release_year: if previous.is_some_and(|old| old.release_year == item.release_year) {
            latest.release_year
        } else {
            item.release_year
        },
        cover_url: if previous.is_some_and(|old| old.cover_url == item.cover_url) {
            latest.cover_url.clone()
        } else {
            item.cover_url.clone()
        },
        source_data: latest,
    }
}

pub fn apply_refresh(
    database: &Database,
    id: &str,
    latest: SourceData,
    refreshed_at: &str,
) -> Result<bool> {
    let Some(item) = get(database, id)? else {
        return Ok(false);
    };
    if item.source.is_none() {
        return Ok(false);
    }

    let refresh = refreshed(&item, latest);
    let source_data = serde_json::to_string(&refresh.source_data)?;
    let changed = database.connection().execute(
        "UPDATE items
         SET title = ?1,
             release_year = ?2,
             cover_url = ?3,
             source_data = ?4,
             source_refreshed_at = ?5
         WHERE id = ?6",
        params![
            refresh.title,
            refresh.release_year,
            refresh.cover_url,
            source_data,
            refreshed_at,
            id
        ],
    )?;

    Ok(changed > 0)
}

pub fn due_for_refresh<'a>(items: &'a [Item], cutoff: &str) -> Vec<&'a Item> {
    items
        .iter()
        .filter(|item| {
            item.media_kind == MediaKind::Movie
                && item.source.as_deref() == Some(Source::Tmdb.key())
                && item
                    .source_refreshed_at
                    .as_deref()
                    .is_none_or(|refreshed_at| refreshed_at < cutoff)
        })
        .collect()
}

pub fn update(
    database: &Database,
    id: &str,
    media_kind: MediaKind,
    title: &str,
    release_year: Option<i32>,
) -> Result<bool> {
    let changed = database.connection().execute(
        "UPDATE items
         SET media_kind = ?1,
             title = ?2,
             release_year = ?3,
             status_id = CASE
                 WHEN media_kind = ?1 THEN status_id
                 ELSE (SELECT id FROM statuses
                       WHERE media_kind = ?1 AND category = 'planned')
             END,
             finished_at = CASE
                 WHEN media_kind = ?1 THEN finished_at
                 ELSE NULL
             END
         WHERE id = ?4",
        params![media_kind.key(), title, release_year, id],
    )?;

    Ok(changed > 0)
}

pub fn set_status(database: &Database, id: &str, status_id: &str) -> Result<bool> {
    let status_kind: Option<String> = database
        .connection()
        .query_row(
            "SELECT media_kind FROM statuses WHERE id = ?1",
            [status_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(status_kind) = status_kind else {
        return Err(Error::UnknownStatus);
    };

    let item_kind: Option<String> = database
        .connection()
        .query_row("SELECT media_kind FROM items WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .optional()?;
    let Some(item_kind) = item_kind else {
        return Ok(false);
    };

    if status_kind != item_kind {
        return Err(Error::StatusKindMismatch);
    }

    let changed = database.connection().execute(
        "UPDATE items
         SET status_id = ?1,
             finished_at = CASE
                 WHEN (SELECT category FROM statuses WHERE id = ?1) = 'done'
                 THEN COALESCE(finished_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                 ELSE NULL
             END
         WHERE id = ?2",
        params![status_id, id],
    )?;

    Ok(changed > 0)
}

pub fn delete(database: &Database, id: &str) -> Result<bool> {
    let changed = database
        .connection()
        .execute("DELETE FROM items WHERE id = ?1", [id])?;

    Ok(changed > 0)
}

#[cfg(test)]
mod tests {
    use super::{
        Item, Lookup, SourceData, all, apply_refresh, create, create_from_lookup, delete,
        due_for_refresh, duplicate, get, refreshed, set_status, update,
    };
    use crate::Database;
    use crate::Error;
    use crate::media::MediaKind;

    fn database() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn status_of(database: &Database, id: &str) -> Item {
        all(database)
            .unwrap()
            .into_iter()
            .find(|item| item.id == id)
            .unwrap()
    }

    #[test]
    fn a_created_item_starts_in_the_planned_status_for_its_kind() {
        let database = database();

        let movie = create(&database, MediaKind::Movie, "Dune", None).unwrap();
        let show = create(&database, MediaKind::Show, "Dark", None).unwrap();
        let book = create(&database, MediaKind::Book, "Piranesi", None).unwrap();
        let game = create(&database, MediaKind::Game, "Outer Wilds", None).unwrap();

        assert_eq!(movie.status_id, "movie-planned");
        assert_eq!(show.status_id, "show-planned");
        assert_eq!(book.status_id, "book-planned");
        assert_eq!(game.status_id, "game-planned");
    }

    #[test]
    fn moving_to_done_records_when_it_was_finished() {
        let database = database();
        let item = create(&database, MediaKind::Movie, "Dune", None).unwrap();

        assert!(set_status(&database, &item.id, "movie-done").unwrap());

        let stored = status_of(&database, &item.id);
        assert_eq!(stored.status_id, "movie-done");
        let finished_at = stored.finished_at.unwrap();
        assert!(finished_at.ends_with('Z'));

        assert!(set_status(&database, &item.id, "movie-active").unwrap());
        assert_eq!(status_of(&database, &item.id).finished_at, None);
    }

    #[test]
    fn setting_done_again_keeps_the_first_finish_time() {
        let database = database();
        let item = create(&database, MediaKind::Book, "Piranesi", None).unwrap();
        set_status(&database, &item.id, "book-done").unwrap();
        let first = status_of(&database, &item.id).finished_at.unwrap();

        set_status(&database, &item.id, "book-done").unwrap();

        assert_eq!(status_of(&database, &item.id).finished_at, Some(first));
    }

    #[test]
    fn a_status_for_another_media_kind_is_rejected() {
        let database = database();
        let item = create(&database, MediaKind::Movie, "Dune", None).unwrap();

        let error = set_status(&database, &item.id, "book-done").unwrap_err();

        assert!(matches!(error, Error::StatusKindMismatch));
        let stored = status_of(&database, &item.id);
        assert_eq!(stored.status_id, "movie-planned");
        assert_eq!(stored.finished_at, None);
    }

    #[test]
    fn an_unknown_status_is_an_error() {
        let database = database();
        let item = create(&database, MediaKind::Game, "Outer Wilds", None).unwrap();

        assert!(matches!(
            set_status(&database, &item.id, "missing"),
            Err(Error::UnknownStatus)
        ));
    }

    #[test]
    fn setting_a_status_on_a_missing_item_reports_no_change() {
        let database = database();
        create(&database, MediaKind::Movie, "Dune", None).unwrap();

        assert!(!set_status(&database, "missing", "movie-done").unwrap());
    }

    #[test]
    fn changing_the_media_kind_resets_the_status_and_finish_time() {
        let database = database();
        let item = create(&database, MediaKind::Movie, "Dune", None).unwrap();
        set_status(&database, &item.id, "movie-done").unwrap();

        assert!(update(&database, &item.id, MediaKind::Book, "Dune", None).unwrap());

        let stored = status_of(&database, &item.id);
        assert_eq!(stored.status_id, "book-planned");
        assert_eq!(stored.finished_at, None);
    }

    #[test]
    fn editing_a_title_keeps_the_status_and_finish_time() {
        let database = database();
        let item = create(&database, MediaKind::Movie, "Dune", Some(1984)).unwrap();
        set_status(&database, &item.id, "movie-done").unwrap();
        let finished_at = status_of(&database, &item.id).finished_at;

        assert!(
            update(
                &database,
                &item.id,
                MediaKind::Movie,
                "Dune Messiah",
                Some(1984)
            )
            .unwrap()
        );

        let stored = status_of(&database, &item.id);
        assert_eq!(stored.status_id, "movie-done");
        assert_eq!(stored.finished_at, finished_at);
    }

    #[test]
    fn a_created_item_round_trips() {
        let database = database();

        let created = create(&database, MediaKind::Book, "Piranesi", Some(2020)).unwrap();
        let stored = all(&database).unwrap();

        assert_eq!(stored, vec![created]);
        assert_eq!(stored[0].media_kind, MediaKind::Book);
        assert_eq!(stored[0].title, "Piranesi");
        assert_eq!(stored[0].release_year, Some(2020));
        assert_eq!(stored[0].release_date, None);
        assert_eq!(stored[0].finished_at, None);
        assert_eq!(stored[0].source, None);
        assert_eq!(stored[0].external_id, None);
        assert_eq!(stored[0].cover_url, None);
        assert_eq!(stored[0].source_data, None);
        assert_eq!(stored[0].source_refreshed_at, None);
    }

    #[test]
    fn a_looked_up_item_keeps_its_source_and_cover() {
        let database = database();
        let lookup = Lookup {
            source: crate::providers::Source::TvMaze,
            external_id: "39235".to_owned(),
            cover_url: Some("https://example.test/poster.jpg".to_owned()),
        };

        let created = create_from_lookup(
            &database,
            MediaKind::Show,
            "A Confession",
            Some(2019),
            &lookup,
        )
        .unwrap();

        assert_eq!(created.source.as_deref(), Some("tvmaze"));
        assert_eq!(created.external_id.as_deref(), Some("39235"));
        assert_eq!(
            created.cover_url.as_deref(),
            Some("https://example.test/poster.jpg")
        );
        assert_eq!(
            created.source_data,
            Some(SourceData {
                title: "A Confession".to_owned(),
                release_year: Some(2019),
                cover_url: Some("https://example.test/poster.jpg".to_owned()),
            })
        );
        assert!(
            created
                .source_refreshed_at
                .as_deref()
                .is_some_and(|value| value.ends_with('Z'))
        );
        assert_eq!(all(&database).unwrap(), vec![created]);
    }

    #[test]
    fn a_refresh_updates_untouched_visible_fields() {
        let database = database();
        let lookup = Lookup {
            source: crate::providers::Source::Tmdb,
            external_id: "438631".to_owned(),
            cover_url: Some("https://image.example/old.jpg".to_owned()),
        };
        let created =
            create_from_lookup(&database, MediaKind::Movie, "Dune", Some(2021), &lookup).unwrap();
        let latest = SourceData {
            title: "Dune: Part One".to_owned(),
            release_year: Some(2021),
            cover_url: Some("https://image.example/new.jpg".to_owned()),
        };

        assert!(
            apply_refresh(
                &database,
                &created.id,
                latest.clone(),
                "2026-11-09T00:00:00.000Z"
            )
            .unwrap()
        );

        let stored = get(&database, &created.id).unwrap().unwrap();
        assert_eq!(stored.title, "Dune: Part One");
        assert_eq!(stored.release_year, Some(2021));
        assert_eq!(
            stored.cover_url.as_deref(),
            Some("https://image.example/new.jpg")
        );
        assert_eq!(stored.source_data, Some(latest));
        assert_eq!(
            stored.source_refreshed_at.as_deref(),
            Some("2026-11-09T00:00:00.000Z")
        );
    }

    #[test]
    fn a_refresh_keeps_a_visible_field_the_user_edited() {
        let database = database();
        let lookup = Lookup {
            source: crate::providers::Source::Tmdb,
            external_id: "438631".to_owned(),
            cover_url: Some("https://image.example/old.jpg".to_owned()),
        };
        let created =
            create_from_lookup(&database, MediaKind::Movie, "Dune", Some(2021), &lookup).unwrap();
        update(
            &database,
            &created.id,
            MediaKind::Movie,
            "My Dune",
            Some(2021),
        )
        .unwrap();
        let latest = SourceData {
            title: "Dune: Part One".to_owned(),
            release_year: Some(2021),
            cover_url: Some("https://image.example/new.jpg".to_owned()),
        };

        apply_refresh(
            &database,
            &created.id,
            latest.clone(),
            "2026-11-09T00:00:00.000Z",
        )
        .unwrap();

        let stored = get(&database, &created.id).unwrap().unwrap();
        assert_eq!(stored.title, "My Dune");
        assert_eq!(
            stored.cover_url.as_deref(),
            Some("https://image.example/new.jpg")
        );
        assert_eq!(stored.source_data, Some(latest));
    }

    #[test]
    fn a_refresh_without_previous_source_data_keeps_visible_fields() {
        let database = database();
        let created = create(&database, MediaKind::Movie, "Dune", Some(2021)).unwrap();
        let latest = SourceData {
            title: "Dune: Part One".to_owned(),
            release_year: Some(2024),
            cover_url: Some("https://image.example/new.jpg".to_owned()),
        };

        assert!(
            !apply_refresh(&database, &created.id, latest, "2026-11-09T00:00:00.000Z").unwrap()
        );
        assert_eq!(get(&database, &created.id).unwrap().unwrap(), created);
    }

    #[test]
    fn due_for_refresh_lists_old_tmdb_items_only() {
        let database = database();
        let tmdb = create_from_lookup(
            &database,
            MediaKind::Movie,
            "Dune",
            Some(2021),
            &Lookup {
                source: crate::providers::Source::Tmdb,
                external_id: "438631".to_owned(),
                cover_url: None,
            },
        )
        .unwrap();
        let fresh = create_from_lookup(
            &database,
            MediaKind::Movie,
            "Dune: Part Two",
            Some(2024),
            &Lookup {
                source: crate::providers::Source::Tmdb,
                external_id: "693134".to_owned(),
                cover_url: None,
            },
        )
        .unwrap();
        let tvmaze = create_from_lookup(
            &database,
            MediaKind::Show,
            "Dark",
            Some(2017),
            &Lookup {
                source: crate::providers::Source::TvMaze,
                external_id: "42".to_owned(),
                cover_url: None,
            },
        )
        .unwrap();
        let stale = Item {
            source_refreshed_at: Some("2026-01-01T00:00:00.000Z".to_owned()),
            ..tmdb
        };
        let fresh = Item {
            source_refreshed_at: Some("2026-10-01T00:00:00.000Z".to_owned()),
            ..fresh
        };

        let items = [stale, fresh, tvmaze];
        let due = due_for_refresh(&items, "2026-06-01T00:00:00.000Z");

        assert_eq!(due.len(), 1);
        assert_eq!(due[0].external_id.as_deref(), Some("438631"));
    }

    #[test]
    fn refreshed_keeps_visible_fields_when_there_is_no_previous_source_data() {
        let database = database();
        let item = create(&database, MediaKind::Movie, "Dune", Some(2021)).unwrap();
        let latest = SourceData {
            title: "Dune: Part One".to_owned(),
            release_year: Some(2024),
            cover_url: Some("https://image.example/new.jpg".to_owned()),
        };

        let result = refreshed(&item, latest.clone());

        assert_eq!(result.title, "Dune");
        assert_eq!(result.release_year, Some(2021));
        assert_eq!(result.cover_url, None);
        assert_eq!(result.source_data, latest);
    }

    #[test]
    fn the_same_lookup_cannot_be_saved_twice() {
        let database = database();
        let lookup = Lookup {
            source: crate::providers::Source::TvMaze,
            external_id: "39235".to_owned(),
            cover_url: None,
        };
        create_from_lookup(
            &database,
            MediaKind::Show,
            "A Confession",
            Some(2019),
            &lookup,
        )
        .unwrap();

        let error = create_from_lookup(
            &database,
            MediaKind::Show,
            "A Confession",
            Some(2019),
            &lookup,
        )
        .unwrap_err();

        assert!(matches!(error, Error::DuplicateLookup));
        assert_eq!(all(&database).unwrap().len(), 1);
    }

    #[test]
    fn hand_typed_duplicates_are_allowed() {
        let database = database();

        create(&database, MediaKind::Movie, "Dune", Some(1984)).unwrap();
        create(&database, MediaKind::Movie, "Dune", Some(2021)).unwrap();

        assert_eq!(all(&database).unwrap().len(), 2);
    }

    #[test]
    fn a_duplicate_is_found_by_title_and_kind_ignoring_case() {
        let database = database();
        let existing = create(&database, MediaKind::Movie, "Dune", Some(1984)).unwrap();
        let stored = all(&database).unwrap();

        assert_eq!(
            duplicate(&stored, MediaKind::Movie, "  dUnE  "),
            Some(&existing)
        );
        assert_eq!(duplicate(&stored, MediaKind::Book, "Dune"), None);
        assert_eq!(duplicate(&stored, MediaKind::Movie, "Dune Messiah"), None);
        assert_eq!(duplicate(&stored, MediaKind::Movie, "   "), None);
    }

    #[test]
    fn a_looked_up_item_can_match_a_hand_typed_title() {
        let database = database();
        let lookup = Lookup {
            source: crate::providers::Source::TvMaze,
            external_id: "42".to_owned(),
            cover_url: None,
        };
        let existing =
            create_from_lookup(&database, MediaKind::Show, "Dark", Some(2017), &lookup).unwrap();
        let stored = all(&database).unwrap();

        assert_eq!(duplicate(&stored, MediaKind::Show, "dark"), Some(&existing));
    }

    #[test]
    fn items_keep_their_insertion_order() {
        let database = database();

        create(&database, MediaKind::Movie, "First", None).unwrap();
        create(&database, MediaKind::Game, "Second", None).unwrap();

        let titles: Vec<String> = all(&database)
            .unwrap()
            .into_iter()
            .map(|item| item.title)
            .collect();
        assert_eq!(titles, ["First", "Second"]);
    }

    #[test]
    fn update_changes_the_item_and_keeps_its_identity() {
        let database = database();
        let item = create(&database, MediaKind::Movie, "Dune", Some(1984)).unwrap();

        assert!(
            update(
                &database,
                &item.id,
                MediaKind::Book,
                "Dune Messiah",
                Some(1969)
            )
            .unwrap()
        );

        let stored = all(&database).unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].id, item.id);
        assert_eq!(stored[0].media_kind, MediaKind::Book);
        assert_eq!(stored[0].title, "Dune Messiah");
        assert_eq!(stored[0].release_year, Some(1969));
    }

    #[test]
    fn updating_a_missing_item_reports_no_change() {
        let database = database();

        assert!(!update(&database, "missing", MediaKind::Movie, "Dune", None).unwrap());
        assert_eq!(all(&database).unwrap(), Vec::<Item>::new());
    }

    #[test]
    fn delete_removes_only_the_named_item() {
        let database = database();
        let keep = create(&database, MediaKind::Movie, "Keep", None).unwrap();
        let remove = create(&database, MediaKind::Book, "Remove", None).unwrap();

        assert!(delete(&database, &remove.id).unwrap());

        assert_eq!(all(&database).unwrap(), vec![keep]);
    }

    #[test]
    fn deleting_a_missing_item_reports_no_change() {
        let database = database();
        let item = create(&database, MediaKind::Game, "Keep", None).unwrap();

        assert!(!delete(&database, "missing").unwrap());
        assert_eq!(all(&database).unwrap(), vec![item]);
    }
}
