use crate::{Database, Error, Result, items::Item};
use rusqlite::{Row, params};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Membership {
    pub list_id: String,
    pub item_id: String,
    pub position: i32,
}

impl Membership {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            list_id: row.get(0)?,
            item_id: row.get(1)?,
            position: row.get(2)?,
        })
    }
}

pub fn memberships(database: &Database) -> Result<Vec<Membership>> {
    let mut statement = database
        .connection()
        .prepare("SELECT list_id, item_id, position FROM list_items ORDER BY list_id, position")?;
    let rows = statement.query_map([], Membership::from_row)?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Error::from)
}

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
        "SELECT i.id, i.title, i.media_kind, i.release_year, i.release_date, i.finished_at, i.status_id
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
    use super::{Membership, add, all, memberships, remove};
    use crate::media::MediaKind;
    use crate::{Database, items, lists};

    fn database() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn a_fresh_database_has_no_memberships() {
        let database = database();

        assert_eq!(memberships(&database).unwrap(), Vec::<Membership>::new());
    }

    #[test]
    fn memberships_list_every_pair_with_its_position() {
        let database = database();
        let first_list = lists::create(&database, "First list").unwrap();
        let second_list = lists::create(&database, "Second list").unwrap();
        let first = items::create(&database, MediaKind::Movie, "First", None).unwrap();
        let second = items::create(&database, MediaKind::Book, "Second", None).unwrap();

        add(&database, &first_list.id, &first.id).unwrap();
        add(&database, &first_list.id, &second.id).unwrap();
        add(&database, &second_list.id, &second.id).unwrap();

        let stored = memberships(&database).unwrap();
        assert_eq!(stored.len(), 3);

        let first_list_items: Vec<&Membership> = stored
            .iter()
            .filter(|membership| membership.list_id == first_list.id)
            .collect();
        assert_eq!(first_list_items.len(), 2);
        assert_eq!(first_list_items[0].item_id, first.id);
        assert_eq!(first_list_items[0].position, 0);
        assert_eq!(first_list_items[1].item_id, second.id);
        assert_eq!(first_list_items[1].position, 1);

        let second_list_items: Vec<&Membership> = stored
            .iter()
            .filter(|membership| membership.list_id == second_list.id)
            .collect();
        assert_eq!(second_list_items.len(), 1);
        assert_eq!(second_list_items[0].item_id, second.id);
        assert_eq!(second_list_items[0].position, 0);
    }

    #[test]
    fn removing_a_membership_drops_only_that_pair() {
        let database = database();
        let list = lists::create(&database, "Queue").unwrap();
        let keep = items::create(&database, MediaKind::Game, "Keep", None).unwrap();
        let drop = items::create(&database, MediaKind::Show, "Drop", None).unwrap();
        add(&database, &list.id, &keep.id).unwrap();
        add(&database, &list.id, &drop.id).unwrap();

        remove(&database, &list.id, &drop.id).unwrap();

        let stored = memberships(&database).unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].item_id, keep.id);
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
