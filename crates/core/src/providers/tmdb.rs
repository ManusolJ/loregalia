use crate::details::Details;
use crate::http;
use crate::providers::{Candidate, Source, full_date, numeric, positive, year_of};
use crate::{Error, Result, text};
use serde::Deserialize;
use std::time::Duration;

const BASE_URL: &str = "https://api.themoviedb.org/3";
const IMAGE_BASE_URL: &str = "https://image.tmdb.org/t/p/w342";
const LANGUAGE: (&str, &str) = ("language", "en-US");

#[derive(Deserialize)]
struct ApiSearch {
    results: Vec<ApiMovie>,
}

#[derive(Deserialize)]
struct ApiMovie {
    id: i64,
    title: Option<String>,
    release_date: Option<String>,
    poster_path: Option<String>,
    overview: Option<String>,
    runtime: Option<i64>,
    #[serde(default)]
    genres: Vec<ApiGenre>,
    credits: Option<ApiCredits>,
}

#[derive(Deserialize)]
struct ApiGenre {
    name: Option<String>,
}

#[derive(Deserialize)]
struct ApiCredits {
    #[serde(default)]
    crew: Vec<ApiCrew>,
}

#[derive(Deserialize)]
struct ApiCrew {
    name: Option<String>,
    job: Option<String>,
}

impl ApiMovie {
    fn details(&self) -> Details {
        Details {
            synopsis: self.overview.as_deref().and_then(text::tidy),
            minutes: positive(self.runtime),
            pages: None,
            genres: self
                .genres
                .iter()
                .filter_map(|genre| genre.name.clone())
                .collect(),
            creators: self
                .credits
                .iter()
                .flat_map(|credits| &credits.crew)
                .filter(|person| person.job.as_deref() == Some("Director"))
                .filter_map(|person| person.name.clone())
                .collect(),
            release_date: full_date(self.release_date.as_deref()),
        }
    }

    fn into_candidate(self) -> Option<Candidate> {
        let title = self.title?.trim().to_owned();
        if title.is_empty() {
            return None;
        }

        Some(Candidate {
            source: Source::Tmdb,
            external_id: self.id.to_string(),
            title,
            release_year: year_of(self.release_date.as_deref()),
            cover_url: self
                .poster_path
                .map(|path| format!("{IMAGE_BASE_URL}{path}")),
            details: None,
            edition: None,
        })
    }
}

pub fn search(token: &str, query: &str) -> Result<Vec<Candidate>> {
    search_at(BASE_URL, token, query, http::TIMEOUT)
}

pub fn details(token: &str, id: &str) -> Result<Candidate> {
    details_at(BASE_URL, token, id, http::TIMEOUT)
}

fn search_at(
    base_url: &str,
    token: &str,
    query: &str,
    timeout: Duration,
) -> Result<Vec<Candidate>> {
    let body = http::get_text(
        &format!("{base_url}/search/movie"),
        &[("query", query), ("include_adult", "false"), LANGUAGE],
        Some(token),
        timeout,
    )?;
    parse_search(&body)
}

fn details_at(base_url: &str, token: &str, id: &str, timeout: Duration) -> Result<Candidate> {
    let body = http::get_text(
        &format!("{base_url}/movie/{}", numeric(id)?),
        &[LANGUAGE, ("append_to_response", "credits")],
        Some(token),
        timeout,
    )?;
    parse_details(&body)
}

pub fn parse_search(body: &str) -> Result<Vec<Candidate>> {
    let results: ApiSearch = serde_json::from_str(body)?;

    Ok(results
        .results
        .into_iter()
        .filter_map(ApiMovie::into_candidate)
        .collect())
}

pub fn parse_details(body: &str) -> Result<Candidate> {
    let movie: ApiMovie = serde_json::from_str(body)?;
    let details = movie.details();

    movie
        .into_candidate()
        .map(|candidate| Candidate {
            details: Some(details),
            ..candidate
        })
        .ok_or(Error::HttpStatus { code: 404 })
}

#[cfg(test)]
mod tests {
    use super::{Error, Source, details_at, parse_details, parse_search, search_at};
    use crate::http::test_server::{TEST_TIMEOUT, refuse, serve};

    #[test]
    fn maps_a_saved_search_response() {
        let body = include_str!("../../tests/fixtures/tmdb/search-dune.json");

        let movies = parse_search(body).unwrap();

        assert_eq!(movies[0].source, Source::Tmdb);
        assert_eq!(movies[0].external_id, "438631");
        assert_eq!(movies[0].title, "Dune");
        assert_eq!(movies[0].release_year, Some(2021));
        assert_eq!(
            movies[0].cover_url.as_deref(),
            Some("https://image.tmdb.org/t/p/w342/v1tRXZ4JtD2Iv6fjkPvT4GiwslV.jpg")
        );
    }

    #[test]
    fn maps_a_saved_details_response() {
        let body = include_str!("../../tests/fixtures/tmdb/details-dune.json");

        let movie = parse_details(body).unwrap();

        assert_eq!(movie.source, Source::Tmdb);
        assert_eq!(movie.external_id, "438631");
        assert_eq!(movie.title, "Dune");
        assert_eq!(movie.release_year, Some(2021));
        assert_eq!(
            movie.cover_url.as_deref(),
            Some("https://image.tmdb.org/t/p/w342/v1tRXZ4JtD2Iv6fjkPvT4GiwslV.jpg")
        );
    }

    #[test]
    fn a_movie_without_a_date_or_poster_maps_to_none() {
        let body =
            r#"{"results":[{"id":7,"title":"Bare","release_date":null,"poster_path":null}]}"#;

        let movies = parse_search(body).unwrap();

        assert_eq!(movies.len(), 1);
        assert_eq!(movies[0].release_year, None);
        assert_eq!(movies[0].cover_url, None);
    }

    #[test]
    fn skips_movies_without_a_title() {
        let body = r#"{"results":[{"id":1,"title":null},{"id":2,"title":"   "}]}"#;

        assert_eq!(parse_search(body).unwrap(), Vec::new());
    }

    #[test]
    fn invalid_json_is_an_error() {
        assert!(matches!(parse_search("{"), Err(Error::Json(_))));
    }

    #[test]
    fn a_rejected_token_is_unauthorized() {
        let base = serve("401 Unauthorized", b"");

        let error = search_at(&base, "bad-token", "dune", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::Unauthorized));
    }

    #[test]
    fn http_429_is_rate_limited() {
        let base = serve("429 Too Many Requests", b"");

        let error = search_at(&base, "token", "dune", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::RateLimited));
    }

    #[test]
    fn http_500_reports_its_status_code() {
        let base = serve("500 Internal Server Error", b"");

        let error = search_at(&base, "token", "dune", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::HttpStatus { code: 500 }));
    }

    #[test]
    fn a_missing_movie_is_a_404() {
        let base = serve("404 Not Found", b"");

        let error = details_at(&base, "token", "1", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::HttpStatus { code: 404 }));
    }

    #[test]
    fn a_refused_connection_is_a_connection_failure() {
        let error = search_at(&refuse(), "token", "dune", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::ConnectionFailed));
    }

    #[test]
    fn maps_saved_movie_details_with_the_director() {
        let body = include_str!("../../tests/fixtures/tmdb/details-dune-credits.json");
        let details = super::parse_details(body).unwrap().details.unwrap();

        assert_eq!(details.minutes, Some(155));
        assert_eq!(details.genres, ["Science Fiction", "Adventure"]);
        assert_eq!(details.creators, ["Denis Villeneuve"]);
        assert_eq!(details.release_date.as_deref(), Some("2021-09-15"));
        assert!(details.synopsis.unwrap().starts_with("Paul Atreides"));
    }

    #[test]
    fn unknown_runtime_and_dates_stay_empty() {
        let body = r#"{"id":1,"title":"Untitled","runtime":0,"release_date":"","overview":""}"#;

        assert_eq!(
            super::parse_details(body).unwrap().details,
            Some(super::Details::default())
        );
    }

    #[test]
    fn a_search_result_carries_no_details() {
        let body = include_str!("../../tests/fixtures/tmdb/search-dune.json");

        assert!(
            super::parse_search(body)
                .unwrap()
                .iter()
                .all(|movie| movie.details.is_none())
        );
    }
}
