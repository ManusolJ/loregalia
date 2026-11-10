use crate::{Database, Error, Result};
use rusqlite::{Row, params};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct List {
    pub id: String,
    pub name: String,
    pub created_at: String,
}

impl List {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            name: row.get(1)?,
            created_at: row.get(2)?,
        })
    }
}

pub fn create(database: &Database, name: &str) -> Result<List> {
    let id = Uuid::now_v7().to_string();

    database
        .connection()
        .query_row(
            "INSERT INTO lists (id, name, created_at)
             VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             RETURNING id, name, created_at",
            params![id, name],
            List::from_row,
        )
        .map_err(Error::from)
}

pub fn all(database: &Database) -> Result<Vec<List>> {
    database.select(
        "SELECT id, name, created_at FROM lists ORDER BY created_at, rowid",
        [],
        List::from_row,
    )
}

pub fn rename(database: &Database, id: &str, name: &str) -> Result<bool> {
    let changed = database.connection().execute(
        "UPDATE lists SET name = ?1 WHERE id = ?2",
        params![name, id],
    )?;

    Ok(changed > 0)
}

pub fn delete(database: &Database, id: &str) -> Result<bool> {
    let changed = database
        .connection()
        .execute("DELETE FROM lists WHERE id = ?1", [id])?;

    Ok(changed > 0)
}

#[cfg(test)]
mod tests {
    use super::{List, all, create, delete, rename};
    use uuid::Uuid;

    fn database() -> crate::Database {
        crate::Database::open_in_memory().unwrap()
    }

    #[test]
    fn a_created_list_round_trips() {
        let database = database();

        let created = create(&database, "Watchlist").unwrap();
        let stored = all(&database).unwrap();

        assert_eq!(stored, vec![created]);
    }

    #[test]
    fn ids_are_uuid_v7() {
        let database = database();

        let list = create(&database, "Watchlist").unwrap();

        let id = Uuid::parse_str(&list.id).unwrap();
        assert_eq!(id.get_version_num(), 7);
    }

    #[test]
    fn timestamps_are_iso_8601_utc() {
        let database = database();

        let list = create(&database, "Watchlist").unwrap();

        assert_eq!(list.created_at.len(), 24);
        assert!(list.created_at.ends_with('Z'));
        assert_eq!(&list.created_at[10..11], "T");
        assert_eq!(&list.created_at[19..20], ".");
    }

    #[test]
    fn names_are_stored_as_values_not_sql() {
        let database = database();
        let name = "O'Brien; DROP TABLE lists; --";

        let list = create(&database, name).unwrap();

        assert_eq!(list.name, name);
        let remaining: i64 = database
            .connection()
            .query_row("SELECT COUNT(*) FROM lists", [], |row| row.get(0))
            .unwrap();
        assert_eq!(remaining, 1);
    }

    #[test]
    fn an_empty_database_has_no_lists() {
        let database = database();

        assert_eq!(all(&database).unwrap(), Vec::<List>::new());
    }

    #[test]
    fn lists_keep_their_insertion_order() {
        let database = database();

        create(&database, "First").unwrap();
        create(&database, "Second").unwrap();

        let names: Vec<String> = all(&database)
            .unwrap()
            .into_iter()
            .map(|list| list.name)
            .collect();
        assert_eq!(names, ["First", "Second"]);
    }

    #[test]
    fn rename_changes_the_name_and_keeps_the_identity() {
        let database = database();
        let list = create(&database, "Watchlist").unwrap();

        assert!(rename(&database, &list.id, "Weekend").unwrap());

        let stored = all(&database).unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].id, list.id);
        assert_eq!(stored[0].name, "Weekend");
        assert_eq!(stored[0].created_at, list.created_at);
    }

    #[test]
    fn renaming_a_missing_list_reports_no_change() {
        let database = database();

        assert!(!rename(&database, "missing", "Weekend").unwrap());
        assert_eq!(all(&database).unwrap(), Vec::<List>::new());
    }

    #[test]
    fn delete_removes_only_the_named_list() {
        let database = database();
        let keep = create(&database, "Keep").unwrap();
        let remove = create(&database, "Remove").unwrap();

        assert!(delete(&database, &remove.id).unwrap());

        assert_eq!(all(&database).unwrap(), vec![keep]);
    }

    #[test]
    fn deleting_a_missing_list_reports_no_change() {
        let database = database();
        let list = create(&database, "Keep").unwrap();

        assert!(!delete(&database, "missing").unwrap());
        assert_eq!(all(&database).unwrap(), vec![list]);
    }
}
