use crate::storage::keyed_column;
use crate::{Database, Result, media::MediaKind};
use rusqlite::Row;

const COLUMNS: &str = "id, media_kind, category, label, position";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Status {
    pub id: String,
    pub media_kind: MediaKind,
    pub category: Category,
    pub label: String,
    pub position: i32,
}

impl Status {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            media_kind: keyed_column(row, 1, MediaKind::from_key)?,
            category: keyed_column(row, 2, Category::from_key)?,
            label: row.get(3)?,
            position: row.get(4)?,
        })
    }
}

pub fn all(database: &Database) -> Result<Vec<Status>> {
    database.select(
        &format!("SELECT {COLUMNS} FROM statuses ORDER BY media_kind, position"),
        [],
        Status::from_row,
    )
}

pub fn for_kind(database: &Database, kind: MediaKind) -> Result<Vec<Status>> {
    database.select(
        &format!("SELECT {COLUMNS} FROM statuses WHERE media_kind = ?1 ORDER BY position"),
        [kind.key()],
        Status::from_row,
    )
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Category {
    Planned,
    Active,
    Paused,
    Done,
    Dropped,
}

impl Category {
    pub const ALL: [Self; 5] = [
        Self::Planned,
        Self::Active,
        Self::Paused,
        Self::Done,
        Self::Dropped,
    ];

    pub const fn key(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Done => "done",
            Self::Dropped => "dropped",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|category| category.key() == key)
    }
}

#[cfg(test)]
mod tests {
    use super::{Category, Status, all, for_kind};
    use crate::Database;
    use crate::media::MediaKind;

    fn database() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn labels(statuses: &[Status]) -> Vec<(&str, &str)> {
        statuses
            .iter()
            .map(|status| (status.category.key(), status.label.as_str()))
            .collect()
    }

    #[test]
    fn a_fresh_database_seeds_five_statuses_for_every_media_kind() {
        let database = database();

        assert_eq!(all(&database).unwrap().len(), 20);
        for kind in MediaKind::ALL {
            assert_eq!(for_kind(&database, kind).unwrap().len(), 5);
        }
    }

    #[test]
    fn every_media_kind_uses_the_labels_from_the_architecture() {
        let database = database();

        assert_eq!(
            labels(&for_kind(&database, MediaKind::Movie).unwrap()),
            [
                ("planned", "To watch"),
                ("active", "In progress"),
                ("paused", "On hold"),
                ("done", "Watched"),
                ("dropped", "Dropped"),
            ]
        );
        assert_eq!(
            labels(&for_kind(&database, MediaKind::Show).unwrap()),
            [
                ("planned", "To watch"),
                ("active", "In progress"),
                ("paused", "On hold"),
                ("done", "Watched"),
                ("dropped", "Dropped"),
            ]
        );
        assert_eq!(
            labels(&for_kind(&database, MediaKind::Book).unwrap()),
            [
                ("planned", "To read"),
                ("active", "In progress"),
                ("paused", "On hold"),
                ("done", "Read"),
                ("dropped", "Dropped"),
            ]
        );
        assert_eq!(
            labels(&for_kind(&database, MediaKind::Game).unwrap()),
            [
                ("planned", "To play"),
                ("active", "In progress"),
                ("paused", "On hold"),
                ("done", "Played"),
                ("dropped", "Dropped"),
            ]
        );
    }

    #[test]
    fn for_kind_returns_only_that_kind_in_position_order() {
        let database = database();

        for kind in MediaKind::ALL {
            let statuses = for_kind(&database, kind).unwrap();
            let positions: Vec<i32> = statuses.iter().map(|status| status.position).collect();

            assert!(statuses.iter().all(|status| status.media_kind == kind));
            assert_eq!(positions, [0, 1, 2, 3, 4]);
        }
    }

    #[test]
    fn seeded_status_ids_are_stable_slugs() {
        let database = database();
        let ids: Vec<String> = all(&database)
            .unwrap()
            .into_iter()
            .map(|status| status.id)
            .collect();

        for id in [
            "movie-planned",
            "show-planned",
            "book-planned",
            "game-planned",
            "movie-done",
            "book-done",
            "game-done",
        ] {
            assert!(ids.contains(&id.to_owned()), "missing status {id}");
        }
    }

    #[test]
    fn every_category_has_a_stable_key() {
        for category in Category::ALL {
            assert_eq!(Category::from_key(category.key()), Some(category));
        }
    }

    #[test]
    fn the_five_categories_are_planned_active_paused_done_dropped() {
        assert_eq!(
            Category::ALL.map(Category::key),
            ["planned", "active", "paused", "done", "dropped"]
        );
    }

    #[test]
    fn an_unknown_category_key_is_rejected() {
        assert_eq!(Category::from_key("someday"), None);
    }
}
