pub mod igdb;
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
        MediaKind::Game => Some(Source::Igdb),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Route {
    Direct,
    OwnKey,
    Relay,
}

pub fn route(
    media_kind: MediaKind,
    tmdb_token: Option<&str>,
    igdb_credentials: Option<(&str, &str)>,
) -> Route {
    match media_kind {
        MediaKind::Show | MediaKind::Book => Route::Direct,
        MediaKind::Movie if tmdb_token.is_some() => Route::OwnKey,
        MediaKind::Game if igdb_credentials.is_some() => Route::OwnKey,
        MediaKind::Movie | MediaKind::Game => Route::Relay,
    }
}

const SERVICE_NAME: &str = "the Loregalia service";

pub fn failure_message(error: &Error, source: Source, route: Route) -> String {
    let name = source.label();
    let target = match route {
        Route::Direct | Route::OwnKey => name,
        Route::Relay => SERVICE_NAME,
    };
    let message = match (error, route) {
        (Error::Unauthorized, Route::OwnKey) => format!(
            "{name} rejected your API key. Check it in Settings, or remove it to search through \
             {SERVICE_NAME}."
        ),
        (Error::Unauthorized, Route::Relay) => format!(
            "{SERVICE_NAME} could not sign in to {name}. Try again later, or add your own {name} \
             key in Settings."
        ),
        (Error::Unauthorized, Route::Direct) => {
            format!("{name} refused the search. Try again later.")
        }
        (Error::RelayUnavailable, _) => format!(
            "{SERVICE_NAME} is unavailable right now. Try again later, or add your own {name} key \
             in Settings."
        ),
        (Error::ConnectionFailed, _) => {
            format!("Could not connect to {target}. Check your internet connection and try again.")
        }
        (Error::RateLimited, _) => {
            format!("{target} is rate limiting searches. Wait a minute and try again.")
        }
        (Error::HttpStatus { code }, _) => {
            format!("{target} returned an error (HTTP {code}). Try again later.")
        }
        (Error::Json(_), _) => {
            format!("{target} sent a reply Loregalia could not read. Try again later.")
        }
        (other, _) => other.to_string(),
    };
    capitalized(&message)
}

fn capitalized(text: &str) -> String {
    let mut characters = text.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().chain(characters).collect(),
        None => String::new(),
    }
}

pub fn search_with_key(
    media_kind: MediaKind,
    query: &str,
    relay_url: &str,
    tmdb_token: Option<&str>,
    igdb_credentials: Option<(&str, &str)>,
) -> Result<Vec<Candidate>> {
    match media_kind {
        MediaKind::Show => tvmaze::search(query),
        MediaKind::Movie => match tmdb_token {
            Some(token) => tmdb::search(token, query),
            None => relay::search_movies(relay_url, query),
        },
        MediaKind::Book => open_library::search(query),
        MediaKind::Game => match igdb_credentials {
            Some((client_id, client_secret)) => {
                let token = igdb::token(client_id, client_secret)?;
                igdb::search(client_id, &token.value, query)
            }
            None => relay::search_games(relay_url, query),
        },
    }
}

pub fn details_with_key(
    source: Source,
    external_id: &str,
    relay_url: &str,
    tmdb_token: Option<&str>,
    igdb_credentials: Option<(&str, &str)>,
) -> Result<Candidate> {
    match source {
        Source::Tmdb => match tmdb_token {
            Some(token) => tmdb::details(token, external_id),
            None => relay::movie_details(relay_url, external_id),
        },
        Source::Igdb => match igdb_credentials {
            Some((client_id, client_secret)) => {
                let token = igdb::token(client_id, client_secret)?;
                igdb::details(client_id, &token.value, external_id)
            }
            None => relay::game_details(relay_url, external_id),
        },
        Source::TvMaze | Source::OpenLibrary => Err(Error::UnsupportedLookup),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Candidate, Route, Source, details_with_key, failure_message, route, search_with_key,
        source, supported, year_of,
    };
    use crate::Error;
    use crate::media::MediaKind;

    const UNREACHABLE_RELAY: &str = "http://127.0.0.1:1";

    #[test]
    fn shows_and_books_always_go_direct() {
        for kind in [MediaKind::Show, MediaKind::Book] {
            assert_eq!(
                route(kind, Some("token"), Some(("id", "secret"))),
                Route::Direct
            );
            assert_eq!(route(kind, None, None), Route::Direct);
        }
    }

    #[test]
    fn movies_and_games_use_the_relay_unless_the_user_has_a_key() {
        assert_eq!(route(MediaKind::Movie, None, None), Route::Relay);
        assert_eq!(route(MediaKind::Movie, Some("token"), None), Route::OwnKey);
        assert_eq!(route(MediaKind::Game, Some("token"), None), Route::Relay);
        assert_eq!(
            route(MediaKind::Game, None, Some(("id", "secret"))),
            Route::OwnKey
        );
    }

    #[test]
    fn a_rejected_own_key_points_to_settings() {
        let message = failure_message(&Error::Unauthorized, Source::Tmdb, Route::OwnKey);

        assert!(message.starts_with("TMDB rejected your API key."));
        assert!(message.contains("Settings"));
    }

    #[test]
    fn a_relay_that_cannot_sign_in_is_not_blamed_on_the_user() {
        let message = failure_message(&Error::Unauthorized, Source::Igdb, Route::Relay);

        assert!(message.starts_with("The Loregalia service could not sign in to IGDB."));
        assert!(!message.contains("your API key"));
    }

    #[test]
    fn no_network_names_what_could_not_be_reached() {
        assert_eq!(
            failure_message(&Error::ConnectionFailed, Source::TvMaze, Route::Direct),
            "Could not connect to TVmaze. Check your internet connection and try again."
        );
        assert_eq!(
            failure_message(&Error::ConnectionFailed, Source::Tmdb, Route::Relay),
            "Could not connect to the Loregalia service. Check your internet connection and try \
             again."
        );
    }

    #[test]
    fn a_down_relay_suggests_an_own_key() {
        let message = failure_message(&Error::RelayUnavailable, Source::Tmdb, Route::Relay);

        assert!(message.starts_with("The Loregalia service is unavailable right now."));
        assert!(message.contains("your own TMDB key"));
    }

    #[test]
    fn rate_limits_and_server_errors_say_to_try_later() {
        assert_eq!(
            failure_message(&Error::RateLimited, Source::Tmdb, Route::Relay),
            "The Loregalia service is rate limiting searches. Wait a minute and try again."
        );
        assert_eq!(
            failure_message(
                &Error::HttpStatus { code: 500 },
                Source::OpenLibrary,
                Route::Direct
            ),
            "Open Library returned an error (HTTP 500). Try again later."
        );
    }

    #[test]
    fn an_unreadable_reply_is_not_shown_as_raw_json_errors() {
        let error = Error::Json(serde_json::from_str::<u8>("{").unwrap_err());

        assert_eq!(
            failure_message(&error, Source::TvMaze, Route::Direct),
            "TVmaze sent a reply Loregalia could not read. Try again later."
        );
    }

    #[test]
    fn other_errors_keep_their_description_with_a_capital_letter() {
        assert_eq!(
            failure_message(&Error::UnsupportedLookup, Source::TvMaze, Route::Direct),
            "No metadata source is available for this media kind yet"
        );
    }

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
    fn every_media_kind_has_a_supported_source() {
        for kind in MediaKind::ALL {
            assert!(supported(kind));
        }
    }

    #[test]
    fn a_supported_kind_maps_to_its_source() {
        assert_eq!(source(MediaKind::Show), Some(Source::TvMaze));
        assert_eq!(source(MediaKind::Book), Some(Source::OpenLibrary));
        assert_eq!(source(MediaKind::Movie), Some(Source::Tmdb));
        assert_eq!(source(MediaKind::Game), Some(Source::Igdb));
    }

    #[test]
    fn details_exist_only_for_sources_that_refresh() {
        assert!(matches!(
            details_with_key(Source::TvMaze, "42", UNREACHABLE_RELAY, None, None),
            Err(Error::UnsupportedLookup)
        ));
        assert!(matches!(
            details_with_key(Source::Tmdb, "438631", UNREACHABLE_RELAY, None, None),
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

    fn tmdb_candidate() -> Candidate {
        Candidate {
            source: Source::Tmdb,
            external_id: "438631".to_owned(),
            title: "Dune".to_owned(),
            release_year: Some(2021),
            cover_url: None,
        }
    }

    #[test]
    fn a_movie_search_without_a_key_goes_to_the_relay() {
        let body = serde_json::to_string(&crate::providers::relay::SearchResponse {
            results: vec![tmdb_candidate()],
        })
        .unwrap();
        let base = crate::http::test_server::serve("200 OK", body.as_bytes());

        assert_eq!(
            search_with_key(MediaKind::Movie, "dune", &base, None, None).unwrap(),
            vec![tmdb_candidate()]
        );
    }

    #[test]
    fn a_movie_details_without_a_key_goes_to_the_relay() {
        let body = serde_json::to_string(&tmdb_candidate()).unwrap();
        let base = crate::http::test_server::serve("200 OK", body.as_bytes());

        assert_eq!(
            details_with_key(Source::Tmdb, "438631", &base, None, None).unwrap(),
            tmdb_candidate()
        );
    }

    #[test]
    fn a_game_search_goes_to_the_relay() {
        let game = Candidate {
            source: Source::Igdb,
            external_id: "11737".to_owned(),
            title: "Outer Wilds".to_owned(),
            release_year: Some(2019),
            cover_url: None,
        };
        let body = serde_json::to_string(&crate::providers::relay::SearchResponse {
            results: vec![game.clone()],
        })
        .unwrap();
        let base = crate::http::test_server::serve("200 OK", body.as_bytes());

        assert_eq!(
            search_with_key(MediaKind::Game, "outer wilds", &base, None, None).unwrap(),
            vec![game]
        );
    }

    #[test]
    fn details_with_a_key_are_limited_to_tmdb() {
        assert!(matches!(
            details_with_key(Source::TvMaze, "42", UNREACHABLE_RELAY, Some("token"), None),
            Err(Error::UnsupportedLookup)
        ));
        assert!(matches!(
            details_with_key(
                Source::OpenLibrary,
                "42",
                UNREACHABLE_RELAY,
                Some("token"),
                None
            ),
            Err(Error::UnsupportedLookup)
        ));
    }
}
