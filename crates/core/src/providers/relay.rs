use super::Candidate;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const LOCAL_RELAY_URL: &str = "http://127.0.0.1:8080";

const USER_AGENT: &str = concat!(
    "Loregalia/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/ManusolJ/loregalia)"
);
const TIMEOUT: Duration = Duration::from_secs(10);

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
    search_movies_at(base_url, query, TIMEOUT)
}

pub fn movie(base_url: &str, id: &str) -> Result<Candidate> {
    movie_at(base_url, id, TIMEOUT)
}

fn search_movies_at(base_url: &str, query: &str, timeout: Duration) -> Result<Vec<Candidate>> {
    let body = get_at(
        &format!("{base_url}/v1/movies/search"),
        &[("q", query)],
        timeout,
    )?;
    let response: SearchResponse = serde_json::from_str(&body)?;

    Ok(response.results)
}

fn movie_at(base_url: &str, id: &str, timeout: Duration) -> Result<Candidate> {
    let body = get_at(&format!("{base_url}/v1/movies/{id}"), &[], timeout)?;

    serde_json::from_str(&body).map_err(Error::from)
}

fn get_at(url: &str, query: &[(&str, &str)], timeout: Duration) -> Result<String> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .build();
    let agent: ureq::Agent = config.into();
    let mut request = agent.get(url).header("User-Agent", USER_AGENT);

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
        ureq::Error::StatusCode(502..=504) => Error::RelayUnavailable,
        ureq::Error::StatusCode(code) => Error::HttpStatus { code },
        _ => Error::ConnectionFailed,
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, LOCAL_RELAY_URL, SearchResponse, base_url, movie_at, search_movies_at};
    use crate::providers::{Candidate, Source};
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
        let base = serve("200 OK", &body);

        assert_eq!(
            search_movies_at(&base, "dune", TEST_TIMEOUT).unwrap(),
            vec![candidate()]
        );
    }

    #[test]
    fn details_reads_a_candidate() {
        let body = serde_json::to_string(&candidate()).unwrap();
        let base = serve("200 OK", &body);

        assert_eq!(
            movie_at(&base, "438631", TEST_TIMEOUT).unwrap(),
            candidate()
        );
    }

    #[test]
    fn a_missing_relay_is_unavailable() {
        let base = serve("503 Service Unavailable", "");

        let error = search_movies_at(&base, "dune", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::RelayUnavailable));
    }

    #[test]
    fn a_rate_limited_relay_is_rate_limited() {
        let base = serve("429 Too Many Requests", "");

        let error = search_movies_at(&base, "dune", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::RateLimited));
    }

    #[test]
    fn invalid_relay_json_is_an_error() {
        let base = serve("200 OK", "{");

        assert!(matches!(
            movie_at(&base, "1", TEST_TIMEOUT),
            Err(Error::Json(_))
        ));
    }
}
