use crate::{Error, Result};
use serde::Deserialize;
use std::time::Duration;

const BASE_URL: &str = "https://api.tvmaze.com";
const SEARCH_PATH: &str = "/search/shows";
const USER_AGENT: &str = concat!(
    "Loregalia/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/ManusolJ/loregalia)"
);
const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Show {
    pub external_id: i64,
    pub title: String,
    pub release_year: Option<i32>,
    pub cover_url: Option<String>,
}

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
    fn into_show(self) -> Show {
        Show {
            external_id: self.id,
            release_year: release_year(self.premiered.as_deref()),
            title: self.name,
            cover_url: self.image.and_then(|image| image.medium.or(image.original)),
        }
    }
}

fn release_year(premiered: Option<&str>) -> Option<i32> {
    premiered?.get(..4)?.parse().ok()
}

pub fn search(query: &str) -> Result<Vec<Show>> {
    search_at(BASE_URL, query, TIMEOUT)
}

pub fn search_json(query: &str) -> Result<String> {
    search_json_at(BASE_URL, query, TIMEOUT)
}

fn search_at(base_url: &str, query: &str, timeout: Duration) -> Result<Vec<Show>> {
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

pub fn parse_search(body: &str) -> Result<Vec<Show>> {
    let results: Vec<ApiSearchResult> = serde_json::from_str(body)?;

    Ok(results
        .into_iter()
        .map(|result| result.show.into_show())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::{Error, Show, parse_search, search_at};
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

    fn serve_silently() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();

        std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            std::thread::sleep(Duration::from_millis(750));
            drop(stream);
        });

        format!("http://{address}")
    }

    #[test]
    fn maps_a_saved_search_response() {
        let body = include_str!("../../tests/fixtures/tvmaze/search-a.json");

        let shows = parse_search(body).unwrap();

        let show = shows
            .iter()
            .find(|show| show.external_id == 39235)
            .expect("fixture contains A Confession");
        assert_eq!(
            show,
            &Show {
                external_id: 39235,
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
            .find(|show| show.external_id == 67073)
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
            vec![Show {
                external_id: 7,
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
        assert_eq!(parse_search("[]").unwrap(), Vec::<Show>::new());
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
        let base = serve("429 Too Many Requests", "");

        let error = search_at(&base, "a", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::RateLimited));
    }

    #[test]
    fn http_500_reports_its_status_code() {
        let base = serve("500 Internal Server Error", "");

        let error = search_at(&base, "a", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::HttpStatus { code: 500 }));
    }

    #[test]
    fn a_refused_connection_is_a_connection_failure() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);

        let error = search_at(&format!("http://{address}"), "a", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::ConnectionFailed));
    }

    #[test]
    fn a_silent_server_times_out() {
        let base = serve_silently();

        let error = search_at(&base, "a", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::ConnectionFailed));
    }

    #[test]
    fn invalid_json_after_a_200_is_a_json_error() {
        let base = serve("200 OK", "{");

        let error = search_at(&base, "a", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::Json(_)));
    }

    #[test]
    fn an_empty_result_after_a_200_is_not_an_error() {
        let base = serve("200 OK", "[]");

        assert_eq!(
            search_at(&base, "a", TEST_TIMEOUT).unwrap(),
            Vec::<Show>::new()
        );
    }
}
