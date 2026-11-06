use crate::{Database, Error, Result};
use rusqlite::{OptionalExtension, Row, params};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tag {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tagging {
    pub owner_id: String,
    pub tag_id: String,
}

impl Tag {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            name: row.get(1)?,
        })
    }
}

impl Tagging {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            owner_id: row.get(0)?,
            tag_id: row.get(1)?,
        })
    }
}

pub fn all(database: &Database) -> Result<Vec<Tag>> {
    let mut statement = database
        .connection()
        .prepare("SELECT id, name FROM tags ORDER BY name")?;
    let rows = statement.query_map([], Tag::from_row)?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Error::from)
}

fn by_name(database: &Database, name: &str) -> Result<Option<Tag>> {
    Ok(database
        .connection()
        .query_row(
            "SELECT id, name FROM tags WHERE name = ?1",
            [name],
            Tag::from_row,
        )
        .optional()?)
}

pub fn create(database: &Database, name: &str) -> Result<Tag> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::EmptyTagName);
    }
    if let Some(tag) = by_name(database, name)? {
        return Ok(tag);
    }

    let id = Uuid::now_v7().to_string();
    database
        .connection()
        .query_row(
            "INSERT INTO tags (id, name, created_at)
             VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             RETURNING id, name",
            params![id, name],
            Tag::from_row,
        )
        .map_err(Error::from)
}

pub fn for_item(database: &Database, item_id: &str) -> Result<Vec<Tag>> {
    let mut statement = database.connection().prepare(
        "SELECT t.id, t.name FROM item_tags link
         JOIN tags t ON t.id = link.tag_id
         WHERE link.item_id = ?1 ORDER BY t.name",
    )?;
    let rows = statement.query_map([item_id], Tag::from_row)?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Error::from)
}

pub fn for_list(database: &Database, list_id: &str) -> Result<Vec<Tag>> {
    let mut statement = database.connection().prepare(
        "SELECT t.id, t.name FROM list_tags link
         JOIN tags t ON t.id = link.tag_id
         WHERE link.list_id = ?1 ORDER BY t.name",
    )?;
    let rows = statement.query_map([list_id], Tag::from_row)?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Error::from)
}

pub fn item_taggings(database: &Database) -> Result<Vec<Tagging>> {
    let mut statement = database
        .connection()
        .prepare("SELECT item_id, tag_id FROM item_tags ORDER BY item_id, tag_id")?;
    let rows = statement.query_map([], Tagging::from_row)?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Error::from)
}

pub fn list_taggings(database: &Database) -> Result<Vec<Tagging>> {
    let mut statement = database
        .connection()
        .prepare("SELECT list_id, tag_id FROM list_tags ORDER BY list_id, tag_id")?;
    let rows = statement.query_map([], Tagging::from_row)?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Error::from)
}

pub fn attach_to_item(database: &Database, item_id: &str, tag_id: &str) -> Result<bool> {
    let changed = database.connection().execute(
        "INSERT INTO item_tags (item_id, tag_id) VALUES (?1, ?2)
         ON CONFLICT (item_id, tag_id) DO NOTHING",
        params![item_id, tag_id],
    )?;

    Ok(changed > 0)
}

pub fn detach_from_item(database: &Database, item_id: &str, tag_id: &str) -> Result<bool> {
    let changed = database.connection().execute(
        "DELETE FROM item_tags WHERE item_id = ?1 AND tag_id = ?2",
        params![item_id, tag_id],
    )?;

    Ok(changed > 0)
}

pub fn attach_to_list(database: &Database, list_id: &str, tag_id: &str) -> Result<bool> {
    let changed = database.connection().execute(
        "INSERT INTO list_tags (list_id, tag_id) VALUES (?1, ?2)
         ON CONFLICT (list_id, tag_id) DO NOTHING",
        params![list_id, tag_id],
    )?;

    Ok(changed > 0)
}

pub fn detach_from_list(database: &Database, list_id: &str, tag_id: &str) -> Result<bool> {
    let changed = database.connection().execute(
        "DELETE FROM list_tags WHERE list_id = ?1 AND tag_id = ?2",
        params![list_id, tag_id],
    )?;

    Ok(changed > 0)
}

#[cfg(test)]
mod tests {
    use super::{
        Tagging, all, attach_to_item, attach_to_list, create, detach_from_item, detach_from_list,
        for_item, for_list, item_taggings, list_taggings,
    };
    use crate::media::MediaKind;
    use crate::{Database, Error, items, lists};

    fn database() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn a_created_tag_round_trips() {
        let database = database();

        let tag = create(&database, "Sci-Fi").unwrap();

        assert_eq!(tag.name, "Sci-Fi");
        assert_eq!(all(&database).unwrap(), vec![tag]);
    }

    #[test]
    fn a_tag_name_is_unique_ignoring_case() {
        let database = database();

        let first = create(&database, "Sci-Fi").unwrap();
        let second = create(&database, "sci-fi").unwrap();

        assert_eq!(second.id, first.id);
        assert_eq!(second.name, "Sci-Fi");
        assert_eq!(all(&database).unwrap().len(), 1);
    }

    #[test]
    fn an_empty_tag_name_is_an_error() {
        let database = database();

        assert!(matches!(create(&database, "  "), Err(Error::EmptyTagName)));
        assert!(all(&database).unwrap().is_empty());
    }

    #[test]
    fn attaching_a_tag_to_an_item_round_trips() {
        let database = database();
        let item = items::create(&database, MediaKind::Movie, "Dune", None).unwrap();
        let tag = create(&database, "Sci-Fi").unwrap();

        assert!(attach_to_item(&database, &item.id, &tag.id).unwrap());
        assert!(!attach_to_item(&database, &item.id, &tag.id).unwrap());

        assert_eq!(for_item(&database, &item.id).unwrap(), vec![tag.clone()]);
        assert_eq!(
            item_taggings(&database).unwrap(),
            vec![Tagging {
                owner_id: item.id,
                tag_id: tag.id,
            }]
        );
    }

    #[test]
    fn detaching_a_tag_from_an_item_round_trips() {
        let database = database();
        let item = items::create(&database, MediaKind::Movie, "Dune", None).unwrap();
        let tag = create(&database, "Sci-Fi").unwrap();
        attach_to_item(&database, &item.id, &tag.id).unwrap();

        assert!(detach_from_item(&database, &item.id, &tag.id).unwrap());
        assert!(!detach_from_item(&database, &item.id, &tag.id).unwrap());

        assert_eq!(for_item(&database, &item.id).unwrap(), Vec::new());
        assert!(item_taggings(&database).unwrap().is_empty());
    }

    #[test]
    fn attaching_a_tag_to_a_list_round_trips() {
        let database = database();
        let list = lists::create(&database, "Queue").unwrap();
        let tag = create(&database, "Weekend").unwrap();

        assert!(attach_to_list(&database, &list.id, &tag.id).unwrap());
        assert!(!attach_to_list(&database, &list.id, &tag.id).unwrap());

        assert_eq!(for_list(&database, &list.id).unwrap(), vec![tag.clone()]);
        assert_eq!(
            list_taggings(&database).unwrap(),
            vec![Tagging {
                owner_id: list.id,
                tag_id: tag.id,
            }]
        );
    }

    #[test]
    fn detaching_a_tag_from_a_list_round_trips() {
        let database = database();
        let list = lists::create(&database, "Queue").unwrap();
        let tag = create(&database, "Weekend").unwrap();
        attach_to_list(&database, &list.id, &tag.id).unwrap();

        assert!(detach_from_list(&database, &list.id, &tag.id).unwrap());
        assert!(!detach_from_list(&database, &list.id, &tag.id).unwrap());

        assert_eq!(for_list(&database, &list.id).unwrap(), Vec::new());
        assert!(list_taggings(&database).unwrap().is_empty());
    }

    #[test]
    fn a_tag_can_be_on_an_item_and_a_list_at_once() {
        let database = database();
        let item = items::create(&database, MediaKind::Movie, "Dune", None).unwrap();
        let list = lists::create(&database, "Queue").unwrap();
        let tag = create(&database, "Sci-Fi").unwrap();

        attach_to_item(&database, &item.id, &tag.id).unwrap();
        attach_to_list(&database, &list.id, &tag.id).unwrap();

        assert_eq!(for_item(&database, &item.id).unwrap(), vec![tag.clone()]);
        assert_eq!(for_list(&database, &list.id).unwrap(), vec![tag]);
    }

    #[test]
    fn deleting_an_item_removes_its_tag_links_but_keeps_the_tag() {
        let database = database();
        let item = items::create(&database, MediaKind::Movie, "Dune", None).unwrap();
        let tag = create(&database, "Sci-Fi").unwrap();
        attach_to_item(&database, &item.id, &tag.id).unwrap();

        items::delete(&database, &item.id).unwrap();

        assert!(item_taggings(&database).unwrap().is_empty());
        assert_eq!(all(&database).unwrap(), vec![tag]);
    }

    #[test]
    fn deleting_a_list_removes_its_tag_links_but_keeps_the_tag() {
        let database = database();
        let list = lists::create(&database, "Queue").unwrap();
        let tag = create(&database, "Weekend").unwrap();
        attach_to_list(&database, &list.id, &tag.id).unwrap();

        lists::delete(&database, &list.id).unwrap();

        assert!(list_taggings(&database).unwrap().is_empty());
        assert_eq!(all(&database).unwrap(), vec![tag]);
    }

    #[test]
    fn deleting_a_tag_removes_its_links() {
        let database = database();
        let item = items::create(&database, MediaKind::Movie, "Dune", None).unwrap();
        let list = lists::create(&database, "Queue").unwrap();
        let tag = create(&database, "Sci-Fi").unwrap();
        attach_to_item(&database, &item.id, &tag.id).unwrap();
        attach_to_list(&database, &list.id, &tag.id).unwrap();

        database
            .connection()
            .execute("DELETE FROM tags WHERE id = ?1", [&tag.id])
            .unwrap();

        assert!(item_taggings(&database).unwrap().is_empty());
        assert!(list_taggings(&database).unwrap().is_empty());
    }

    #[test]
    fn attaching_a_missing_item_or_tag_is_an_error() {
        let database = database();
        let item = items::create(&database, MediaKind::Movie, "Dune", None).unwrap();
        let tag = create(&database, "Sci-Fi").unwrap();

        assert!(attach_to_item(&database, "missing", &tag.id).is_err());
        assert!(attach_to_item(&database, &item.id, "missing").is_err());
    }
}
