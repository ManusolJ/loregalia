use super::Candidate;
use crate::http;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const LOCAL_RELAY_URL: &str = "http://127.0.0.1:8080";

pub fn base_url() -> &'static str {
    option_env!("LOREGALIA_RELAY_URL").unwrap_or(LOCAL_RELAY_URL)
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SearchResponse {
    pub results: Vec<Candidate>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub fn search_movies(base_url: &str, query: &str) -> Result<Vec<Candidate>> {
    search_movies_at(base_url, query, http::TIMEOUT)
}

pub fn movie_details(base_url: &str, id: &str) -> Result<Candidate> {
    movie_details_at(base_url, id, http::TIMEOUT)
}

fn search_movies_at(base_url: &str, query: &str, timeout: Duration) -> Result<Vec<Candidate>> {
    let body = get(
        &format!("{base_url}/v1/movies/search"),
        &[("q", query)],
        timeout,
    )?;
    let response: SearchResponse = serde_json::from_str(&body)?;

    Ok(response.results)
}

fn movie_details_at(base_url: &str, id: &str, timeout: Duration) -> Result<Candidate> {
    let body = get(&format!("{base_url}/v1/movies/{id}"), &[], timeout)?;

    serde_json::from_str(&body).map_err(Error::from)
}

fn get(url: &str, query: &[(&str, &str)], timeout: Duration) -> Result<String> {
    http::get_text(url, query, None, timeout).map_err(|error| match error {
        Error::HttpStatus { code: 502..=504 } => Error::RelayUnavailable,
        other => other,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        Error, LOCAL_RELAY_URL, SearchResponse, base_url, movie_details_at, search_movies_at,
    };
    use crate::http::test_server::{TEST_TIMEOUT, serve};
    use crate::providers::{Candidate, Source};

    fn candidate() -> Candidate {
        Candidate {
            source: Source::Tmdb,
            external_id: "438631".to_owned(),
            title: "Dune".to_owned(),
            release_year: Some(2021),
            cover_url: Some("https://image.example/dune.jpg".to_owned()),
        }
    }

    #[test]
    fn the_relay_url_has_a_usable_default() {
        assert!(LOCAL_RELAY_URL.starts_with("http://"));
        assert!(base_url().starts_with("http"));
    }

    #[test]
    fn a_search_response_round_trips() {
        let response = SearchResponse {
            results: vec![candidate()],
        };

        let json = serde_json::to_string(&response).unwrap();
        assert_eq!(
            serde_json::from_str::<SearchResponse>(&json).unwrap(),
            response
        );
    }

    #[test]
    fn an_error_response_round_trips() {
        let response = super::ErrorResponse {
            error: "rate_limited".to_owned(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert_eq!(
            serde_json::from_str::<super::ErrorResponse>(&json).unwrap(),
            response
        );
    }

    #[test]
    fn search_reads_relay_results() {
        let body = serde_json::to_string(&SearchResponse {
            results: vec![candidate()],
        })
        .unwrap();
        let base = serve("200 OK", body.as_bytes());

        assert_eq!(
            search_movies_at(&base, "dune", TEST_TIMEOUT).unwrap(),
            vec![candidate()]
        );
    }

    #[test]
    fn details_reads_a_candidate() {
        let body = serde_json::to_string(&candidate()).unwrap();
        let base = serve("200 OK", body.as_bytes());

        assert_eq!(
            movie_details_at(&base, "438631", TEST_TIMEOUT).unwrap(),
            candidate()
        );
    }

    #[test]
    fn a_missing_relay_is_unavailable() {
        let base = serve("503 Service Unavailable", b"");

        let error = search_movies_at(&base, "dune", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::RelayUnavailable));
    }

    #[test]
    fn a_rate_limited_relay_is_rate_limited() {
        let base = serve("429 Too Many Requests", b"");

        let error = search_movies_at(&base, "dune", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::RateLimited));
    }

    #[test]
    fn a_missing_movie_keeps_its_status() {
        let base = serve("404 Not Found", b"");

        let error = movie_details_at(&base, "1", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::HttpStatus { code: 404 }));
    }

    #[test]
    fn invalid_relay_json_is_an_error() {
        let base = serve("200 OK", b"{");

        assert!(matches!(
            movie_details_at(&base, "1", TEST_TIMEOUT),
            Err(Error::Json(_))
        ));
    }
}
