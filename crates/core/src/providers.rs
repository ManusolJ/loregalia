pub mod open_library;
pub mod relay;
pub mod tmdb;
pub mod tvmaze;

use crate::media::MediaKind;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    TvMaze,
    OpenLibrary,
    Tmdb,
    Igdb,
}

impl Source {
    pub const ALL: [Self; 4] = [Self::TvMaze, Self::OpenLibrary, Self::Tmdb, Self::Igdb];

    pub fn key(self) -> &'static str {
        match self {
            Self::TvMaze => "tvmaze",
            Self::OpenLibrary => "openlibrary",
            Self::Tmdb => "tmdb",
            Self::Igdb => "igdb",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::TvMaze => "TVmaze",
            Self::OpenLibrary => "Open Library",
            Self::Tmdb => "TMDB",
            Self::Igdb => "IGDB",
        }
    }

    pub fn from_key(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|source| source.key() == value)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Candidate {
    pub source: Source,
    pub external_id: String,
    pub title: String,
    pub release_year: Option<i32>,
    pub cover_url: Option<String>,
}

pub(crate) fn year_of(date: Option<&str>) -> Option<i32> {
    date?.get(..4)?.parse().ok()
}

pub fn supported(media_kind: MediaKind) -> bool {
    source(media_kind).is_some()
}

pub fn source(media_kind: MediaKind) -> Option<Source> {
    match media_kind {
        MediaKind::Show => Some(Source::TvMaze),
        MediaKind::Movie => Some(Source::Tmdb),
        MediaKind::Book => Some(Source::OpenLibrary),
        MediaKind::Game => None,
    }
}

pub fn search(media_kind: MediaKind, query: &str, relay_url: &str) -> Result<Vec<Candidate>> {
    match media_kind {
        MediaKind::Show => tvmaze::search(query),
        MediaKind::Movie => relay::search_movies(relay_url, query),
        MediaKind::Book => open_library::search(query),
        MediaKind::Game => Err(Error::UnsupportedLookup),
    }
}

pub fn details(source: Source, external_id: &str, relay_url: &str) -> Result<Candidate> {
    match source {
        Source::Tmdb => relay::movie_details(relay_url, external_id),
        Source::TvMaze | Source::OpenLibrary | Source::Igdb => Err(Error::UnsupportedLookup),
    }
}

#[cfg(test)]
mod tests {
    use super::{Source, details, search, source, supported, year_of};
    use crate::Error;
    use crate::media::MediaKind;

    const UNREACHABLE_RELAY: &str = "http://127.0.0.1:1";

    #[test]
    fn every_source_round_trips_through_its_key() {
        for source in Source::ALL {
            assert_eq!(Source::from_key(source.key()), Some(source));
        }
    }

    #[test]
    fn source_keys_match_their_json_values() {
        for source in Source::ALL {
            assert_eq!(
                serde_json::to_value(source).unwrap(),
                serde_json::Value::String(source.key().to_owned())
            );
        }
    }

    #[test]
    fn only_sources_that_exist_are_supported() {
        assert!(supported(MediaKind::Show));
        assert!(supported(MediaKind::Book));
        assert!(supported(MediaKind::Movie));
        assert!(!supported(MediaKind::Game));
    }

    #[test]
    fn a_supported_kind_maps_to_its_source() {
        assert_eq!(source(MediaKind::Show), Some(Source::TvMaze));
        assert_eq!(source(MediaKind::Book), Some(Source::OpenLibrary));
        assert_eq!(source(MediaKind::Movie), Some(Source::Tmdb));
        assert_eq!(source(MediaKind::Game), None);
    }

    #[test]
    fn a_kind_without_a_source_fails_before_any_request() {
        assert!(matches!(
            search(MediaKind::Game, "outer wilds", UNREACHABLE_RELAY),
            Err(Error::UnsupportedLookup)
        ));
    }

    #[test]
    fn details_exist_only_for_sources_that_refresh() {
        assert!(matches!(
            details(Source::TvMaze, "42", UNREACHABLE_RELAY),
            Err(Error::UnsupportedLookup)
        ));
        assert!(matches!(
            details(Source::Tmdb, "438631", UNREACHABLE_RELAY),
            Err(Error::ConnectionFailed)
        ));
    }

    #[test]
    fn the_year_is_the_first_four_digits_of_a_date() {
        assert_eq!(year_of(Some("2021-10-22")), Some(2021));
        assert_eq!(year_of(Some("2021")), Some(2021));
        assert_eq!(year_of(Some("soon")), None);
        assert_eq!(year_of(Some("")), None);
        assert_eq!(year_of(None), None);
    }
}
