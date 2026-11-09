use crate::providers::{Candidate, Source};
use crate::{Error, Result};
use serde::Deserialize;
use std::time::Duration;

const BASE_URL: &str = "https://api.themoviedb.org/3";
const IMAGE_BASE_URL: &str = "https://image.tmdb.org/t/p/w342";
const USER_AGENT: &str = concat!(
    "Loregalia/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/ManusolJ/loregalia)"
);
const TIMEOUT: Duration = Duration::from_secs(10);

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
}

impl ApiMovie {
    fn into_candidate(self) -> Option<Candidate> {
        let title = self.title?.trim().to_owned();
        if title.is_empty() {
            return None;
        }

        Some(Candidate {
            source: Source::Tmdb,
            external_id: self.id.to_string(),
            title,
            release_year: release_year(self.release_date.as_deref()),
            cover_url: self
                .poster_path
                .map(|path| format!("{IMAGE_BASE_URL}{path}")),
        })
    }
}

fn release_year(release_date: Option<&str>) -> Option<i32> {
    release_date?.get(..4)?.parse().ok()
}

pub fn search(token: &str, query: &str) -> Result<Vec<Candidate>> {
    search_at(BASE_URL, token, query, TIMEOUT)
}

pub fn details(token: &str, id: &str) -> Result<Candidate> {
    details_at(BASE_URL, token, id, TIMEOUT)
}

fn search_at(
    base_url: &str,
    token: &str,
    query: &str,
    timeout: Duration,
) -> Result<Vec<Candidate>> {
    let body = get_at(
        &format!("{base_url}/search/movie"),
        token,
        &[
            ("query", query),
            ("include_adult", "false"),
            ("language", "en-US"),
        ],
        timeout,
    )?;
    parse_search(&body)
}

fn details_at(base_url: &str, token: &str, id: &str, timeout: Duration) -> Result<Candidate> {
    let body = get_at(
        &format!("{base_url}/movie/{id}"),
        token,
        &[("language", "en-US")],
        timeout,
    )?;
    parse_details(&body)
}

fn get_at(url: &str, token: &str, query: &[(&str, &str)], timeout: Duration) -> Result<String> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .build();
    let agent: ureq::Agent = config.into();
    let mut request = agent
        .get(url)
        .header("User-Agent", USER_AGENT)
        .header("Authorization", &format!("Bearer {token}"));

    for (name, value) in query {
        request = request.query(name, value);
    }

    let mut response = request.call().map_err(classify)?;
    response.body_mut().read_to_string().map_err(classify)
}

fn classify(error: ureq::Error) -> Error {
    match error {
        ureq::Error::StatusCode(401 | 403) => Error::Unauthorized,
        ureq::Error::StatusCode(429) => Error::RateLimited,
        ureq::Error::StatusCode(code) => Error::HttpStatus { code },
        _ => Error::ConnectionFailed,
    }
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

    movie
        .into_candidate()
        .ok_or(Error::HttpStatus { code: 404 })
}

#[cfg(test)]
mod tests {
    use super::{Error, Source, details_at, parse_details, parse_search, search_at};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::Duration;

    const TEST_TIMEOUT: Duration = Duration::from_millis(250);

    fn serve(status: &str, body: &str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            let _ = stream.write_all(response.as_bytes());
        });

        format!("http://{address}")
    }

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
        let base = serve("401 Unauthorized", "");

        let error = search_at(&base, "bad-token", "dune", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::Unauthorized));
    }

    #[test]
    fn http_429_is_rate_limited() {
        let base = serve("429 Too Many Requests", "");

        let error = search_at(&base, "token", "dune", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::RateLimited));
    }

    #[test]
    fn http_500_reports_its_status_code() {
        let base = serve("500 Internal Server Error", "");

        let error = search_at(&base, "token", "dune", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::HttpStatus { code: 500 }));
    }

    #[test]
    fn a_missing_movie_is_a_404() {
        let base = serve("404 Not Found", "");

        let error = details_at(&base, "token", "1", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::HttpStatus { code: 404 }));
    }

    #[test]
    fn a_refused_connection_is_a_connection_failure() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);

        let error =
            search_at(&format!("http://{address}"), "token", "dune", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::ConnectionFailed));
    }
}
