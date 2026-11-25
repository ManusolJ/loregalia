use crate::details::clock;
use crate::items::Item;
use crate::library::{category_of, comparable};
use crate::list_items::Membership;
use crate::lists::List;
use crate::media::MediaKind;
use crate::statuses::{Category, Status};
use crate::{dates, details};
use std::collections::HashMap;

pub const SHOWN: usize = 3;
const MOST: usize = 150;
const MOST_GENRES: usize = 10;
const MOST_REASONS: usize = 3;
const MOVIE_GRACE: u32 = 10;

const NEXT_IN_LIST: i32 = 30;
const IN_LIST: i32 = 15;
const ON_HOLD: i32 = 20;
const WAITING: i32 = 15;
const FITS: i32 = 20;
const SAME_KIND: i32 = 25;
const CHANCE: i32 = 40;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Ask {
    pub kind: Option<MediaKind>,
    pub minutes: Option<u32>,
    pub genre: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Suggestion {
    pub item_id: String,
    pub reasons: Vec<String>,
}

pub struct Shelves<'a> {
    pub items: &'a [Item],
    pub statuses: &'a [Status],
    pub lists: &'a [List],
    pub memberships: &'a [Membership],
}

enum Place<'a> {
    Next(&'a str),
    Within(&'a str),
}

struct Choice<'a> {
    item: &'a Item,
    score: i32,
    reasons: Vec<String>,
}

pub fn waiting<'a>(shelves: &Shelves<'a>, today: &str) -> Vec<&'a Item> {
    shelves
        .items
        .iter()
        .filter(|item| is_waiting(shelves.statuses, item))
        .filter(|item| {
            item.release_date
                .as_deref()
                .is_none_or(|released| released <= today)
        })
        .collect()
}

pub fn genres(shelves: &Shelves<'_>, kind: Option<MediaKind>, today: &str) -> Vec<String> {
    let mut counts: HashMap<String, (usize, &str)> = HashMap::new();
    for item in waiting(shelves, today) {
        if kind.is_some_and(|kind| item.media_kind != kind) {
            continue;
        }
        for genre in item.details().into_iter().flat_map(|found| &found.genres) {
            counts.entry(comparable(genre)).or_insert((0, genre)).0 += 1;
        }
    }

    let mut ranked: Vec<(usize, &str)> = counts.into_values().collect();
    ranked.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(right.1)));
    let mut names: Vec<String> = ranked
        .into_iter()
        .take(MOST_GENRES)
        .map(|(_, name)| name.to_owned())
        .collect();
    names.sort();
    names
}

pub fn same_genre(genres: &[String], wanted: &str) -> Option<String> {
    let wanted = comparable(wanted);

    genres
        .iter()
        .find(|genre| comparable(genre) == wanted)
        .cloned()
}

pub fn suggest(shelves: &Shelves<'_>, ask: &Ask, today: &str, seed: u64) -> Vec<Suggestion> {
    let waiting = waiting(shelves, today);
    let places = places(shelves);
    let total = waiting.len();
    let mut choices: Vec<Choice<'_>> = waiting
        .into_iter()
        .enumerate()
        .filter(|(_, item)| matches(item, ask))
        .filter_map(|(index, item)| {
            let fit = fit(item, ask.minutes)?;
            Some(weigh(
                shelves,
                &places,
                ask,
                item,
                fit,
                (index, total),
                seed,
            ))
        })
        .collect();

    choices.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.item.id.cmp(&right.item.id))
    });
    choices.truncate(MOST);
    if ask.kind.is_none() {
        choices = varied(choices);
    }

    choices
        .into_iter()
        .map(|choice| Suggestion {
            item_id: choice.item.id.clone(),
            reasons: choice.reasons,
        })
        .collect()
}

fn is_waiting(statuses: &[Status], item: &Item) -> bool {
    matches!(
        category_of(statuses, &item.status_id),
        Some(Category::Planned | Category::Paused)
    )
}

fn matches(item: &Item, ask: &Ask) -> bool {
    let kind = ask.kind.is_none_or(|kind| item.media_kind == kind);
    let genre = ask.genre.as_deref().is_none_or(|wanted| {
        let wanted = comparable(wanted);
        item.details()
            .is_some_and(|found| found.genres.iter().any(|genre| comparable(genre) == wanted))
    });

    kind && genre
}

fn fit(item: &Item, available: Option<u32>) -> Option<(i32, Option<String>)> {
    let known = item.details();
    let length = known.and_then(|found| found.length(item.media_kind));
    let Some(available) = available else {
        return Some((0, length));
    };
    let minutes = known.and_then(|found| found.minutes);

    match (item.media_kind, minutes) {
        (MediaKind::Book | MediaKind::Game, _) => {
            let reason = match length {
                Some(length) => format!("{length}, stop whenever you like"),
                None => "Stop whenever you like".to_owned(),
            };
            Some((FITS / 4, Some(reason)))
        }
        (_, None) => Some((-FITS, Some("Length unknown".to_owned()))),
        (MediaKind::Movie, Some(minutes)) if minutes <= available + MOVIE_GRACE => {
            let closeness = minutes.min(available) as i32 * FITS / available.max(1) as i32;
            let reason = format!("{}, fits your {}", clock(minutes), clock(available));
            Some((closeness, Some(reason)))
        }
        (MediaKind::Show, Some(minutes)) if minutes <= available => {
            let reason = match available / minutes.max(1) {
                1 => "Time for one episode".to_owned(),
                episodes => format!("Time for {episodes} episodes"),
            };
            Some((FITS / 2, Some(reason)))
        }
        _ => None,
    }
}

fn places<'a>(shelves: &Shelves<'a>) -> HashMap<&'a str, Place<'a>> {
    let by_id: HashMap<&str, &Item> = shelves
        .items
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect();
    let mut places = HashMap::new();
    for list in shelves.lists {
        let mut members: Vec<&Membership> = shelves
            .memberships
            .iter()
            .filter(|member| member.list_id == list.id)
            .filter(|member| {
                by_id
                    .get(member.item_id.as_str())
                    .is_some_and(|item| is_waiting(shelves.statuses, item))
            })
            .collect();
        members.sort_by_key(|member| member.position);
        for (index, member) in members.into_iter().enumerate() {
            let id = member.item_id.as_str();
            if index == 0 {
                places.insert(id, Place::Next(&list.name));
            } else {
                places.entry(id).or_insert(Place::Within(&list.name));
            }
        }
    }
    places
}

fn weigh<'a>(
    shelves: &Shelves<'_>,
    places: &HashMap<&str, Place<'_>>,
    ask: &Ask,
    item: &'a Item,
    (fit_score, fit_reason): (i32, Option<String>),
    (index, total): (usize, usize),
    seed: u64,
) -> Choice<'a> {
    let on_hold = category_of(shelves.statuses, &item.status_id) == Some(Category::Paused);
    let age = WAITING - (index * WAITING as usize / total.max(1)) as i32;
    let mut score = fit_score + age + chance(seed, &item.id);
    let mut reasons = Vec::new();

    reasons.extend(fit_reason);
    match places.get(item.id.as_str()) {
        Some(Place::Next(name)) => {
            score += NEXT_IN_LIST;
            reasons.push(format!("Next in {name}"));
        }
        Some(Place::Within(name)) => {
            score += IN_LIST;
            reasons.push(format!("In {name}"));
        }
        None => {}
    }
    if on_hold {
        score += ON_HOLD;
        reasons.push("You put it on hold".to_owned());
    }
    reasons.extend(ask.genre.clone());
    if let Some(added) = item.added_on.as_deref().and_then(dates::short) {
        reasons.push(format!("Waiting since {added}"));
    }
    reasons.truncate(MOST_REASONS);

    Choice {
        item,
        score,
        reasons,
    }
}

fn varied(mut choices: Vec<Choice<'_>>) -> Vec<Choice<'_>> {
    let mut ordered: Vec<Choice<'_>> = Vec::with_capacity(choices.len());
    while !choices.is_empty() {
        let start = ordered.len() - ordered.len() % SHOWN;
        let worth = |choice: &Choice<'_>| {
            let same = ordered[start..]
                .iter()
                .filter(|chosen| chosen.item.media_kind == choice.item.media_kind)
                .count();
            choice.score - SAME_KIND * same as i32
        };
        let best = choices
            .iter()
            .enumerate()
            .rev()
            .max_by_key(|(_, choice)| worth(choice))
            .map_or(0, |(index, _)| index);
        ordered.push(choices.remove(best));
    }
    ordered
}

fn chance(seed: u64, id: &str) -> i32 {
    let hash = id.bytes().fold(seed ^ 0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    });
    let mixed = (hash ^ (hash >> 33)).wrapping_mul(0xff51_afd7_ed55_8ccd);

    ((mixed >> 40) % CHANCE as u64) as i32
}

pub fn start_status(statuses: &[Status], kind: MediaKind) -> Option<&Status> {
    statuses
        .iter()
        .find(|status| status.media_kind == kind && status.category == Category::Active)
}

pub fn time_label(minutes: Option<u32>) -> String {
    minutes.map_or_else(|| "Any".to_owned(), details::clock)
}

#[cfg(test)]
mod tests {
    use super::{Ask, SHOWN, Shelves, Suggestion, genres, start_status, suggest, time_label};
    use crate::details::Details;
    use crate::media::MediaKind;
    use crate::providers::Source;
    use crate::{Database, items, list_items, lists, statuses};
    use std::collections::HashSet;

    const TODAY: &str = "2026-11-25";

    struct Fixture {
        database: Database,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                database: Database::open_in_memory().unwrap(),
            }
        }

        fn add(&self, kind: MediaKind, title: &str, details: Details) -> String {
            let source = Source::TvMaze;
            let item = items::create_from_lookup(
                &self.database,
                kind,
                title,
                Some(2020),
                &items::Lookup {
                    source,
                    external_id: title.to_owned(),
                    cover_url: None,
                },
            )
            .unwrap();
            let data = items::SourceData {
                title: title.to_owned(),
                release_year: Some(2020),
                cover_url: None,
                details: Some(details),
            };
            items::apply_refresh(&self.database, &item.id, data, "2026-11-25T00:00:00.000Z")
                .unwrap();
            item.id
        }

        fn timed(&self, kind: MediaKind, title: &str, minutes: u32) -> String {
            self.add(
                kind,
                title,
                Details {
                    minutes: Some(minutes),
                    ..Details::default()
                },
            )
        }

        fn ask(&self, ask: &Ask, seed: u64) -> Vec<Suggestion> {
            let items = items::all(&self.database).unwrap();
            let statuses = statuses::all(&self.database).unwrap();
            let lists = lists::all(&self.database).unwrap();
            let memberships = list_items::memberships(&self.database).unwrap();
            suggest(
                &Shelves {
                    items: &items,
                    statuses: &statuses,
                    lists: &lists,
                    memberships: &memberships,
                },
                ask,
                TODAY,
                seed,
            )
        }

        fn ids(&self, ask: &Ask) -> HashSet<String> {
            self.ask(ask, 1)
                .into_iter()
                .map(|suggestion| suggestion.item_id)
                .collect()
        }

        fn reasons(&self, ask: &Ask, id: &str) -> Vec<String> {
            self.ask(ask, 1)
                .into_iter()
                .find(|suggestion| suggestion.item_id == id)
                .unwrap()
                .reasons
        }
    }

    fn within(minutes: u32) -> Ask {
        Ask {
            minutes: Some(minutes),
            ..Ask::default()
        }
    }

    #[test]
    fn only_what_is_waiting_and_released_is_suggested() {
        let fixture = Fixture::new();
        let planned = fixture.timed(MediaKind::Movie, "Planned", 90);
        let paused = fixture.timed(MediaKind::Movie, "Paused", 90);
        for (title, status) in [
            ("Active", "movie-active"),
            ("Done", "movie-done"),
            ("Dropped", "movie-dropped"),
        ] {
            let id = fixture.timed(MediaKind::Movie, title, 90);
            items::set_status(&fixture.database, &id, status).unwrap();
        }
        items::set_status(&fixture.database, &paused, "movie-paused").unwrap();
        fixture.add(
            MediaKind::Movie,
            "Not out yet",
            Details {
                release_date: Some("2027-01-01".to_owned()),
                ..Details::default()
            },
        );
        let out_today = fixture.add(
            MediaKind::Movie,
            "Out today",
            Details {
                release_date: Some(TODAY.to_owned()),
                ..Details::default()
            },
        );

        assert_eq!(
            fixture.ids(&Ask::default()),
            HashSet::from([planned, paused, out_today])
        );
    }

    #[test]
    fn a_movie_must_fit_the_time_with_a_little_grace() {
        let fixture = Fixture::new();
        let short = fixture.timed(MediaKind::Movie, "Short", 95);
        let near = fixture.timed(MediaKind::Movie, "Near", 128);
        fixture.timed(MediaKind::Movie, "Long", 180);

        assert_eq!(
            fixture.ids(&within(120)),
            HashSet::from([short.clone(), near])
        );
        assert_eq!(
            fixture.reasons(&within(120), &short)[0],
            "1 h 35 min, fits your 2 h"
        );
    }

    #[test]
    fn a_show_needs_room_for_an_episode() {
        let fixture = Fixture::new();
        let show = fixture.timed(MediaKind::Show, "Show", 50);

        assert!(fixture.ids(&within(30)).is_empty());
        assert_eq!(
            fixture.reasons(&within(60), &show)[0],
            "Time for one episode"
        );
        assert_eq!(
            fixture.reasons(&within(180), &show)[0],
            "Time for 3 episodes"
        );
    }

    #[test]
    fn books_and_games_fit_any_evening() {
        let fixture = Fixture::new();
        let game = fixture.timed(MediaKind::Game, "Game", 1_310);
        let book = fixture.add(
            MediaKind::Book,
            "Book",
            Details {
                pages: Some(272),
                ..Details::default()
            },
        );

        assert_eq!(
            fixture.reasons(&within(30), &game)[0],
            "About 22 h, stop whenever you like"
        );
        assert_eq!(
            fixture.reasons(&within(30), &book)[0],
            "272 pages, stop whenever you like"
        );
    }

    #[test]
    fn an_unknown_length_comes_after_what_is_known_to_fit() {
        let fixture = Fixture::new();
        let unknown = fixture.add(MediaKind::Movie, "Unknown", Details::default());
        let known = fixture.timed(MediaKind::Movie, "Known", 115);

        for seed in 0..40 {
            let order = fixture.ask(&within(120), seed);
            assert_eq!(order.len(), 2);
            if order[0].item_id == unknown {
                continue;
            }
            assert_eq!(order[0].item_id, known);
        }
        let first = (0..40)
            .filter(|seed| fixture.ask(&within(120), *seed)[0].item_id == known)
            .count();
        assert!(first >= 36, "{first}");
        assert_eq!(fixture.reasons(&within(120), &unknown)[0], "Length unknown");
    }

    #[test]
    fn kind_and_genre_narrow_it_down_whatever_their_spelling() {
        let fixture = Fixture::new();
        let genre = |names: &[&str]| Details {
            genres: names.iter().map(|name| (*name).to_owned()).collect(),
            ..Details::default()
        };
        let show = fixture.add(MediaKind::Show, "Show", genre(&["Science-Fiction"]));
        let movie = fixture.add(MediaKind::Movie, "Movie", genre(&["Science Fiction"]));
        fixture.add(MediaKind::Movie, "Other", genre(&["Drama"]));
        let wanted = Ask {
            genre: Some("science fiction".to_owned()),
            ..Ask::default()
        };

        assert_eq!(fixture.ids(&wanted), HashSet::from([show, movie.clone()]));
        let movies = Ask {
            kind: Some(MediaKind::Movie),
            ..wanted
        };
        assert_eq!(fixture.ids(&movies), HashSet::from([movie]));
    }

    #[test]
    fn genres_are_merged_counted_and_limited_to_the_kind() {
        let fixture = Fixture::new();
        for (kind, title, name) in [
            (MediaKind::Show, "A", "Science-Fiction"),
            (MediaKind::Movie, "B", "Science Fiction"),
            (MediaKind::Movie, "C", "Drama"),
        ] {
            fixture.add(
                kind,
                title,
                Details {
                    genres: vec![name.to_owned()],
                    ..Details::default()
                },
            );
        }
        let items = items::all(&fixture.database).unwrap();
        let statuses = statuses::all(&fixture.database).unwrap();
        let shelves = Shelves {
            items: &items,
            statuses: &statuses,
            lists: &[],
            memberships: &[],
        };

        assert_eq!(genres(&shelves, None, TODAY), ["Drama", "Science-Fiction"]);
        assert_eq!(
            genres(&shelves, Some(MediaKind::Movie), TODAY),
            ["Drama", "Science Fiction"]
        );
    }

    #[test]
    fn a_genre_is_found_again_under_another_spelling() {
        let genres = ["Drama".to_owned(), "Science Fiction".to_owned()];

        assert_eq!(
            super::same_genre(&genres, "Science-Fiction").as_deref(),
            Some("Science Fiction")
        );
        assert_eq!(super::same_genre(&genres, "Comedy"), None);
    }

    #[test]
    fn the_front_of_a_list_skips_what_is_finished() {
        let fixture = Fixture::new();
        let done = fixture.timed(MediaKind::Movie, "Done", 90);
        let next = fixture.timed(MediaKind::Movie, "Next", 90);
        let later = fixture.timed(MediaKind::Movie, "Later", 90);
        items::set_status(&fixture.database, &done, "movie-done").unwrap();
        let list = lists::create(&fixture.database, "Queue").unwrap();
        for id in [&done, &next, &later] {
            list_items::add(&fixture.database, &list.id, id).unwrap();
        }

        assert!(
            fixture
                .reasons(&Ask::default(), &next)
                .contains(&"Next in Queue".to_owned())
        );
        assert!(
            fixture
                .reasons(&Ask::default(), &later)
                .contains(&"In Queue".to_owned())
        );
    }

    #[test]
    fn on_hold_and_waiting_are_explained_with_three_reasons_at_most() {
        let fixture = Fixture::new();
        let id = fixture.timed(MediaKind::Movie, "Paused", 90);
        items::set_status(&fixture.database, &id, "movie-paused").unwrap();
        let list = lists::create(&fixture.database, "Queue").unwrap();
        list_items::add(&fixture.database, &list.id, &id).unwrap();

        let reasons = fixture.reasons(&within(120), &id);
        assert_eq!(reasons.len(), 3);
        assert!(reasons.contains(&"You put it on hold".to_owned()));

        let plain = fixture.timed(MediaKind::Movie, "Plain", 90);
        let reasons = fixture.reasons(&Ask::default(), &plain);
        assert!(reasons[1].starts_with("Waiting since "), "{reasons:?}");
    }

    #[test]
    fn three_suggestions_mix_kinds_when_they_can() {
        let fixture = Fixture::new();
        for number in 0..9 {
            fixture.timed(MediaKind::Movie, &format!("Movie {number}"), 100);
        }
        fixture.timed(MediaKind::Game, "Game", 600);
        fixture.timed(MediaKind::Show, "Show", 40);
        let items = items::all(&fixture.database).unwrap();

        let mixed = (0..30)
            .filter(|seed| {
                let kinds: HashSet<MediaKind> = fixture
                    .ask(&Ask::default(), *seed)
                    .iter()
                    .take(SHOWN)
                    .filter_map(|suggestion| {
                        items
                            .iter()
                            .find(|item| item.id == suggestion.item_id)
                            .map(|item| item.media_kind)
                    })
                    .collect();
                kinds.len() >= 2
            })
            .count();
        assert!(mixed >= 27, "{mixed}");
    }

    #[test]
    fn the_same_seed_repeats_and_others_change_the_first_choice() {
        let fixture = Fixture::new();
        for number in 0..12 {
            fixture.timed(MediaKind::Movie, &format!("Movie {number}"), 100);
        }
        let first = |seed| fixture.ask(&Ask::default(), seed)[0].item_id.clone();

        assert_eq!(first(7), first(7));
        let different: HashSet<String> = (0..30).map(first).collect();
        assert!(different.len() >= 5, "{}", different.len());
    }

    #[test]
    fn every_fitting_item_appears_exactly_once() {
        let fixture = Fixture::new();
        for number in 0..20 {
            let kind = MediaKind::ALL[number % 4];
            fixture.timed(kind, &format!("Item {number}"), 30);
        }
        let order = fixture.ask(&within(60), 3);
        let unique: HashSet<&str> = order
            .iter()
            .map(|suggestion| suggestion.item_id.as_str())
            .collect();

        assert_eq!(order.len(), 20);
        assert_eq!(unique.len(), 20);
    }

    #[test]
    fn nothing_waiting_is_an_empty_answer() {
        assert!(Fixture::new().ask(&Ask::default(), 1).is_empty());
    }

    #[test]
    fn starting_uses_the_status_of_its_kind() {
        let database = Database::open_in_memory().unwrap();
        let statuses = statuses::all(&database).unwrap();

        for kind in MediaKind::ALL {
            let status = start_status(&statuses, kind).unwrap();
            assert_eq!(status.id, format!("{}-active", kind.key()));
        }
    }

    #[test]
    fn times_read_naturally() {
        assert_eq!(time_label(None), "Any");
        assert_eq!(time_label(Some(30)), "30 min");
        assert_eq!(time_label(Some(120)), "2 h");
    }
}
