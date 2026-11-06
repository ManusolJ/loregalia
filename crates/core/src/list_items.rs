use crate::{Database, Error, Result, items::Item};
use rusqlite::{OptionalExtension, Row, params};

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

fn position(database: &Database, list_id: &str, item_id: &str) -> Result<Option<i32>> {
    Ok(database
        .connection()
        .query_row(
            "SELECT position FROM list_items WHERE list_id = ?1 AND item_id = ?2",
            params![list_id, item_id],
            |row| row.get(0),
        )
        .optional()?)
}

fn previous(database: &Database, list_id: &str, position: i32) -> Result<Option<(String, i32)>> {
    Ok(database
        .connection()
        .query_row(
            "SELECT item_id, position FROM list_items
             WHERE list_id = ?1 AND position < ?2
             ORDER BY position DESC LIMIT 1",
            params![list_id, position],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?)
}

fn next(database: &Database, list_id: &str, position: i32) -> Result<Option<(String, i32)>> {
    Ok(database
        .connection()
        .query_row(
            "SELECT item_id, position FROM list_items
             WHERE list_id = ?1 AND position > ?2
             ORDER BY position ASC LIMIT 1",
            params![list_id, position],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?)
}

fn swap(
    database: &Database,
    list_id: &str,
    first_item: &str,
    first_position: i32,
    second_item: &str,
    second_position: i32,
) -> Result<()> {
    database.connection().execute(
        "UPDATE list_items
         SET position = CASE item_id
             WHEN ?1 THEN ?3
             WHEN ?2 THEN ?4
         END
         WHERE list_id = ?5 AND item_id IN (?1, ?2)",
        params![
            first_item,
            second_item,
            second_position,
            first_position,
            list_id
        ],
    )?;

    Ok(())
}

pub fn move_up(database: &Database, list_id: &str, item_id: &str) -> Result<bool> {
    let Some(position) = position(database, list_id, item_id)? else {
        return Ok(false);
    };
    let Some((previous_id, previous_position)) = previous(database, list_id, position)? else {
        return Ok(false);
    };

    swap(
        database,
        list_id,
        item_id,
        position,
        &previous_id,
        previous_position,
    )?;

    Ok(true)
}

pub fn move_down(database: &Database, list_id: &str, item_id: &str) -> Result<bool> {
    let Some(position) = position(database, list_id, item_id)? else {
        return Ok(false);
    };
    let Some((next_id, next_position)) = next(database, list_id, position)? else {
        return Ok(false);
    };

    swap(
        database,
        list_id,
        item_id,
        position,
        &next_id,
        next_position,
    )?;

    Ok(true)
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
    use super::{Membership, add, all, memberships, move_down, move_up, remove};
    use crate::media::MediaKind;
    use crate::{Database, items, lists};

    fn database() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn titles(database: &Database, list_id: &str) -> Vec<String> {
        all(database, list_id)
            .unwrap()
            .into_iter()
            .map(|item| item.title)
            .collect()
    }

    #[test]
    fn moving_an_item_up_swaps_it_with_the_previous_one() {
        let database = database();
        let list = lists::create(&database, "Queue").unwrap();
        let first = items::create(&database, MediaKind::Movie, "A", None).unwrap();
        let second = items::create(&database, MediaKind::Book, "B", None).unwrap();
        let third = items::create(&database, MediaKind::Game, "C", None).unwrap();
        add(&database, &list.id, &first.id).unwrap();
        add(&database, &list.id, &second.id).unwrap();
        add(&database, &list.id, &third.id).unwrap();

        assert!(move_up(&database, &list.id, &third.id).unwrap());

        assert_eq!(titles(&database, &list.id), ["A", "C", "B"]);
        let positions: Vec<i32> = memberships(&database)
            .unwrap()
            .into_iter()
            .filter(|membership| membership.list_id == list.id)
            .map(|membership| membership.position)
            .collect();
        assert_eq!(positions, [0, 1, 2]);
    }

    #[test]
    fn moving_an_item_down_swaps_it_with_the_next_one() {
        let database = database();
        let list = lists::create(&database, "Queue").unwrap();
        let first = items::create(&database, MediaKind::Movie, "A", None).unwrap();
        let second = items::create(&database, MediaKind::Book, "B", None).unwrap();
        let third = items::create(&database, MediaKind::Game, "C", None).unwrap();
        add(&database, &list.id, &first.id).unwrap();
        add(&database, &list.id, &second.id).unwrap();
        add(&database, &list.id, &third.id).unwrap();

        assert!(move_down(&database, &list.id, &first.id).unwrap());

        assert_eq!(titles(&database, &list.id), ["B", "A", "C"]);
    }

    #[test]
    fn the_first_item_cannot_move_up() {
        let database = database();
        let list = lists::create(&database, "Queue").unwrap();
        let first = items::create(&database, MediaKind::Movie, "A", None).unwrap();
        let second = items::create(&database, MediaKind::Book, "B", None).unwrap();
        add(&database, &list.id, &first.id).unwrap();
        add(&database, &list.id, &second.id).unwrap();

        assert!(!move_up(&database, &list.id, &first.id).unwrap());

        assert_eq!(titles(&database, &list.id), ["A", "B"]);
    }

    #[test]
    fn the_last_item_cannot_move_down() {
        let database = database();
        let list = lists::create(&database, "Queue").unwrap();
        let first = items::create(&database, MediaKind::Movie, "A", None).unwrap();
        let second = items::create(&database, MediaKind::Book, "B", None).unwrap();
        add(&database, &list.id, &first.id).unwrap();
        add(&database, &list.id, &second.id).unwrap();

        assert!(!move_down(&database, &list.id, &second.id).unwrap());

        assert_eq!(titles(&database, &list.id), ["A", "B"]);
    }

    #[test]
    fn moving_a_missing_item_reports_no_change() {
        let database = database();
        let list = lists::create(&database, "Queue").unwrap();
        let item = items::create(&database, MediaKind::Movie, "A", None).unwrap();
        add(&database, &list.id, &item.id).unwrap();

        assert!(!move_up(&database, &list.id, "missing").unwrap());
        assert!(!move_down(&database, &list.id, "missing").unwrap());

        assert_eq!(titles(&database, &list.id), ["A"]);
    }

    #[test]
    fn moving_an_item_only_affects_its_own_list() {
        let database = database();
        let first_list = lists::create(&database, "First").unwrap();
        let second_list = lists::create(&database, "Second").unwrap();
        let first = items::create(&database, MediaKind::Movie, "A", None).unwrap();
        let second = items::create(&database, MediaKind::Book, "B", None).unwrap();
        let other = items::create(&database, MediaKind::Game, "C", None).unwrap();
        add(&database, &first_list.id, &first.id).unwrap();
        add(&database, &first_list.id, &second.id).unwrap();
        add(&database, &second_list.id, &other.id).unwrap();

        assert!(!move_up(&database, &second_list.id, &other.id).unwrap());

        assert_eq!(titles(&database, &first_list.id), ["A", "B"]);
        assert_eq!(titles(&database, &second_list.id), ["C"]);
    }

    #[test]
    fn moving_after_a_removal_swaps_with_the_previous_neighbour() {
        let database = database();
        let list = lists::create(&database, "Queue").unwrap();
        let first = items::create(&database, MediaKind::Movie, "A", None).unwrap();
        let second = items::create(&database, MediaKind::Book, "B", None).unwrap();
        let third = items::create(&database, MediaKind::Game, "C", None).unwrap();
        add(&database, &list.id, &first.id).unwrap();
        add(&database, &list.id, &second.id).unwrap();
        add(&database, &list.id, &third.id).unwrap();
        remove(&database, &list.id, &second.id).unwrap();

        assert!(move_up(&database, &list.id, &third.id).unwrap());

        assert_eq!(titles(&database, &list.id), ["C", "A"]);
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
