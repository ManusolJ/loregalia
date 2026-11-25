use crate::dates;
use crate::details;
use crate::items::Item;
use crate::library::category_of;
use crate::providers::Source;
use crate::statuses::{Category, Status};

const JUST_OUT_DAYS: i64 = 14;
const SOON_DAYS: i64 = 14;
const END_OF_YEAR: &str = "12-32";
const YEAR_ONLY_DAY: &str = "-12-31";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum When {
    Date(String),
    Year(i32),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Upcoming {
    pub when: When,
    pub season: Option<i32>,
}

impl Upcoming {
    pub fn order(&self) -> String {
        match &self.when {
            When::Date(date) => date.clone(),
            When::Year(year) => format!("{year:04}-{END_OF_YEAR}"),
        }
    }

    pub fn label(&self, today: &str) -> String {
        let when = match &self.when {
            When::Date(date) => out(date, today),
            When::Year(year) => expected(*year),
        };

        match self.season {
            Some(number) => format!("Season {number}: {}", uncapitalized(&when)),
            None => when,
        }
    }
}

pub fn upcoming(item: &Item, statuses: &[Status], today: &str) -> Option<Upcoming> {
    let category = category_of(statuses, &item.status_id)?;

    release(item, category, today).or_else(|| next_season(item, category, today))
}

pub fn may_change(item: &Item, statuses: &[Status], today: &str) -> bool {
    let Some(details) = item.details() else {
        return false;
    };
    let category = category_of(statuses, &item.status_id);
    match item.source.as_deref().and_then(Source::from_key) {
        Some(Source::TvMaze) => details.ended != Some(true) && category != Some(Category::Dropped),
        Some(Source::Tmdb | Source::Igdb) => {
            let open = !matches!(category, Some(Category::Done | Category::Dropped));
            let unreleased = match (&item.release_date, item.release_year) {
                (Some(date), _) => date.as_str() > today,
                (None, Some(year)) => year > year_of(today),
                (None, None) => details.ended != Some(true),
            };
            open && unreleased
        }
        Some(Source::OpenLibrary) | None => false,
    }
}

pub fn release_label(item: &Item, today: &str) -> Option<String> {
    expected_year(item, today).map(expected).or_else(|| {
        item.release_date
            .as_deref()
            .and_then(|date| details::release(date, today))
    })
}

fn expected(year: i32) -> String {
    format!("Expected in {year}")
}

fn knows_only_the_year(item: &Item, date: &str) -> bool {
    item.source.as_deref() == Some(Source::Igdb.key()) && date.ends_with(YEAR_ONLY_DAY)
}

fn expected_year(item: &Item, today: &str) -> Option<i32> {
    match &item.release_date {
        Some(date) => {
            (knows_only_the_year(item, date) && date.as_str() >= today).then(|| year_of(date))
        }
        None => item.release_year.filter(|year| *year > year_of(today)),
    }
}

fn release(item: &Item, category: Category, today: &str) -> Option<Upcoming> {
    if matches!(category, Category::Done | Category::Dropped) {
        return None;
    }
    if let Some(year) = expected_year(item, today) {
        return Some(Upcoming {
            when: When::Year(year),
            season: None,
        });
    }
    let when = match &item.release_date {
        Some(date) if knows_only_the_year(item, date) => return None,
        Some(date) if date.as_str() > today => When::Date(date.clone()),
        Some(date) if category == Category::Planned && just_out(date, today) => {
            When::Date(date.clone())
        }
        _ => return None,
    };

    Some(Upcoming { when, season: None })
}

fn next_season(item: &Item, category: Category, today: &str) -> Option<Upcoming> {
    if category == Category::Dropped {
        return None;
    }
    let season = item.details()?.next_season.as_ref()?;
    if season.date.as_str() <= today && !just_out(&season.date, today) {
        return None;
    }

    Some(Upcoming {
        when: When::Date(season.date.clone()),
        season: Some(season.number),
    })
}

fn just_out(date: &str, today: &str) -> bool {
    dates::days_between(date, today).is_some_and(|days| (0..=JUST_OUT_DAYS).contains(&days))
}

fn out(date: &str, today: &str) -> String {
    let shown = dates::short(date).unwrap_or_else(|| date.to_owned());
    match dates::days_between(today, date) {
        Some(0) => "Out today".to_owned(),
        Some(1) => "Out tomorrow".to_owned(),
        Some(days) if days < 0 => format!("Out since {shown}"),
        Some(days) if days <= SOON_DAYS => format!("Out in {days} days"),
        _ => format!("Out {shown}"),
    }
}

fn year_of(date: &str) -> i32 {
    date.get(..4)
        .and_then(|year| year.parse().ok())
        .unwrap_or_default()
}

fn uncapitalized(text: &str) -> String {
    let mut letters = text.chars();
    letters.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(letters).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::{Upcoming, When, may_change, upcoming};
    use crate::details::{Details, NextSeason};
    use crate::media::MediaKind;
    use crate::providers::Source;
    use crate::{Database, items, statuses};

    const TODAY: &str = "2026-11-25";

    fn stored(
        kind: MediaKind,
        source: Source,
        year: Option<i32>,
        details: Details,
        status: &str,
    ) -> (items::Item, Vec<statuses::Status>) {
        let database = Database::open_in_memory().unwrap();
        let lookup = items::Lookup {
            source,
            external_id: "1".to_owned(),
            cover_url: None,
        };
        let item = items::create_from_lookup(&database, kind, "Title", year, &lookup).unwrap();
        let data = items::SourceData {
            title: "Title".to_owned(),
            release_year: year,
            cover_url: None,
            details: Some(details),
        };
        items::apply_refresh(&database, &item.id, data, "2026-11-25T00:00:00.000Z").unwrap();
        let status = format!("{}-{status}", kind.key());
        items::set_status(&database, &item.id, &status).unwrap();
        (
            items::get(&database, &item.id).unwrap().unwrap(),
            statuses::all(&database).unwrap(),
        )
    }

    fn movie(date: Option<&str>, year: Option<i32>, status: &str) -> Option<String> {
        let details = Details {
            release_date: date.map(str::to_owned),
            ..Details::default()
        };
        let (item, statuses) = stored(MediaKind::Movie, Source::Tmdb, year, details, status);
        upcoming(&item, &statuses, TODAY).map(|found| found.label(TODAY))
    }

    fn show(date: &str, status: &str) -> Option<String> {
        let details = Details {
            release_date: Some("2023-05-05".to_owned()),
            next_season: Some(NextSeason {
                number: 4,
                date: date.to_owned(),
            }),
            ended: Some(false),
            ..Details::default()
        };
        let (item, statuses) = stored(MediaKind::Show, Source::TvMaze, Some(2023), details, status);
        upcoming(&item, &statuses, TODAY).map(|found| found.label(TODAY))
    }

    #[test]
    fn dates_read_naturally() {
        let label = |date| movie(Some(date), None, "planned");

        assert_eq!(label("2026-11-26").as_deref(), Some("Out tomorrow"));
        assert_eq!(label("2026-11-28").as_deref(), Some("Out in 3 days"));
        assert_eq!(label("2026-12-09").as_deref(), Some("Out in 14 days"));
        assert_eq!(label("2026-12-10").as_deref(), Some("Out 10 Dec 2026"));
        assert_eq!(label("2027-07-09").as_deref(), Some("Out 9 Jul 2027"));
        assert_eq!(label(TODAY).as_deref(), Some("Out today"));
        assert_eq!(
            label("2026-11-20").as_deref(),
            Some("Out since 20 Nov 2026")
        );
        assert_eq!(
            label("2026-11-11").as_deref(),
            Some("Out since 11 Nov 2026")
        );
        assert_eq!(label("2026-11-10"), None);
    }

    #[test]
    fn a_future_year_is_enough() {
        assert_eq!(
            movie(None, Some(2027), "planned").as_deref(),
            Some("Expected in 2027")
        );
        assert_eq!(movie(None, Some(2026), "planned"), None);
        assert_eq!(movie(None, None, "planned"), None);
    }

    #[test]
    fn finished_and_dropped_items_are_not_awaited() {
        for status in ["done", "dropped"] {
            assert_eq!(movie(Some("2027-07-09"), None, status), None);
        }
        assert!(movie(Some("2027-07-09"), None, "paused").is_some());
    }

    #[test]
    fn just_out_is_only_news_for_what_was_planned() {
        assert_eq!(movie(Some("2026-11-20"), None, "active"), None);
        assert_eq!(movie(Some("2026-11-20"), None, "paused"), None);
    }

    #[test]
    fn a_new_season_is_awaited_even_after_finishing_the_show() {
        assert_eq!(
            show("2027-07-09", "done").as_deref(),
            Some("Season 4: out 9 Jul 2027")
        );
        assert_eq!(
            show("2026-11-20", "done").as_deref(),
            Some("Season 4: out since 20 Nov 2026")
        );
        assert_eq!(show("2026-10-01", "done"), None);
        assert_eq!(show("2027-07-09", "dropped"), None);
    }

    #[test]
    fn a_year_alone_comes_after_every_date_in_it() {
        let order = |when| Upcoming { when, season: None }.order();

        assert!(order(When::Date("2027-12-31".to_owned())) < order(When::Year(2027)));
        assert!(order(When::Year(2027)) < order(When::Date("2028-01-01".to_owned())));
    }

    fn changes(
        source: Source,
        date: Option<&str>,
        year: Option<i32>,
        ended: Option<bool>,
        status: &str,
    ) -> bool {
        let kind = match source {
            Source::TvMaze => MediaKind::Show,
            Source::Tmdb => MediaKind::Movie,
            Source::Igdb => MediaKind::Game,
            Source::OpenLibrary => MediaKind::Book,
        };
        let details = Details {
            release_date: date.map(str::to_owned),
            ended,
            ..Details::default()
        };
        let (item, statuses) = stored(kind, source, year, details, status);
        may_change(&item, &statuses, TODAY)
    }

    #[test]
    fn only_what_can_still_change_is_checked_again() {
        let future = Some("2027-07-09");
        let past = Some("2020-01-01");

        assert!(changes(Source::Tmdb, future, Some(2027), None, "planned"));
        assert!(changes(Source::Igdb, None, Some(2027), None, "planned"));
        assert!(changes(Source::Igdb, None, None, None, "planned"));
        assert!(!changes(Source::Igdb, None, None, Some(true), "planned"));
        assert!(!changes(Source::Tmdb, past, Some(2020), None, "planned"));
        assert!(!changes(Source::Tmdb, future, Some(2027), None, "dropped"));
        assert!(changes(
            Source::TvMaze,
            past,
            Some(2020),
            Some(false),
            "done"
        ));
        assert!(!changes(
            Source::TvMaze,
            past,
            Some(2020),
            Some(true),
            "planned"
        ));
        assert!(!changes(
            Source::TvMaze,
            past,
            Some(2020),
            Some(false),
            "dropped"
        ));
        assert!(!changes(
            Source::OpenLibrary,
            None,
            Some(2027),
            None,
            "planned"
        ));
    }

    fn game(date: &str, year: i32) -> Option<String> {
        let details = Details {
            release_date: Some(date.to_owned()),
            ..Details::default()
        };
        let (item, statuses) = stored(
            MediaKind::Game,
            Source::Igdb,
            Some(year),
            details,
            "planned",
        );
        upcoming(&item, &statuses, TODAY).map(|found| found.label(TODAY))
    }

    #[test]
    fn the_last_day_of_the_year_is_how_igdb_says_only_the_year_is_known() {
        assert_eq!(
            game("2028-12-31", 2028).as_deref(),
            Some("Expected in 2028")
        );
        assert_eq!(game("2028-12-30", 2028).as_deref(), Some("Out 30 Dec 2028"));
        assert_eq!(
            movie(Some("2028-12-31"), Some(2028), "planned").as_deref(),
            Some("Out 31 Dec 2028")
        );
    }

    #[test]
    fn a_year_that_passed_without_a_date_is_not_just_out() {
        let details = Details {
            release_date: Some("2026-12-31".to_owned()),
            ..Details::default()
        };
        let (item, statuses) = stored(
            MediaKind::Game,
            Source::Igdb,
            Some(2026),
            details,
            "planned",
        );

        assert_eq!(upcoming(&item, &statuses, "2027-01-03"), None);
    }
}
