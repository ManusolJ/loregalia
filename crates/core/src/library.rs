use crate::items::Item;
use crate::media::MediaKind;
use crate::statuses::{Category, Status};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Shelf {
    All,
    UpNext,
    Finished,
}

impl Shelf {
    pub const ALL: [Self; 3] = [Self::All, Self::UpNext, Self::Finished];

    pub const fn key(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::UpNext => "up-next",
            Self::Finished => "finished",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "All items",
            Self::UpNext => "Up next",
            Self::Finished => "Finished",
        }
    }

    pub const fn empty_message(self) -> &'static str {
        match self {
            Self::All => "Nothing planned yet",
            Self::UpNext => "Nothing in progress or on hold",
            Self::Finished => "Nothing finished yet",
        }
    }

    fn holds(self, category: Option<Category>) -> bool {
        match self {
            Self::All => true,
            Self::UpNext => matches!(category, Some(Category::Active | Category::Paused)),
            Self::Finished => category == Some(Category::Done),
        }
    }
}

pub fn category_of(statuses: &[Status], status_id: &str) -> Option<Category> {
    statuses
        .iter()
        .find(|status| status.id == status_id)
        .map(|status| status.category)
}

pub fn shelve<'a>(
    items: &'a [Item],
    statuses: &[Status],
    shelf: Shelf,
    kind: Option<MediaKind>,
) -> Vec<&'a Item> {
    let mut shelved: Vec<&Item> = items
        .iter()
        .filter(|item| kind.is_none_or(|kind| item.media_kind == kind))
        .filter(|item| shelf.holds(category_of(statuses, &item.status_id)))
        .collect();

    match shelf {
        Shelf::All => {}
        Shelf::UpNext => shelved
            .sort_by_key(|item| category_of(statuses, &item.status_id) != Some(Category::Active)),
        Shelf::Finished => shelved.sort_by(|left, right| right.finished_at.cmp(&left.finished_at)),
    }

    shelved
}

#[cfg(test)]
mod tests {
    use super::{Shelf, category_of, shelve};
    use crate::media::MediaKind;
    use crate::statuses::Category;
    use crate::{Database, items, statuses};

    fn titles(items: &[&items::Item]) -> Vec<String> {
        items.iter().map(|item| item.title.clone()).collect()
    }

    struct Library {
        items: Vec<items::Item>,
        statuses: Vec<statuses::Status>,
    }

    fn library() -> Library {
        let database = Database::open_in_memory().unwrap();
        let paused = items::create(&database, MediaKind::Book, "Paused book", None).unwrap();
        items::create(&database, MediaKind::Movie, "Planned movie", None).unwrap();
        let active = items::create(&database, MediaKind::Show, "Active show", None).unwrap();
        let first_done = items::create(&database, MediaKind::Game, "First done", None).unwrap();
        let second_done = items::create(&database, MediaKind::Movie, "Second done", None).unwrap();
        items::set_status(&database, &paused.id, "book-paused").unwrap();
        items::set_status(&database, &active.id, "show-active").unwrap();
        items::set_status(&database, &first_done.id, "game-done").unwrap();
        items::set_status(&database, &second_done.id, "movie-done").unwrap();

        let mut items = items::all(&database).unwrap();
        for item in &mut items {
            if item.id == first_done.id {
                item.finished_at = Some("2026-01-01T00:00:00.000Z".to_owned());
            }
            if item.id == second_done.id {
                item.finished_at = Some("2026-06-01T00:00:00.000Z".to_owned());
            }
        }

        Library {
            items,
            statuses: statuses::all(&database).unwrap(),
        }
    }

    #[test]
    fn every_shelf_has_a_stable_key_label_and_empty_message() {
        let keys: Vec<&str> = Shelf::ALL.iter().map(|shelf| shelf.key()).collect();

        assert_eq!(keys, ["all", "up-next", "finished"]);
        assert!(Shelf::ALL.iter().all(|shelf| !shelf.label().is_empty()));
        assert!(
            Shelf::ALL
                .iter()
                .all(|shelf| !shelf.empty_message().is_empty())
        );
    }

    #[test]
    fn all_keeps_every_item_in_its_order() {
        let library = library();

        let shelved = shelve(&library.items, &library.statuses, Shelf::All, None);

        assert_eq!(
            titles(&shelved),
            [
                "Paused book",
                "Planned movie",
                "Active show",
                "First done",
                "Second done"
            ]
        );
    }

    #[test]
    fn up_next_holds_active_items_before_paused_ones() {
        let library = library();

        let shelved = shelve(&library.items, &library.statuses, Shelf::UpNext, None);

        assert_eq!(titles(&shelved), ["Active show", "Paused book"]);
    }

    #[test]
    fn finished_holds_done_items_newest_first() {
        let library = library();

        let shelved = shelve(&library.items, &library.statuses, Shelf::Finished, None);

        assert_eq!(titles(&shelved), ["Second done", "First done"]);
    }

    #[test]
    fn a_kind_narrows_any_shelf() {
        let library = library();

        let movies = shelve(
            &library.items,
            &library.statuses,
            Shelf::All,
            Some(MediaKind::Movie),
        );
        let finished_games = shelve(
            &library.items,
            &library.statuses,
            Shelf::Finished,
            Some(MediaKind::Game),
        );

        assert_eq!(titles(&movies), ["Planned movie", "Second done"]);
        assert_eq!(titles(&finished_games), ["First done"]);
    }

    #[test]
    fn an_unknown_status_has_no_category() {
        let library = library();

        assert_eq!(
            category_of(&library.statuses, "movie-done"),
            Some(Category::Done)
        );
        assert_eq!(category_of(&library.statuses, "missing"), None);
    }
}
