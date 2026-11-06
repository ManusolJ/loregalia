use crate::{Database, Error, Result, items::Item};
use rusqlite::params;

pub fn add(database: &Database, list_id: &str, item_id: &str) -> Result<bool> {
    let changed = database.connection().execute(
        "INSERT INTO list_items (list_id, item_id, position)
         SELECT ?1, ?2, COALESCE(MAX(position) + 1, 0)
         FROM list_items WHERE list_id = ?1
         ON CONFLICT (list_id, item_id) DO NOTHING",
        params![list_id, item_id],
    )?;

    Ok(changed > 0)
}

pub fn remove(database: &Database, list_id: &str, item_id: &str) -> Result<bool> {
    let changed = database.connection().execute(
        "DELETE FROM list_items WHERE list_id = ?1 AND item_id = ?2",
        params![list_id, item_id],
    )?;

    Ok(changed > 0)
}

pub fn all(database: &Database, list_id: &str) -> Result<Vec<Item>> {
    let mut statement = database.connection().prepare(
        "SELECT i.id, i.title, i.media_kind, i.release_year, i.release_date, i.finished_at
         FROM list_items li
         JOIN items i ON i.id = li.item_id
         WHERE li.list_id = ?1
         ORDER BY li.position, li.rowid",
    )?;
    let rows = statement.query_map([list_id], Item::from_row)?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Error::from)
}

#[cfg(test)]
mod tests {
    use super::{add, all, remove};
    use crate::media::MediaKind;
    use crate::{Database, items, lists};

    fn database() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn membership_count(database: &Database) -> i64 {
        database
            .connection()
            .query_row("SELECT COUNT(*) FROM list_items", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn adding_items_keeps_their_position_order() {
        let database = database();
        let list = lists::create(&database, "Queue").unwrap();
        let first = items::create(&database, MediaKind::Movie, "First", None).unwrap();
        let second = items::create(&database, MediaKind::Book, "Second", None).unwrap();

        assert!(add(&database, &list.id, &first.id).unwrap());
        assert!(add(&database, &list.id, &second.id).unwrap());

        let stored: Vec<String> = all(&database, &list.id)
            .unwrap()
            .into_iter()
            .map(|item| item.title)
            .collect();
        assert_eq!(stored, ["First", "Second"]);
    }

    #[test]
    fn adding_the_same_item_twice_does_not_duplicate_it() {
        let database = database();
        let list = lists::create(&database, "Queue").unwrap();
        let item = items::create(&database, MediaKind::Game, "Only once", None).unwrap();

        assert!(add(&database, &list.id, &item.id).unwrap());
        assert!(!add(&database, &list.id, &item.id).unwrap());

        assert_eq!(all(&database, &list.id).unwrap().len(), 1);
        assert_eq!(membership_count(&database), 1);
    }

    #[test]
    fn removing_an_item_keeps_the_rest() {
        let database = database();
        let list = lists::create(&database, "Queue").unwrap();
        let keep = items::create(&database, MediaKind::Show, "Keep", None).unwrap();
        let dropped = items::create(&database, MediaKind::Movie, "Remove", None).unwrap();
        add(&database, &list.id, &keep.id).unwrap();
        add(&database, &list.id, &dropped.id).unwrap();

        assert!(remove(&database, &list.id, &dropped.id).unwrap());

        assert_eq!(all(&database, &list.id).unwrap(), vec![keep]);
        assert!(!remove(&database, &list.id, &dropped.id).unwrap());
    }

    #[test]
    fn deleting_a_list_removes_its_memberships() {
        let database = database();
        let list = lists::create(&database, "Queue").unwrap();
        let item = items::create(&database, MediaKind::Movie, "Item", None).unwrap();
        add(&database, &list.id, &item.id).unwrap();

        assert!(lists::delete(&database, &list.id).unwrap());

        assert_eq!(membership_count(&database), 0);
        assert_eq!(items::all(&database).unwrap(), vec![item]);
    }

    #[test]
    fn deleting_an_item_removes_its_memberships() {
        let database = database();
        let list = lists::create(&database, "Queue").unwrap();
        let item = items::create(&database, MediaKind::Movie, "Item", None).unwrap();
        add(&database, &list.id, &item.id).unwrap();

        assert!(items::delete(&database, &item.id).unwrap());

        assert_eq!(membership_count(&database), 0);
        assert_eq!(lists::all(&database).unwrap(), vec![list]);
    }

    #[test]
    fn adding_a_missing_item_or_list_is_an_error() {
        let database = database();
        let list = lists::create(&database, "Queue").unwrap();
        let item = items::create(&database, MediaKind::Movie, "Item", None).unwrap();

        assert!(add(&database, "missing", &item.id).is_err());
        assert!(add(&database, &list.id, "missing").is_err());
        assert_eq!(membership_count(&database), 0);
    }
}
