use crate::{Database, Error, Result, media::MediaKind};
use rusqlite::{OptionalExtension, Row, params, types::Type};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Item {
    pub id: String,
    pub media_kind: MediaKind,
    pub title: String,
    pub release_year: Option<i32>,
    pub release_date: Option<String>,
    pub finished_at: Option<String>,
    pub status_id: String,
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
        })
    }
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
             RETURNING id, title, media_kind, release_year, release_date, finished_at, status_id",
            params![id, media_kind.key(), title, release_year],
            Item::from_row,
        )
        .map_err(Error::from)
}

pub fn all(database: &Database) -> Result<Vec<Item>> {
    let mut statement = database.connection().prepare(
        "SELECT id, title, media_kind, release_year, release_date, finished_at, status_id
         FROM items ORDER BY created_at, rowid",
    )?;
    let rows = statement.query_map([], Item::from_row)?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Error::from)
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
    use super::{Item, all, create, delete, set_status, update};
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
    }

    #[test]
    fn hand_typed_duplicates_are_allowed() {
        let database = database();

        create(&database, MediaKind::Movie, "Dune", Some(1984)).unwrap();
        create(&database, MediaKind::Movie, "Dune", Some(2021)).unwrap();

        assert_eq!(all(&database).unwrap().len(), 2);
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
