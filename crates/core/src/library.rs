use std::cmp::Reverse;

use crate::items::Item;
use crate::media::MediaKind;
use crate::statuses::{Category, Status};
use crate::tags::Tagging;

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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Sort {
    #[default]
    Shelf,
    Title,
    Newest,
    Oldest,
}

impl Sort {
    pub const ALL: [Self; 4] = [Self::Shelf, Self::Title, Self::Newest, Self::Oldest];

    pub const fn key(self) -> &'static str {
        match self {
            Self::Shelf => "shelf",
            Self::Title => "title",
            Self::Newest => "newest",
            Self::Oldest => "oldest",
        }
    }

    pub const fn label(self, shelf: Shelf) -> &'static str {
        match (self, shelf) {
            (Self::Shelf, Shelf::All) => "Order added",
            (Self::Shelf, Shelf::UpNext) => "In progress first",
            (Self::Shelf, Shelf::Finished) => "Last finished",
            (Self::Title, _) => "Title",
            (Self::Newest, _) => "Newest",
            (Self::Oldest, _) => "Oldest",
        }
    }
}

pub fn sort_items(items: &mut [&Item], sort: Sort) {
    match sort {
        Sort::Shelf => {}
        Sort::Title => items.sort_by_cached_key(|item| fold(&item.title)),
        Sort::Newest => {
            items.sort_by_key(|item| (item.release_year.is_none(), item.release_year.map(Reverse)));
        }
        Sort::Oldest => {
            items.sort_by_key(|item| (item.release_year.is_none(), item.release_year));
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Filter {
    pub text: String,
    pub tags: Vec<String>,
}

impl Filter {
    pub fn is_active(&self) -> bool {
        !self.text.trim().is_empty() || !self.tags.is_empty()
    }

    pub fn toggle_tag(&mut self, tag_id: &str) {
        if self.tags.iter().any(|tag| tag == tag_id) {
            self.tags.retain(|tag| tag != tag_id);
        } else {
            self.tags.push(tag_id.to_owned());
        }
    }

    pub fn keeps(&self, item: &Item, item_tags: &[Tagging]) -> bool {
        matches_text(&item.title, &self.text)
            && self.tags.iter().all(|tag_id| {
                item_tags
                    .iter()
                    .any(|link| link.owner_id == item.id && &link.tag_id == tag_id)
            })
    }
}

pub fn matches_text(title: &str, text: &str) -> bool {
    let title = fold(title);

    fold(text)
        .split_whitespace()
        .all(|word| title.contains(word))
}

fn fold(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|letter| !('\u{0300}'..='\u{036f}').contains(letter))
        .map(|letter| match letter {
            'à'..='å' | 'ā' | 'ă' | 'ą' => 'a',
            'ç' | 'ć' | 'č' => 'c',
            'ď' | 'đ' => 'd',
            'è'..='ë' | 'ē' | 'ė' | 'ę' | 'ě' => 'e',
            'ğ' => 'g',
            'ì'..='ï' | 'ī' | 'ı' => 'i',
            'ł' => 'l',
            'ñ' | 'ń' | 'ň' => 'n',
            'ò'..='ö' | 'ø' | 'ō' | 'ő' => 'o',
            'ř' => 'r',
            'ś' | 'ş' | 'š' => 's',
            'ť' | 'ţ' => 't',
            'ù'..='ü' | 'ū' | 'ů' | 'ű' => 'u',
            'ý' | 'ÿ' => 'y',
            'ź' | 'ż' | 'ž' => 'z',
            '’' | '‘' => '\'',
            other => other,
        })
        .collect()
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
    use super::{Filter, Shelf, Sort, category_of, matches_text, shelve, sort_items};
    use crate::media::MediaKind;
    use crate::statuses::Category;
    use crate::{Database, items, statuses, tags};

    #[test]
    fn text_matches_every_word_in_any_order() {
        assert!(matches_text("Dune: Part Two", "two dune"));
        assert!(matches_text("Dune: Part Two", "  DUN "));
        assert!(!matches_text("Dune: Part Two", "dune three"));
    }

    #[test]
    fn empty_text_matches_everything() {
        assert!(matches_text("Dune", ""));
        assert!(matches_text("Dune", "   "));
    }

    #[test]
    fn text_ignores_accents_in_both_directions() {
        assert!(matches_text("Pokémon Ñandú", "pokemon nandu"));
        assert!(matches_text("Amelie", "Amélie"));
        assert!(matches_text("Ame\u{301}lie", "amelie"));
        assert!(matches_text("Howl’s Moving Castle", "howl's"));
    }

    #[test]
    fn a_filter_needs_every_chosen_tag() {
        let database = Database::open_in_memory().unwrap();
        let both = items::create(&database, MediaKind::Movie, "Both", None).unwrap();
        let one = items::create(&database, MediaKind::Movie, "One", None).unwrap();
        let cozy = tags::create(&database, "cozy").unwrap();
        let short = tags::create(&database, "short").unwrap();
        tags::attach_to_item(&database, &both.id, &cozy.id).unwrap();
        tags::attach_to_item(&database, &both.id, &short.id).unwrap();
        tags::attach_to_item(&database, &one.id, &cozy.id).unwrap();
        let links = tags::item_taggings(&database).unwrap();

        let mut filter = Filter::default();
        assert!(!filter.is_active());
        filter.toggle_tag(&cozy.id);
        assert!(filter.is_active());
        assert!(filter.keeps(&both, &links) && filter.keeps(&one, &links));
        filter.toggle_tag(&short.id);
        assert!(filter.keeps(&both, &links) && !filter.keeps(&one, &links));
        filter.toggle_tag(&short.id);
        assert!(filter.keeps(&one, &links));
        filter.text = "bot".to_owned();
        assert!(filter.keeps(&both, &links) && !filter.keeps(&one, &links));
    }

    #[test]
    fn sorting_by_year_leaves_unknown_years_last_both_ways() {
        let database = Database::open_in_memory().unwrap();
        for (title, year) in [("Unknown", None), ("Old", Some(1969)), ("New", Some(2024))] {
            items::create(&database, MediaKind::Book, title, year).unwrap();
        }
        let items = items::all(&database).unwrap();
        let sorted = |sort| {
            let mut view: Vec<&items::Item> = items.iter().collect();
            sort_items(&mut view, sort);
            view.iter()
                .map(|item| item.title.clone())
                .collect::<Vec<_>>()
        };

        assert_eq!(sorted(Sort::Newest), ["New", "Old", "Unknown"]);
        assert_eq!(sorted(Sort::Oldest), ["Old", "New", "Unknown"]);
        assert_eq!(sorted(Sort::Shelf), ["Unknown", "Old", "New"]);
    }

    #[test]
    fn sorting_by_title_ignores_case_and_accents() {
        let database = Database::open_in_memory().unwrap();
        for title in ["zebra", "Éclair", "apple", "Dune"] {
            items::create(&database, MediaKind::Book, title, None).unwrap();
        }
        let items = items::all(&database).unwrap();
        let mut view: Vec<&items::Item> = items.iter().collect();
        sort_items(&mut view, Sort::Title);

        assert_eq!(titles_of(&view), ["apple", "Dune", "Éclair", "zebra"]);
    }

    fn titles_of(items: &[&items::Item]) -> Vec<String> {
        items.iter().map(|item| item.title.clone()).collect()
    }

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
