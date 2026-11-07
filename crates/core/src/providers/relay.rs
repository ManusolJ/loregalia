use super::Candidate;
use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use super::{ErrorResponse, LOCAL_RELAY_URL, SearchResponse, base_url};
    use crate::providers::{Candidate, Source};

    #[test]
    fn the_relay_url_has_a_usable_default() {
        assert!(LOCAL_RELAY_URL.starts_with("http://"));
        assert!(base_url().starts_with("http"));
    }

    #[test]
    fn a_search_response_round_trips() {
        let response = SearchResponse {
            results: vec![Candidate {
                source: Source::Tmdb,
                external_id: "438631".to_owned(),
                title: "Dune".to_owned(),
                release_year: Some(2021),
                cover_url: Some("https://image.example/dune.jpg".to_owned()),
            }],
        };

        let json = serde_json::to_string(&response).unwrap();
        assert_eq!(
            serde_json::from_str::<SearchResponse>(&json).unwrap(),
            response
        );
    }

    #[test]
    fn an_error_response_round_trips() {
        let response = ErrorResponse {
            error: "rate_limited".to_owned(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert_eq!(
            serde_json::from_str::<ErrorResponse>(&json).unwrap(),
            response
        );
    }
}
