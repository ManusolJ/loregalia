use crate::details::Details;
use crate::providers::{Candidate, Source};
use crate::storage::keyed_column;
use crate::{Database, Error, Result, media::MediaKind};
use rusqlite::{OptionalExtension, Row, params, types::Type};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::SystemTime;
use uuid::Uuid;

pub(crate) const COLUMNS: &str = "items.id, items.title, items.media_kind, items.release_year, items.release_date, items.finished_at, items.status_id, items.source, items.external_id, items.cover_url, items.source_data, items.source_refreshed_at, date(items.finished_at, 'localtime')";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceData {
    pub title: String,
    pub release_year: Option<i32>,
    pub cover_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<Details>,
}

impl From<Candidate> for SourceData {
    fn from(candidate: Candidate) -> Self {
        Self {
            title: candidate.title,
            release_year: candidate.release_year,
            cover_url: candidate.cover_url,
            details: candidate.details,
        }
    }
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
    pub finished_on: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lookup {
    pub source: Source,
    pub external_id: String,
    pub cover_url: Option<String>,
}

impl Item {
    pub fn details(&self) -> Option<&Details> {
        self.source_data.as_ref()?.details.as_ref()
    }

    pub(crate) fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            title: row.get(1)?,
            media_kind: keyed_column(row, 2, MediaKind::from_key)?,
            release_year: row.get(3)?,
            release_date: row.get(4)?,
            finished_at: row.get(5)?,
            status_id: row.get(6)?,
            source: row.get(7)?,
            external_id: row.get(8)?,
            cover_url: row.get(9)?,
            source_data: source_data(row, 10)?,
            source_refreshed_at: row.get(11)?,
            finished_on: row.get(12)?,
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
            &format!(
                "INSERT INTO items (id, media_kind, title, release_year, status_id, created_at)
                 VALUES (?1, ?2, ?3, ?4,
                         (SELECT id FROM statuses
                          WHERE media_kind = ?2 AND category = 'planned'),
                         strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                 RETURNING {COLUMNS}"
            ),
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
        details: None,
    };
    let source_data_json = serde_json::to_string(&source_data)?;
    let refreshed_at = crate::log::timestamp(SystemTime::now());

    database
        .connection()
        .query_row(
            &format!(
                "INSERT INTO items
                     (id, media_kind, title, release_year, status_id, source, external_id, cover_url, source_data, source_refreshed_at, created_at)
                 VALUES (?1, ?2, ?3, ?4,
                         (SELECT id FROM statuses
                          WHERE media_kind = ?2 AND category = 'planned'),
                         ?5, ?6, ?7, ?8, ?9,
                         strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                 RETURNING {COLUMNS}"
            ),
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
    database.select(
        &format!("SELECT {COLUMNS} FROM items ORDER BY created_at, rowid"),
        [],
        Item::from_row,
    )
}

pub fn get(database: &Database, id: &str) -> Result<Option<Item>> {
    database
        .connection()
        .query_row(
            &format!("SELECT {COLUMNS} FROM items WHERE id = ?1"),
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

pub fn refreshed(item: &Item, mut latest: SourceData) -> Refresh {
    let previous = item.source_data.as_ref();
    if latest.details.is_none() {
        latest.details = item.details().cloned();
    }

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
    let release_date = refresh
        .source_data
        .details
        .as_ref()
        .and_then(|details| details.release_date.clone());
    let changed = database.connection().execute(
        "UPDATE items
         SET title = ?1,
             release_year = ?2,
             cover_url = ?3,
             source_data = ?4,
             source_refreshed_at = ?5,
             release_date = COALESCE(?7, release_date)
         WHERE id = ?6",
        params![
            refresh.title,
            refresh.release_year,
            refresh.cover_url,
            source_data,
            refreshed_at,
            id,
            release_date
        ],
    )?;

    Ok(changed > 0)
}

pub fn mark_without_details(database: &Database, id: &str) -> Result<bool> {
    let Some(item) = get(database, id)? else {
        return Ok(false);
    };
    let data = SourceData {
        details: Some(Details::default()),
        ..item.source_data.unwrap_or(SourceData {
            title: item.title,
            release_year: item.release_year,
            cover_url: item.cover_url,
            details: None,
        })
    };
    let changed = database.connection().execute(
        "UPDATE items SET source_data = ?1 WHERE id = ?2",
        params![serde_json::to_string(&data)?, id],
    )?;

    Ok(changed > 0)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Due {
    pub id: String,
    pub source: Source,
    pub external_id: String,
}

pub fn next_due(items: &[Item], cutoff: &str, skip: &HashSet<String>) -> Option<Due> {
    let missing = items.iter().rev().filter(|item| item.details().is_none());

    missing
        .chain(due_for_refresh(items, cutoff))
        .filter(|item| !skip.contains(&item.id))
        .find_map(|item| {
            Some(Due {
                id: item.id.clone(),
                source: Source::from_key(item.source.as_deref()?)?,
                external_id: item.external_id.clone()?,
            })
        })
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
    use crate::providers::Source;

    fn database() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn stored_item(database: &Database, id: &str) -> Item {
        get(database, id).unwrap().unwrap()
    }

    #[test]
    fn a_candidate_becomes_source_data() {
        let candidate = crate::providers::Candidate {
            source: crate::providers::Source::Tmdb,
            external_id: "438631".to_owned(),
            title: "Dune".to_owned(),
            release_year: Some(2021),
            cover_url: Some("https://image.example/dune.jpg".to_owned()),
            details: None,
            edition: None,
        };

        assert_eq!(
            SourceData::from(candidate),
            SourceData {
                title: "Dune".to_owned(),
                release_year: Some(2021),
                cover_url: Some("https://image.example/dune.jpg".to_owned()),
                details: None,
            }
        );
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

        let stored = stored_item(&database, &item.id);
        assert_eq!(stored.status_id, "movie-done");
        let finished_at = stored.finished_at.unwrap();
        assert!(finished_at.ends_with('Z'));

        assert!(set_status(&database, &item.id, "movie-active").unwrap());
        assert_eq!(stored_item(&database, &item.id).finished_at, None);
    }

    #[test]
    fn setting_done_again_keeps_the_first_finish_time() {
        let database = database();
        let item = create(&database, MediaKind::Book, "Piranesi", None).unwrap();
        set_status(&database, &item.id, "book-done").unwrap();
        let first = stored_item(&database, &item.id).finished_at.unwrap();

        set_status(&database, &item.id, "book-done").unwrap();

        assert_eq!(stored_item(&database, &item.id).finished_at, Some(first));
    }

    #[test]
    fn a_status_for_another_media_kind_is_rejected() {
        let database = database();
        let item = create(&database, MediaKind::Movie, "Dune", None).unwrap();

        let error = set_status(&database, &item.id, "book-done").unwrap_err();

        assert!(matches!(error, Error::StatusKindMismatch));
        let stored = stored_item(&database, &item.id);
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

        let stored = stored_item(&database, &item.id);
        assert_eq!(stored.status_id, "book-planned");
        assert_eq!(stored.finished_at, None);
    }

    #[test]
    fn editing_a_title_keeps_the_status_and_finish_time() {
        let database = database();
        let item = create(&database, MediaKind::Movie, "Dune", Some(1984)).unwrap();
        set_status(&database, &item.id, "movie-done").unwrap();
        let finished_at = stored_item(&database, &item.id).finished_at;

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

        let stored = stored_item(&database, &item.id);
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
                details: None,
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
            details: None,
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
            details: None,
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
            details: None,
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
            details: None,
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

    #[test]
    fn a_finished_item_knows_its_local_finish_day() {
        let database = Database::open_in_memory().unwrap();
        let item = create(&database, MediaKind::Book, "Piranesi", None).unwrap();
        assert_eq!(item.finished_on, None);

        database
            .connection()
            .execute(
                "UPDATE items SET finished_at = '2026-11-24T12:00:00.000Z' WHERE id = ?1",
                [&item.id],
            )
            .unwrap();

        let stored = super::get(&database, &item.id).unwrap().unwrap();
        assert_eq!(stored.finished_on.as_deref(), Some("2026-11-24"));
    }

    fn looked_up(database: &Database, kind: MediaKind, source: Source, id: &str) -> Item {
        create_from_lookup(
            database,
            kind,
            "Title",
            Some(2020),
            &Lookup {
                source,
                external_id: id.to_owned(),
                cover_url: None,
            },
        )
        .unwrap()
    }

    fn with_details(minutes: u32) -> SourceData {
        SourceData {
            title: "Title".to_owned(),
            release_year: Some(2020),
            cover_url: None,
            details: Some(crate::details::Details {
                minutes: Some(minutes),
                release_date: Some("2020-05-01".to_owned()),
                ..Default::default()
            }),
        }
    }

    #[test]
    fn refreshing_stores_details_and_the_full_release_date() {
        let database = Database::open_in_memory().unwrap();
        let item = looked_up(&database, MediaKind::Show, Source::TvMaze, "1");
        assert_eq!(item.details(), None);

        apply_refresh(
            &database,
            &item.id,
            with_details(49),
            "2026-11-24T00:00:00.000Z",
        )
        .unwrap();

        let stored = get(&database, &item.id).unwrap().unwrap();
        assert_eq!(stored.details().unwrap().minutes, Some(49));
        assert_eq!(stored.release_date.as_deref(), Some("2020-05-01"));
    }

    #[test]
    fn a_refresh_without_details_keeps_the_ones_already_stored() {
        let database = Database::open_in_memory().unwrap();
        let item = looked_up(&database, MediaKind::Movie, Source::Tmdb, "1");
        apply_refresh(
            &database,
            &item.id,
            with_details(90),
            "2026-11-24T00:00:00.000Z",
        )
        .unwrap();
        let older = SourceData {
            details: None,
            ..with_details(90)
        };

        apply_refresh(&database, &item.id, older, "2026-11-25T00:00:00.000Z").unwrap();

        let stored = get(&database, &item.id).unwrap().unwrap();
        assert_eq!(stored.details().unwrap().minutes, Some(90));
    }

    #[test]
    fn details_never_replace_what_the_user_typed() {
        let database = Database::open_in_memory().unwrap();
        let item = looked_up(&database, MediaKind::Movie, Source::Tmdb, "1");
        update(
            &database,
            &item.id,
            MediaKind::Movie,
            "My title",
            Some(1999),
        )
        .unwrap();

        apply_refresh(
            &database,
            &item.id,
            with_details(90),
            "2026-11-24T00:00:00.000Z",
        )
        .unwrap();

        let stored = get(&database, &item.id).unwrap().unwrap();
        assert_eq!(stored.title, "My title");
        assert_eq!(stored.release_year, Some(1999));
        assert_eq!(stored.details().unwrap().minutes, Some(90));
    }

    #[test]
    fn the_newest_item_without_details_is_due_first() {
        let database = Database::open_in_memory().unwrap();
        create(&database, MediaKind::Book, "By hand", None).unwrap();
        let old = looked_up(&database, MediaKind::Show, Source::TvMaze, "1");
        let new = looked_up(&database, MediaKind::Book, Source::OpenLibrary, "OL1W");
        let items = all(&database).unwrap();
        let mut skip = std::collections::HashSet::new();
        let cutoff = "2000-01-01T00:00:00.000Z";

        let first = super::next_due(&items, cutoff, &skip).unwrap();
        assert_eq!(first.id, new.id);
        assert_eq!(first.source, Source::OpenLibrary);
        skip.insert(first.id);
        assert_eq!(super::next_due(&items, cutoff, &skip).unwrap().id, old.id);
        skip.insert(old.id);
        assert_eq!(super::next_due(&items, cutoff, &skip), None);
    }

    #[test]
    fn an_item_the_source_no_longer_has_is_not_asked_for_again() {
        let database = Database::open_in_memory().unwrap();
        let item = looked_up(&database, MediaKind::Show, Source::TvMaze, "1");

        assert!(super::mark_without_details(&database, &item.id).unwrap());

        let items = all(&database).unwrap();
        assert_eq!(items[0].details(), Some(&Default::default()));
        assert_eq!(items[0].title, "Title");
        assert_eq!(
            super::next_due(&items, "2000-01-01T00:00:00.000Z", &Default::default()),
            None
        );
        assert!(!super::mark_without_details(&database, "missing").unwrap());
    }
}
