use crate::{Error, Result};
use serde::Deserialize;
use std::time::Duration;

const SEARCH_URL: &str = "https://api.tvmaze.com/search/shows";
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
            cover_url: self.image.and_then(|image| image.original.or(image.medium)),
        }
    }
}

fn release_year(premiered: Option<&str>) -> Option<i32> {
    premiered?.get(..4)?.parse().ok()
}

pub fn search(query: &str) -> Result<Vec<Show>> {
    let body = search_json(query)?;
    parse_search(&body)
}

pub fn search_json(query: &str) -> Result<String> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .build();
    let agent: ureq::Agent = config.into();
    let mut response = agent
        .get(SEARCH_URL)
        .header("User-Agent", USER_AGENT)
        .query("q", query)
        .call()
        .map_err(|error| Error::Network(error.to_string()))?;

    response
        .body_mut()
        .read_to_string()
        .map_err(|error| Error::Network(error.to_string()))
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
    use super::{Show, parse_search};

    #[test]
    fn maps_a_saved_search_response() {
        let body = include_str!("../../tests/fixtures/tvmaze/search-a.json");

        let shows = parse_search(body).unwrap();

        let show = shows
            .iter()
            .find(|show| show.external_id == 38339)
            .expect("fixture contains A Teacher");
        assert_eq!(
            show,
            &Show {
                external_id: 38339,
                title: "A Teacher".to_owned(),
                release_year: Some(2020),
                cover_url: Some(
                    "https://static.tvmaze.com/uploads/images/original_untouched/272/681431.jpg"
                        .to_owned()
                ),
            }
        );
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
}
