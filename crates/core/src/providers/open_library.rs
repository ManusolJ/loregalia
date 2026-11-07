use crate::providers::{Candidate, Source};
use crate::{Error, Result};
use serde::Deserialize;
use std::time::Duration;

const BASE_URL: &str = "https://openlibrary.org";
const SEARCH_PATH: &str = "/search.json";
const COVER_BASE_URL: &str = "https://covers.openlibrary.org/b/id";
const LIMIT: &str = "20";
const USER_AGENT: &str = concat!(
    "Loregalia/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/ManusolJ/loregalia)"
);
const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Deserialize)]
struct ApiSearch {
    docs: Vec<ApiDoc>,
}

#[derive(Deserialize)]
struct ApiDoc {
    key: Option<String>,
    title: Option<String>,
    first_publish_year: Option<i32>,
    cover_i: Option<i64>,
}

impl ApiDoc {
    fn into_candidate(self) -> Option<Candidate> {
        let title = self.title?.trim().to_owned();
        let external_id = self.key?.trim_start_matches("/works/").to_owned();
        if title.is_empty() || external_id.is_empty() {
            return None;
        }

        Some(Candidate {
            source: Source::OpenLibrary,
            external_id,
            title,
            release_year: self.first_publish_year,
            cover_url: self
                .cover_i
                .map(|cover| format!("{COVER_BASE_URL}/{cover}-M.jpg?default=false")),
        })
    }
}

pub fn search(query: &str) -> Result<Vec<Candidate>> {
    search_at(BASE_URL, query, TIMEOUT)
}

fn search_at(base_url: &str, query: &str, timeout: Duration) -> Result<Vec<Candidate>> {
    let body = search_json_at(base_url, query, timeout)?;
    parse_search(&body)
}

fn search_json_at(base_url: &str, query: &str, timeout: Duration) -> Result<String> {
    let url = format!("{base_url}{SEARCH_PATH}");
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .build();
    let agent: ureq::Agent = config.into();
    let mut response = agent
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .query("q", query)
        .query("limit", LIMIT)
        .call()
        .map_err(classify)?;

    response.body_mut().read_to_string().map_err(classify)
}

fn classify(error: ureq::Error) -> Error {
    match error {
        ureq::Error::StatusCode(429) => Error::RateLimited,
        ureq::Error::StatusCode(code) => Error::HttpStatus { code },
        _ => Error::ConnectionFailed,
    }
}

pub fn parse_search(body: &str) -> Result<Vec<Candidate>> {
    let results: ApiSearch = serde_json::from_str(body)?;

    Ok(results
        .docs
        .into_iter()
        .filter_map(ApiDoc::into_candidate)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::{Error, Source, parse_search, search_at};
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
        let body = include_str!("../../tests/fixtures/openlibrary/search-piranesi.json");

        let books = parse_search(body).unwrap();

        assert_eq!(books[0].source, Source::OpenLibrary);
        assert_eq!(books[0].external_id, "OL20893680W");
        assert_eq!(books[0].title, "Piranesi");
        assert_eq!(books[0].release_year, Some(2020));
        assert_eq!(
            books[0].cover_url.as_deref(),
            Some("https://covers.openlibrary.org/b/id/10226290-M.jpg?default=false")
        );
    }

    #[test]
    fn keeps_a_book_without_a_cover() {
        let body = include_str!("../../tests/fixtures/openlibrary/search-piranesi.json");

        let books = parse_search(body).unwrap();
        let book = books
            .iter()
            .find(|book| book.external_id == "OL12523946W")
            .unwrap();

        assert_eq!(book.title, "Piranesi");
        assert_eq!(book.release_year, Some(1910));
        assert_eq!(book.cover_url, None);
    }

    #[test]
    fn skips_entries_without_a_title_or_work_key() {
        let body = r#"{"docs":[
            {"key":"/works/OL1W","title":"Keep"},
            {"key":"/works/OL2W"},
            {"title":"No key"},
            {"key":"/works/OL3W","title":"   "}
        ]}"#;

        let books = parse_search(body).unwrap();

        assert_eq!(books.len(), 1);
        assert_eq!(books[0].title, "Keep");
    }

    #[test]
    fn an_empty_result_is_not_an_error() {
        assert_eq!(parse_search(r#"{"docs":[]}"#).unwrap(), Vec::new());
    }

    #[test]
    fn invalid_json_is_an_error() {
        assert!(matches!(parse_search("{"), Err(Error::Json(_))));
    }

    #[test]
    fn http_429_is_rate_limited() {
        let base = serve("429 Too Many Requests", "");

        let error = search_at(&base, "piranesi", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::RateLimited));
    }

    #[test]
    fn http_500_reports_its_status_code() {
        let base = serve("500 Internal Server Error", "");

        let error = search_at(&base, "piranesi", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::HttpStatus { code: 500 }));
    }

    #[test]
    fn a_refused_connection_is_a_connection_failure() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);

        let error = search_at(&format!("http://{address}"), "piranesi", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::ConnectionFailed));
    }
}
