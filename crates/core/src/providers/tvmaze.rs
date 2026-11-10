use crate::Result;
use crate::http;
use crate::providers::{Candidate, Source, year_of};
use serde::Deserialize;
use std::time::Duration;

const BASE_URL: &str = "https://api.tvmaze.com";
const SEARCH_PATH: &str = "/search/shows";

#[derive(Deserialize)]
struct ApiSearchResult {
    show: ApiShow,
}

#[derive(Deserialize)]
struct ApiShow {
    id: i64,
    name: String,
    premiered: Option<String>,
    image: Option<ApiImage>,
}

#[derive(Deserialize)]
struct ApiImage {
    medium: Option<String>,
    original: Option<String>,
}

impl ApiShow {
    fn into_candidate(self) -> Candidate {
        Candidate {
            source: Source::TvMaze,
            external_id: self.id.to_string(),
            title: self.name,
            release_year: year_of(self.premiered.as_deref()),
            cover_url: self.image.and_then(|image| image.medium.or(image.original)),
        }
    }
}

pub fn search(query: &str) -> Result<Vec<Candidate>> {
    search_at(BASE_URL, query, http::TIMEOUT)
}

pub fn search_json(query: &str) -> Result<String> {
    search_json_at(BASE_URL, query, http::TIMEOUT)
}

fn search_at(base_url: &str, query: &str, timeout: Duration) -> Result<Vec<Candidate>> {
    let body = search_json_at(base_url, query, timeout)?;
    parse_search(&body)
}

fn search_json_at(base_url: &str, query: &str, timeout: Duration) -> Result<String> {
    http::get_text(
        &format!("{base_url}{SEARCH_PATH}"),
        &[("q", query)],
        None,
        timeout,
    )
}

pub fn parse_search(body: &str) -> Result<Vec<Candidate>> {
    let results: Vec<ApiSearchResult> = serde_json::from_str(body)?;

    Ok(results
        .into_iter()
        .map(|result| result.show.into_candidate())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::{Candidate, Source, parse_search, search_at};
    use crate::Error;
    use crate::http::test_server::{TEST_TIMEOUT, refuse, serve, serve_silently};

    #[test]
    fn maps_a_saved_search_response() {
        let body = include_str!("../../tests/fixtures/tvmaze/search-a.json");

        let shows = parse_search(body).unwrap();

        let show = shows
            .iter()
            .find(|show| show.external_id == "39235")
            .expect("fixture contains A Confession");
        assert_eq!(
            show,
            &Candidate {
                source: Source::TvMaze,
                external_id: "39235".to_owned(),
                title: "A Confession".to_owned(),
                release_year: Some(2019),
                cover_url: Some(
                    "https://static.tvmaze.com/uploads/images/medium_portrait/210/525047.jpg"
                        .to_owned()
                ),
            }
        );
    }

    #[test]
    fn the_saved_response_keeps_shows_without_details() {
        let body = include_str!("../../tests/fixtures/tvmaze/search-a.json");

        let shows = parse_search(body).unwrap();

        let show = shows
            .iter()
            .find(|show| show.external_id == "67073")
            .expect("fixture contains A Lot");
        assert_eq!(show.title, "A Lot");
        assert_eq!(show.release_year, None);
        assert_eq!(show.cover_url, None);
    }

    #[test]
    fn a_show_without_a_premiere_or_image_maps_to_none() {
        let body = r#"[{"score":1.0,"show":{"id":7,"name":"Bare","premiered":null,"image":null}}]"#;

        let shows = parse_search(body).unwrap();

        assert_eq!(
            shows,
            vec![Candidate {
                source: Source::TvMaze,
                external_id: "7".to_owned(),
                title: "Bare".to_owned(),
                release_year: None,
                cover_url: None,
            }]
        );
    }

    #[test]
    fn invalid_json_is_an_error() {
        assert!(parse_search("{").is_err());
    }

    #[test]
    fn an_empty_result_is_not_an_error() {
        assert_eq!(parse_search("[]").unwrap(), Vec::<Candidate>::new());
    }

    #[test]
    fn original_is_used_when_medium_is_missing() {
        let body = r#"[{"show":{"id":8,"name":"Fallback","premiered":null,"image":{"medium":null,"original":"https://example.test/original.jpg"}}}]"#;

        let shows = parse_search(body).unwrap();

        assert_eq!(
            shows[0].cover_url.as_deref(),
            Some("https://example.test/original.jpg")
        );
    }

    #[test]
    fn http_429_is_rate_limited() {
        let base = serve("429 Too Many Requests", b"");

        let error = search_at(&base, "a", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::RateLimited));
    }

    #[test]
    fn http_500_reports_its_status_code() {
        let base = serve("500 Internal Server Error", b"");

        let error = search_at(&base, "a", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::HttpStatus { code: 500 }));
    }

    #[test]
    fn a_refused_connection_is_a_connection_failure() {
        let error = search_at(&refuse(), "a", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::ConnectionFailed));
    }

    #[test]
    fn a_silent_server_times_out() {
        let error = search_at(&serve_silently(), "a", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::ConnectionFailed));
    }

    #[test]
    fn invalid_json_after_a_200_is_a_json_error() {
        let base = serve("200 OK", b"{");

        let error = search_at(&base, "a", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::Json(_)));
    }

    #[test]
    fn an_empty_result_after_a_200_is_not_an_error() {
        let base = serve("200 OK", b"[]");

        assert_eq!(
            search_at(&base, "a", TEST_TIMEOUT).unwrap(),
            Vec::<Candidate>::new()
        );
    }
}
