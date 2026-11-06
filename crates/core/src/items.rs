use crate::{Database, Error, Result, media::MediaKind};
use rusqlite::{Row, params, types::Type};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Item {
    pub id: String,
    pub media_kind: MediaKind,
    pub title: String,
    pub release_year: Option<i32>,
    pub release_date: Option<String>,
    pub finished_at: Option<String>,
}

impl Item {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            title: row.get(1)?,
            media_kind: media_kind(row, 2)?,
            release_year: row.get(3)?,
            release_date: row.get(4)?,
            finished_at: row.get(5)?,
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
            "INSERT INTO items (id, media_kind, title, release_year, created_at)
             VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             RETURNING id, title, media_kind, release_year, release_date, finished_at",
            params![id, media_kind.key(), title, release_year],
            Item::from_row,
        )
        .map_err(Error::from)
}

pub fn all(database: &Database) -> Result<Vec<Item>> {
    let mut statement = database.connection().prepare(
        "SELECT id, title, media_kind, release_year, release_date, finished_at
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
        "UPDATE items SET media_kind = ?1, title = ?2, release_year = ?3 WHERE id = ?4",
        params![media_kind.key(), title, release_year, id],
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
    use super::{Item, all, create, delete, update};
    use crate::Database;
    use crate::media::MediaKind;

    fn database() -> Database {
        Database::open_in_memory().unwrap()
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
